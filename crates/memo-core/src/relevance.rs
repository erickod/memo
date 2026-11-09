//! Relevância em torno de um alvo explícito: quatro estratégias com peso visível.
//!
//! - `vizinhanca`: chamadas exatas (`CHAMA`/`CRIA` sem `~`) até 2 saltos na mesma direção
//!   (chamador do chamador, chamado do chamado); o segundo vale metade. Irmãos que só se ligam
//!   por um hub em comum (todos chamam `get_session`) ficam de fora.
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
    for level in 1..=2usize {
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
