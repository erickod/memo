//! Relevância em torno de um alvo explícito: quatro estratégias com peso visível.
//!
//! - `vizinhanca`: chamadas exatas (`CHAMA`/`CRIA` sem `~`) até 3 saltos na mesma direção
//!   (chamador do chamador, chamado do chamado); o salto `d` vale `peso / d`. Irmãos que só se
//!   ligam por um hub em comum (todos chamam `get_session`) ficam de fora.
//! - `caminho`: o símbolo está num caminho de chamadas exatas de um entrypoint até o alvo.
//! - `conexao`: fração das ligações exatas do candidato que vão para a região do alvo (0 a 1);
//!   um hub chamado por todo o repositório fica perto de 0.
//!   Só pontua candidatos das outras estratégias; não acrescenta nenhum.
//! - `nome`: uso com o nome exato do alvo sem chamada resolvida, e homônimos; só quando o nome
//!   não é genérico. Evidência fraca: homônimos de bibliotecas externas também casam.
//!
//! Medido no `bope` (8 alvos com referência lida no código): só chamadas exatas até 2 saltos
//! deram 96% das referências no top 5, contra 77% do `impact` anterior; o nome exato
//! acrescentou 45 usos no repositório inteiro, com cerca de 73% de acerto.

use crate::config::StrategyWeights;
use crate::error::CoreError;
use crate::query::{SymbolAddr, addr_by_id, resolve_qname};
use crate::store::SqliteStore;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// Abaixo deste número de definições um nome nunca é genérico, mesmo num repositório pequeno
/// em que o P99 é 1 ou 2 (no `bope`, o P99 é 4).
const MIN_GENERIC_DEFINITIONS: i64 = 4;
/// Piso do limiar de usos para o critério de precisão, pelo mesmo motivo.
const MIN_GENERIC_USES: i64 = 10;
/// Saltos da vizinhança. Medido no `bope`: 3 empata com 2 em fluxos curtos e acha mais do
/// fluxo nos longos (alcance de 65% para 87%); 4 ou mais só trouxe ruído.
const NEIGHBOR_DEPTH: usize = 3;
/// Profundidade da busca reversa até um entrypoint.
const PATH_DEPTH: usize = 6;

/// Quanto o nome exato do alvo distingue o alvo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NameStats {
    pub name: String,
    /// Funções e métodos do repositório com este nome.
    pub definitions: i64,
    /// Chamadas cujo último segmento é este nome.
    pub uses: i64,
    /// Dessas, quantas resolvem de forma exata para alguma definição com este nome.
    pub resolved: i64,
    /// Limiares do repositório (P99, com piso): definições e usos.
    pub definitions_threshold: i64,
    pub uses_threshold: i64,
    /// Definições ≥ limiar, ou usos ≥ limiar com menos da metade resolvida.
    pub generic: bool,
}

fn last_segment(text: &str) -> &str {
    text.rsplit('.').next().unwrap_or(text)
}

fn p99(mut values: Vec<i64>) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[(values.len() - 1) * 99 / 100]
}

pub fn name_stats(store: &SqliteStore, name: &str) -> Result<NameStats, CoreError> {
    let mut defs: HashMap<String, i64> = HashMap::new();
    for row in store
        .conn
        .prepare("SELECT name FROM symbol WHERE kind IN ('function', 'method')")?
        .query_map([], |r| r.get::<_, String>(0))?
    {
        *defs.entry(row?).or_insert(0) += 1;
    }
    let mut uses: HashMap<String, i64> = HashMap::new();
    for row in store
        .conn
        .prepare(
            "SELECT callee_text FROM raw_ref
             WHERE kind IN ('CHAMA', 'CHAMA_CONSTRUIDA') AND callee_text IS NOT NULL",
        )?
        .query_map([], |r| r.get::<_, String>(0))?
    {
        *uses.entry(last_segment(&row?).to_string()).or_insert(0) += 1;
    }
    let resolved: i64 = store.conn.query_row(
        "SELECT COUNT(*) FROM raw_ref r
         WHERE r.kind IN ('CHAMA', 'CHAMA_CONSTRUIDA')
           AND (r.callee_text = ?1 OR substr(r.callee_text, -length(?1) - 1) = '.' || ?1)
           AND EXISTS (SELECT 1 FROM edge e JOIN symbol d ON d.id = e.dst
                       WHERE e.src = r.src AND e.line = r.line AND e.kind = 'CHAMA'
                         AND e.approx = 0 AND d.name = ?1)",
        [name],
        |r| r.get(0),
    )?;
    let definitions_threshold = p99(defs.values().copied().collect()).max(MIN_GENERIC_DEFINITIONS);
    let uses_threshold = p99(defs
        .keys()
        .map(|n| uses.get(n).copied().unwrap_or(0))
        .collect())
    .max(MIN_GENERIC_USES);
    let definitions = defs.get(name).copied().unwrap_or(0);
    let use_count = uses.get(name).copied().unwrap_or(0);
    let generic = definitions >= definitions_threshold
        || (use_count >= uses_threshold && resolved * 2 < use_count);
    Ok(NameStats {
        name: name.to_string(),
        definitions,
        uses: use_count,
        resolved,
        definitions_threshold,
        uses_threshold,
        generic,
    })
}

/// Peso de uma estratégia para um item.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrategyScore {
    pub strategy: &'static str,
    pub weight: f64,
}

/// Um símbolo relevante para o alvo, com o peso de cada estratégia que o sustenta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Relevant {
    #[serde(flatten)]
    pub addr: SymbolAddr,
    /// Saltos de chamada exata até o alvo; `None` quando só o nome o liga.
    pub depth: Option<usize>,
    /// `chamador`, `chamado` ou `nome`.
    pub direction: &'static str,
    /// Rota HTTP quando o símbolo é um entrypoint com rota.
    pub route: Option<String>,
    pub entrypoint: bool,
    pub strategies: Vec<StrategyScore>,
    pub score: f64,
}

struct CallGraph {
    forward: HashMap<i64, BTreeSet<i64>>,
    reverse: HashMap<i64, BTreeSet<i64>>,
    tests: HashSet<i64>,
}

impl CallGraph {
    fn load(store: &SqliteStore) -> Result<Self, CoreError> {
        let mut forward: HashMap<i64, BTreeSet<i64>> = HashMap::new();
        let mut reverse: HashMap<i64, BTreeSet<i64>> = HashMap::new();
        for row in store
            .conn
            .prepare(
                "SELECT src, dst FROM edge
                 WHERE kind IN ('CHAMA', 'CRIA') AND approx = 0 AND dst IS NOT NULL AND src != dst",
            )?
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?
        {
            let (src, dst) = row?;
            forward.entry(src).or_default().insert(dst);
            reverse.entry(dst).or_default().insert(src);
        }
        let mut tests = HashSet::new();
        for row in store
            .conn
            .prepare("SELECT s.id, s.is_test, COALESCE(f.path, '') FROM symbol s LEFT JOIN file f ON f.id = s.file_id")?
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? != 0, r.get::<_, String>(2)?))
            })?
        {
            let (id, is_test, path) = row?;
            if is_test || crate::index::is_test_file(&path) {
                tests.insert(id);
            }
        }
        Ok(Self {
            forward,
            reverse,
            tests,
        })
    }

    fn next<'a>(
        &'a self,
        map: &'a HashMap<i64, BTreeSet<i64>>,
        node: i64,
    ) -> impl Iterator<Item = i64> + 'a {
        map.get(&node)
            .into_iter()
            .flatten()
            .copied()
            .filter(|n| !self.tests.contains(n))
    }

    /// Distância por chamadas exatas reversas (quem chama), sem passar por testes.
    fn ancestors(&self, target: i64, depth: usize) -> HashMap<i64, usize> {
        let mut dist = HashMap::from([(target, 0)]);
        let mut queue = VecDeque::from([target]);
        while let Some(node) = queue.pop_front() {
            if dist[&node] >= depth {
                continue;
            }
            for other in self.next(&self.reverse, node).collect::<Vec<_>>() {
                if !dist.contains_key(&other) {
                    dist.insert(other, dist[&node] + 1);
                    queue.push_back(other);
                }
            }
        }
        dist
    }
}

fn degree(graph: &CallGraph, node: i64) -> usize {
    graph.forward.get(&node).map_or(0, BTreeSet::len)
        + graph.reverse.get(&node).map_or(0, BTreeSet::len)
}

/// As quatro estratégias em torno de `target` (id), ordenadas por pontuação total.
pub fn relevant(
    store: &SqliteStore,
    target: i64,
    weights: &StrategyWeights,
) -> Result<Vec<Relevant>, CoreError> {
    let graph = CallGraph::load(store)?;
    let mut scores: BTreeMap<i64, Vec<StrategyScore>> = BTreeMap::new();
    let mut depth: HashMap<i64, usize> = HashMap::new();
    let mut direction: HashMap<i64, &'static str> = HashMap::new();

    // Vizinhança: até 2 saltos por chamada exata, nos dois sentidos.
    let mut frontier = vec![(target, "")];
    let mut seen = HashSet::from([target]);
    for level in 1..=NEIGHBOR_DEPTH {
        let mut next = Vec::new();
        for (node, dir) in &frontier {
            for (map, step) in [(&graph.reverse, "chamador"), (&graph.forward, "chamado")] {
                for other in graph.next(map, *node).collect::<Vec<_>>() {
                    if !dir.is_empty() && *dir != step {
                        continue;
                    }
                    if !seen.insert(other) {
                        continue;
                    }
                    let label = step;
                    depth.insert(other, level);
                    direction.insert(other, label);
                    let weight = weights.neighborhood / level as f64;
                    scores.entry(other).or_default().push(StrategyScore {
                        strategy: "vizinhanca",
                        weight,
                    });
                    next.push((other, label));
                }
            }
        }
        frontier = next;
    }

    // Caminho: de cada entrypoint que alcança o alvo, os nós que descem até ele.
    let ancestors = graph.ancestors(target, PATH_DEPTH);
    let entrypoints: BTreeSet<i64> = store
        .conn
        .prepare("SELECT id FROM symbol WHERE is_entrypoint = 1")?
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut on_path: BTreeSet<i64> = BTreeSet::new();
    let mut stack: Vec<i64> = entrypoints
        .iter()
        .copied()
        .filter(|e| ancestors.contains_key(e))
        .collect();
    while let Some(node) = stack.pop() {
        if !on_path.insert(node) {
            continue;
        }
        let here = ancestors[&node];
        stack.extend(
            graph
                .next(&graph.forward, node)
                .filter(|n| ancestors.get(n).is_some_and(|d| here > 0 && *d == here - 1)),
        );
    }
    on_path.remove(&target);
    for &node in &on_path {
        depth.entry(node).or_insert(ancestors[&node]);
        direction.entry(node).or_insert("chamador");
        scores.entry(node).or_default().push(StrategyScore {
            strategy: "caminho",
            weight: weights.path,
        });
    }

    // Nome exato: só para nome não genérico; usos sem chamada exata e homônimos.
    let target_name: String =
        store
            .conn
            .query_row("SELECT name FROM symbol WHERE id = ?1", [target], |r| {
                r.get(0)
            })?;
    let stats = name_stats(store, &target_name)?;
    if !stats.generic && weights.name > 0.0 {
        let users: Vec<i64> = store
            .conn
            .prepare(
                "SELECT DISTINCT r.src FROM raw_ref r
                 WHERE r.kind IN ('CHAMA', 'CHAMA_CONSTRUIDA')
                   AND (r.callee_text = ?1 OR substr(r.callee_text, -length(?1) - 1) = '.' || ?1)
                   AND NOT EXISTS (SELECT 1 FROM edge e JOIN symbol d ON d.id = e.dst
                                   WHERE e.src = r.src AND e.line = r.line AND e.kind = 'CHAMA'
                                     AND e.approx = 0 AND d.name = ?1)
                 UNION
                 SELECT id FROM symbol WHERE name = ?1 AND kind IN ('function', 'method')
                 ORDER BY 1",
            )?
            .query_map([&target_name], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for node in users {
            if node == target || graph.tests.contains(&node) {
                continue;
            }
            direction.entry(node).or_insert("nome");
            scores.entry(node).or_default().push(StrategyScore {
                strategy: "nome",
                weight: weights.name,
            });
        }
    }

    // Conexão: das ligações exatas do candidato, a fração que toca a região do alvo.
    let region: HashSet<i64> = scores.keys().copied().chain([target]).collect();
    let local: HashMap<i64, f64> = scores
        .keys()
        .map(|&c| {
            let touches = graph
                .next(&graph.forward, c)
                .chain(graph.next(&graph.reverse, c))
                .filter(|n| *n != c && region.contains(n))
                .collect::<HashSet<_>>()
                .len();
            (
                c,
                (touches as f64 / degree(&graph, c).max(1) as f64).min(1.0),
            )
        })
        .collect();
    if weights.connectivity > 0.0 {
        for (node, value) in &local {
            if *value > 0.0 {
                scores
                    .get_mut(node)
                    .expect("candidate")
                    .push(StrategyScore {
                        strategy: "conexao",
                        weight: weights.connectivity * value,
                    });
            }
        }
    }

    let mut out = Vec::with_capacity(scores.len());
    let mut route_of = store
        .conn
        .prepare("SELECT route, is_entrypoint FROM symbol WHERE id = ?1")?;
    for (node, strategies) in scores {
        let (route, entrypoint): (Option<String>, bool) =
            route_of.query_row([node], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))?;
        let score = strategies.iter().map(|s| s.weight).sum();
        out.push(Relevant {
            addr: addr_by_id(store, node)?,
            depth: depth.get(&node).copied(),
            direction: direction.get(&node).copied().unwrap_or("nome"),
            route,
            entrypoint,
            strategies,
            score,
        });
    }
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| {
                a.depth
                    .unwrap_or(usize::MAX)
                    .cmp(&b.depth.unwrap_or(usize::MAX))
            })
            .then_with(|| a.addr.qname.cmp(&b.addr.qname))
    });
    Ok(out)
}

/// Candidatas de um nome ambíguo que formam um só fluxo por chamadas exatas diretas entre elas
/// (ex.: resource → service com o mesmo nome), em ordem de quem chama para quem é chamado.
/// Testes ficam de fora; sem ligação entre todas, `None`.
pub fn exact_group(
    store: &SqliteStore,
    candidates: &[String],
) -> Result<Option<Vec<String>>, CoreError> {
    let mut ids: Vec<(i64, String)> = Vec::new();
    for qname in candidates {
        let (id, is_test, path): (i64, i64, String) = store.conn.query_row(
            "SELECT s.id, s.is_test, COALESCE(f.path, '') FROM symbol s
             LEFT JOIN file f ON f.id = s.file_id WHERE s.qname = ?1",
            [qname],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if is_test == 0 && !crate::index::is_test_file(&path) {
            ids.push((id, qname.clone()));
        }
    }
    if ids.len() < 2 {
        return Ok(None);
    }
    let mut stmt = store.conn.prepare(
        "SELECT EXISTS(SELECT 1 FROM edge WHERE src = ?1 AND dst = ?2
                       AND kind IN ('CHAMA', 'CRIA') AND approx = 0)",
    )?;
    let mut calls: Vec<(usize, usize)> = Vec::new();
    for (i, (a, _)) in ids.iter().enumerate() {
        for (j, (b, _)) in ids.iter().enumerate() {
            if i != j && stmt.query_row(params![a, b], |r| r.get::<_, bool>(0))? {
                calls.push((i, j));
            }
        }
    }
    // Conexo, ignorando a direção.
    let mut reached = BTreeSet::from([0usize]);
    loop {
        let before = reached.len();
        for &(i, j) in &calls {
            if reached.contains(&i) || reached.contains(&j) {
                reached.insert(i);
                reached.insert(j);
            }
        }
        if reached.len() == before {
            break;
        }
    }
    if reached.len() != ids.len() {
        return Ok(None);
    }
    let callers_of = |j: usize| calls.iter().filter(|(_, t)| *t == j).count();
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by(|&a, &b| {
        callers_of(a)
            .cmp(&callers_of(b))
            .then_with(|| ids[a].1.cmp(&ids[b].1))
    });
    Ok(Some(order.into_iter().map(|i| ids[i].1.clone()).collect()))
}

/// O alvo pedido: um `qname`, ou o grupo quando o sufixo é ambíguo mas as candidatas se ligam
/// por chamada exata. Ambíguo sem grupo continua `Ambiguous`.
pub fn resolve_targets(store: &SqliteStore, target: &str) -> Result<Vec<String>, CoreError> {
    match resolve_qname(store, target) {
        Ok(qname) => Ok(vec![qname]),
        Err(CoreError::Ambiguous { target, candidates }) => {
            match exact_group(store, &candidates)? {
                Some(group) => Ok(group),
                None => Err(CoreError::Ambiguous { target, candidates }),
            }
        }
        Err(e) => Err(e),
    }
}

/// Âncora escolhida pela interseção das palavras de um pedido em texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersectionAnchor {
    pub id: i64,
    /// Palavras do pedido (fora do `skeleton`) que o nome cobre, e quantas há.
    pub covered: usize,
    pub total: usize,
}

/// Até 3 funções ou métodos cujo nome cobre o máximo de palavras do pedido fora do `skeleton`
/// (pelo menos 2). Um método conta também as palavras da classe dona (`OrderTaxRepository.run`
/// cobre `order` e `tax`), salvo métodos especiais (`__init__`). Só ficam as candidatas
/// empatadas com a primeira em cobertura total e em cobertura pelo próprio nome, para um
/// método auxiliar que só herda as palavras não disputar com a função que as tem no nome.
/// Desempate: mais palavras do `skeleton` do pedido no nome (`process` favorece `process_x`
/// sobre `has_x`), depois menos palavras sobrando no nome.
/// Medido no `bope`: âncora certa em 13 de 16 pedidos antes destes dois ajustes.
pub fn intersection_anchors(
    store: &SqliteStore,
    tokens: &[String],
) -> Result<Vec<IntersectionAnchor>, CoreError> {
    const MIN_PREFIX: usize = 5;
    const MAX_ANCHORS: usize = 3;
    let words: Vec<(i64, String, bool)> = store
        .conn
        .prepare(
            "SELECT w.id, w.text, COALESCE(ws.esqueleto, 0) FROM word w
             LEFT JOIN word_stats ws ON ws.word_id = w.id
             WHERE EXISTS (SELECT 1 FROM occurrence o WHERE o.word_id = w.id
                           AND o.source = 'name' AND o.symbol_id IS NOT NULL)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
        .collect::<Result<_, _>>()?;
    // Cada token vira o conjunto de palavras que ele casa (igual, senão prefixo de 5 ou mais).
    let mut usable: Vec<BTreeSet<i64>> = Vec::new();
    let mut skeleton: Vec<BTreeSet<i64>> = Vec::new();
    for token in tokens {
        let exact: Vec<&(i64, String, bool)> = words.iter().filter(|w| &w.1 == token).collect();
        let hits: Vec<&(i64, String, bool)> = if exact.is_empty() {
            words
                .iter()
                .filter(|w| {
                    w.1.len().min(token.len()) >= MIN_PREFIX
                        && (w.1.starts_with(token.as_str()) || token.starts_with(w.1.as_str()))
                })
                .collect()
        } else {
            exact
        };
        let generic: BTreeSet<i64> = hits.iter().filter(|w| w.2).map(|w| w.0).collect();
        let specific: BTreeSet<i64> = hits.iter().filter(|w| !w.2).map(|w| w.0).collect();
        if !specific.is_empty() {
            usable.push(specific);
        } else if !generic.is_empty() {
            skeleton.push(generic);
        }
    }
    if usable.len() < 2 {
        return Ok(Vec::new());
    }
    // Palavras de nome de cada símbolo candidato, próprias e herdadas da classe dona.
    let mut own: HashMap<i64, BTreeSet<i64>> = HashMap::new();
    let mut inherited: HashMap<i64, BTreeSet<i64>> = HashMap::new();
    let mut by_word = store.conn.prepare(
        "SELECT symbol_id FROM occurrence WHERE word_id = ?1 AND source = 'name'
         AND symbol_id IS NOT NULL",
    )?;
    let mut methods_of = store.conn.prepare(
        "SELECT m.id FROM symbol m JOIN occurrence o ON o.symbol_id = m.parent_id
         WHERE o.word_id = ?1 AND o.source = 'name' AND m.kind = 'method'
           AND substr(m.name, 1, 2) != '__'",
    )?;
    for word in usable.iter().chain(&skeleton).flatten() {
        for id in by_word.query_map([word], |r| r.get::<_, i64>(0))? {
            own.entry(id?).or_default().insert(*word);
        }
        for id in methods_of.query_map([word], |r| r.get::<_, i64>(0))? {
            inherited.entry(id?).or_default().insert(*word);
        }
    }
    let graph_tests = |id: i64| -> Result<bool, CoreError> {
        let (kind, is_test, path): (String, i64, String) = store.conn.query_row(
            "SELECT s.kind, s.is_test, COALESCE(f.path, '') FROM symbol s
             LEFT JOIN file f ON f.id = s.file_id WHERE s.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok(!matches!(kind.as_str(), "function" | "method")
            || is_test != 0
            || crate::index::is_test_file(&path))
    };
    let mut name_len = store
        .conn
        .prepare("SELECT COUNT(*) FROM occurrence WHERE symbol_id = ?1 AND source = 'name'")?;
    let ids: BTreeSet<i64> = own.keys().chain(inherited.keys()).copied().collect();
    let mut scored: Vec<(usize, usize, usize, i64, String, i64)> = Vec::new();
    for id in ids {
        if graph_tests(id)? {
            continue;
        }
        let empty = BTreeSet::new();
        let mine = own.get(&id).unwrap_or(&empty);
        let all: BTreeSet<i64> = mine
            .union(inherited.get(&id).unwrap_or(&empty))
            .copied()
            .collect();
        let covered = usable.iter().filter(|g| !g.is_disjoint(&all)).count();
        if covered < 2 {
            continue;
        }
        let tie = skeleton.iter().filter(|g| !g.is_disjoint(&all)).count();
        let own_covered = usable.iter().filter(|g| !g.is_disjoint(mine)).count();
        let own_hits = usable
            .iter()
            .chain(&skeleton)
            .filter(|g| !g.is_disjoint(mine))
            .count() as i64;
        let extra = name_len.query_row([id], |r| r.get::<_, i64>(0))? - own_hits;
        let qname: String =
            store
                .conn
                .query_row("SELECT qname FROM symbol WHERE id = ?1", [id], |r| r.get(0))?;
        scored.push((covered, own_covered, tie, extra, qname, id));
    }
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.4.cmp(&b.4))
    });
    let Some(&(best, best_own, ..)) = scored.first() else {
        return Ok(Vec::new());
    };
    Ok(scored
        .into_iter()
        .filter(|s| s.0 == best && s.1 == best_own)
        .take(MAX_ANCHORS)
        .map(|(covered, _, _, _, _, id)| IntersectionAnchor {
            id,
            covered,
            total: usable.len(),
        })
        .collect())
}
