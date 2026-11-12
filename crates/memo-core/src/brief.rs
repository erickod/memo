//! `brief`: âncoras, símbolos, contorno, padrão, escritores e notas para um pedido (spec §8.5).

use crate::config::{BridgeWeights, StrategyWeights};
use crate::delta::filter_new;
use crate::error::CoreError;
use crate::notes::{Note, notes_for};
use crate::query::{
    ReadSpot, SymbolAddr, addr_by_id, short_name, structural_read_spot, word_pattern,
};
use crate::store::SqliteStore;
use crate::tags::bridge_tags;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_ANCHORS: usize = 5;
const MAX_SYMBOLS: usize = 15;
const MAX_TEST_SYMBOLS: usize = 5;
/// Peso de um sinal fraco (import) frente ao nome, igual ao bônus do slot `role`.
const WEAK_SIGNAL: f64 = 0.2;
const OUTLINE_SYMBOLS: usize = 5;
const MIN_PREFIX: usize = 5;
const HOP_FACTOR: f64 = 0.5;
/// Vagas de produção reservadas aos símbolos que só chegam por salto (TASK-4, D4).
const NEIGHBOR_SLOTS: usize = 5;

const STOPWORDS: &[&str] = &[
    // pt
    "de", "da", "do", "das", "dos", "em", "na", "no", "nas", "nos", "um", "uma", "uns", "umas",
    "os", "as", "ao", "aos", "pelo", "pela", "pelos", "pelas", "por", "para", "com", "sem", "que",
    "se", "ou", "ser", "ter", "mais", "como", "mas", "nao", "sao", "ha", "foi", "isso", "esse",
    "essa", "este", "esta", "seu", "sua", "ele", "ela", "ja", "so", // en
    "the", "an", "of", "to", "in", "on", "at", "for", "with", "and", "is", "are", "be", "it",
    "this", "that", "by", "from", "as", "not", "or", "was", "were", "has", "have", "its", "into",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anchor {
    pub word: String,
    /// Peso normalizado pelo maior (o primeiro vale 1,0).
    pub score: f64,
    /// Casou só por prefixo (`~`).
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoredSymbol {
    pub addr: SymbolAddr,
    pub score: f64,
    /// Pontes que trouxeram o símbolo (`nome`, `docstring`, `teste`, `chamada`, `tipo`, `import`).
    pub tags: Vec<String>,
    /// Teste, ou símbolo de um arquivo de teste (como o módulo): vai para a seção de testes.
    pub in_tests: bool,
}

/// Uma linha do contorno: arestas de chamada de um símbolo listado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outline {
    pub symbol: String,
    /// `-> nome :Llinha` (chama) e `<- nome :Llinha` (é chamado), com `~` quando aproximada.
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriterLine {
    pub attr: String,
    pub writers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Brief {
    pub request: String,
    pub anchors: Vec<Anchor>,
    pub ignored: Vec<String>,
    pub symbols: Vec<ScoredSymbol>,
    pub outline: Vec<Outline>,
    pub pattern: Option<Pattern>,
    pub writers: Vec<WriterLine>,
    pub notes: Vec<String>,
    /// Itens omitidos por já terem sido entregues nesta task (spec §8.4).
    pub unchanged: Option<usize>,
    /// Trechos que vale ler: os 5 primeiros símbolos de produção (TASK-7, D4).
    pub ler: Vec<ReadSpot>,
}

fn strip_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

/// Minúsculas, sem acentos, separado por não letras, sem stopwords pt e en.
pub fn tokenize(text: &str) -> Vec<String> {
    let folded: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphabetic() {
                strip_accent(c)
            } else {
                ' '
            }
        })
        .collect();
    let mut tokens: Vec<String> = Vec::new();
    for token in folded.split_whitespace() {
        let keep = token.chars().count() >= 2 && !STOPWORDS.contains(&token);
        if keep && !tokens.iter().any(|t| t == token) {
            tokens.push(token.to_string());
        }
    }
    tokens
}

struct Candidate {
    id: i64,
    text: String,
    approx: bool,
    weight: f64,
    skeleton: bool,
}

fn slot_bonus(slot: &str) -> f64 {
    match slot {
        "object" => 1.0,
        "action" => 0.7,
        _ => 0.2,
    }
}

/// Casa cada token com uma palavra: igualdade exata, ou prefixo de ao menos 5 caracteres.
fn match_words(store: &SqliteStore, tokens: &[String]) -> Result<Vec<Candidate>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT w.id, w.text, COALESCE(ws.especificidade, 0), COALESCE(ws.centralidade, 0),
                COALESCE(ws.esqueleto, 0)
         FROM word w LEFT JOIN word_stats ws ON ws.word_id = w.id
         WHERE EXISTS (SELECT 1 FROM occurrence o WHERE o.word_id = w.id AND o.symbol_id IS NOT NULL)
         ORDER BY w.text",
    )?;
    let words: Vec<(i64, String, f64, bool)> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, f64>(2)? * r.get::<_, f64>(3)?,
                r.get::<_, i64>(4)? != 0,
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut found: Vec<Candidate> = Vec::new();
    for token in tokens {
        let exact = words.iter().any(|w| &w.1 == token);
        for (id, text, weight, skeleton) in &words {
            let hit = if exact {
                text == token
            } else {
                let (short, long) = if text.len() <= token.len() {
                    (text, token)
                } else {
                    (token, text)
                };
                short.len() >= MIN_PREFIX && long.starts_with(short.as_str())
            };
            if !hit {
                continue;
            }
            match found.iter_mut().find(|c| c.id == *id) {
                Some(existing) => existing.approx &= !exact,
                None => found.push(Candidate {
                    id: *id,
                    text: text.clone(),
                    approx: !exact,
                    weight: *weight,
                    skeleton: *skeleton,
                }),
            }
        }
    }
    Ok(found)
}

fn neighbors(store: &SqliteStore, id: i64) -> Result<Vec<i64>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT dst FROM edge WHERE src = ?1 AND dst IS NOT NULL AND kind IN ('CHAMA', 'RECEBE', 'RETORNA')
         UNION
         SELECT src FROM edge WHERE dst = ?1 AND kind IN ('CHAMA', 'RECEBE', 'RETORNA')
         ORDER BY 1",
    )?;
    let ids = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn outline_for(store: &SqliteStore, id: i64) -> Result<Vec<String>, CoreError> {
    let mut links = Vec::new();
    let mut test_callers = 0usize;
    for (arrow, near, far) in [("->", "src", "dst"), ("<-", "dst", "src")] {
        let sql = format!(
            "SELECT o.qname, COALESCE(f.path, ''), e.line, e.approx, o.is_test FROM edge e
             JOIN symbol o ON o.id = e.{far} LEFT JOIN file f ON f.id = o.file_id
             WHERE e.{near} = ?1 AND e.kind IN ('CHAMA', 'CRIA') ORDER BY o.qname, e.line"
        );
        let mut stmt = store.conn.prepare(&sql)?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get::<_, i64>(4)? != 0,
            ))
        })?;
        for row in rows {
            let (qname, path, line, approx, is_test) = row?;
            if is_test {
                test_callers += 1;
                continue;
            }
            let approx = if approx { " ~" } else { "" };
            let line = line.map(|l| format!(" :L{l}")).unwrap_or_default();
            links.push(format!(
                "{arrow} {}{approx}{line}",
                short_name(&qname, &path)
            ));
        }
    }
    if test_callers > 0 {
        links.push(format!("<- +{test_callers} testes"));
    }
    Ok(links)
}

/// Atributos tocados (nos símbolos listados ou por `ESCREVE`/`LE` deles) e quem os escreve.
fn writers_of_touched(store: &SqliteStore, ids: &[i64]) -> Result<Vec<WriterLine>, CoreError> {
    let mut attrs: BTreeMap<String, i64> = BTreeMap::new();
    let mut touch = |id: i64| -> Result<(), CoreError> {
        let addr = addr_by_id(store, id)?;
        if addr.kind == "attribute" {
            attrs.insert(addr.qname, id);
        }
        Ok(())
    };
    for &id in ids {
        touch(id)?;
        let mut stmt = store.conn.prepare(
            "SELECT dst FROM edge WHERE src = ?1 AND dst IS NOT NULL AND kind IN ('ESCREVE', 'LE')",
        )?;
        let dsts: Vec<i64> = stmt
            .query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for dst in dsts {
            touch(dst)?;
        }
    }
    let mut lines = Vec::new();
    for id in attrs.into_values() {
        let mut stmt = store.conn.prepare(
            "SELECT DISTINCT e.src, s.qname FROM edge e JOIN symbol s ON s.id = e.src
             WHERE e.dst = ?1 AND e.kind = 'ESCREVE' ORDER BY s.qname",
        )?;
        let writers: Vec<i64> = stmt
            .query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        if writers.is_empty() {
            continue;
        }
        lines.push(WriterLine {
            attr: addr_by_id(store, id)?.short,
            writers: writers
                .into_iter()
                .map(|w| addr_by_id(store, w).map(|a| a.short))
                .collect::<Result<_, _>>()?,
        });
    }
    Ok(lines)
}

/// `brief` sem notas.
/// Troca tokens que são alias de uma nota `glossario` (âncora `word:`) pela palavra.
fn expand_aliases(tokens: Vec<String>, notes: &[Note]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let word = notes
            .iter()
            .filter(|n| n.tipo == "glossario" && n.estado != "orfa" && n.estado != "substituida")
            .filter_map(|n| n.ancora.strip_prefix("word:").map(|w| (n, w)))
            .find(|(n, _)| {
                n.aliases
                    .iter()
                    .any(|a| tokenize(a).first() == Some(&token))
            })
            .map(|(_, w)| w.to_string());
        let token = word.unwrap_or(token);
        if !out.contains(&token) {
            out.push(token);
        }
    }
    out
}

pub fn brief(store: &SqliteStore, request: &str) -> Result<Brief, CoreError> {
    brief_with(store, request, &[])
}

/// `brief` com as notas visíveis: o alias de nota `glossario` casa tokens e as notas dos nós
/// listados entram na seção `notas`.
pub fn brief_with(store: &SqliteStore, request: &str, notes: &[Note]) -> Result<Brief, CoreError> {
    brief_weighted(store, request, notes, &BridgeWeights::default())
}

type Ranked = Vec<(i64, f64, SymbolAddr)>;

/// Arestas `CHAMA`, `RECEBE` e `RETORNA` resolvidas que tocam cada símbolo, nos dois sentidos.
fn link_degrees(store: &SqliteStore) -> Result<BTreeMap<i64, usize>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, COUNT(*) FROM (
             SELECT src AS id FROM edge WHERE dst IS NOT NULL AND kind IN ('CHAMA', 'RECEBE', 'RETORNA')
             UNION ALL
             SELECT dst AS id FROM edge WHERE dst IS NOT NULL AND kind IN ('CHAMA', 'RECEBE', 'RETORNA'))
         GROUP BY id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
    let mut degrees = BTreeMap::new();
    for row in rows {
        let (id, n) = row?;
        degrees.insert(id, n as usize);
    }
    Ok(degrees)
}

fn degree_of(degrees: &BTreeMap<i64, usize>, id: i64) -> f64 {
    degrees.get(&id).copied().unwrap_or(1).max(1) as f64
}

/// Ordena um nível (pontuação decrescente, depois nome) e separa produção de testes.
fn rank_tier(store: &SqliteStore, tier: Vec<(i64, f64)>) -> Result<(Ranked, Ranked), CoreError> {
    let mut ranked: Ranked = tier
        .into_iter()
        .map(|(id, s)| addr_by_id(store, id).map(|a| (id, s, a)))
        .collect::<Result<_, _>>()?;
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.2.qname.cmp(&b.2.qname)));
    Ok(ranked.into_iter().partition(|r| !r.2.in_tests()))
}

/// Preenche `total` vagas: o primeiro nível leva o que sobrar depois de `reserved` vagas para o
/// segundo; a vaga que um nível não usa passa ao outro.
fn pick_slots(first: Ranked, second: Ranked, total: usize, reserved: usize) -> Ranked {
    let take_first = first.len().min(total - second.len().min(reserved));
    let take_second = second.len().min(total - take_first);
    first
        .into_iter()
        .take(take_first)
        .chain(second.into_iter().take(take_second))
        .collect()
}

/// Como `brief_with`, com os pesos de ponte do repositório (`[bridges]` do `memo.toml`).
pub fn brief_weighted(
    store: &SqliteStore,
    request: &str,
    notes: &[Note],
    weights: &BridgeWeights,
) -> Result<Brief, CoreError> {
    brief_with_changes(store, request, notes, weights, &[])
}

/// Rank explicit identities first, then changed feature symbols, then lexical evidence.
pub fn brief_with_changes(
    store: &SqliteStore,
    request: &str,
    notes: &[Note],
    weights: &BridgeWeights,
    changed: &[String],
) -> Result<Brief, CoreError> {
    brief_full(
        store,
        request,
        notes,
        weights,
        &StrategyWeights::default(),
        changed,
        false,
    )
}

/// Como `brief_with_changes`; os relevantes de cada símbolo explícito entram logo depois dele,
/// com o peso de cada estratégia na etiqueta (`relevante: vizinhanca 1.00 · conexao 0.50`).
pub fn brief_full(
    store: &SqliteStore,
    request: &str,
    notes: &[Note],
    weights: &BridgeWeights,
    strategy_weights: &StrategyWeights,
    changed: &[String],
    summary: bool,
) -> Result<Brief, CoreError> {
    let structural = crate::structural::resolve(store, request)?;
    let tokens = expand_aliases(tokenize(&structural.remaining), notes);
    // Sem símbolo explícito, a interseção das palavras escolhe até 3 âncoras (estratégia 5).
    let has_symbol = structural.symbols.values().any(|k| *k == "simbolo");
    let anchors_found = if has_symbol {
        Vec::new()
    } else {
        crate::relevance::intersection_anchors(store, &tokens)?
    };
    let anchored: BTreeMap<i64, String> = anchors_found
        .iter()
        .map(|a| (a.id, format!("ancora {}/{}", a.covered, a.total)))
        .collect();
    let anchor_rank: BTreeMap<i64, usize> = anchors_found
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id, i))
        .collect();
    let mut relevant: BTreeMap<i64, crate::relevance::Relevant> = BTreeMap::new();
    let roots: Vec<i64> = structural
        .symbols
        .iter()
        .filter(|(_, k)| **k == "simbolo")
        .map(|(id, _)| *id)
        .chain(anchored.keys().copied())
        .collect();
    for id in roots {
        for item in crate::relevance::relevant(store, id, strategy_weights)? {
            let rid = crate::query::target_id_of(store, &item.addr.qname)?;
            if structural.symbols.contains_key(&rid) || anchored.contains_key(&rid) {
                continue;
            }
            match relevant.get(&rid) {
                Some(kept) if kept.score >= item.score => {}
                _ => {
                    relevant.insert(rid, item);
                }
            }
        }
    }
    let changed_ids = crate::structural::changed_ids(store, changed)?;
    let matched = match_words(store, &tokens)?;
    let mut ignored: Vec<String> = Vec::new();
    let mut usable: Vec<&Candidate> = Vec::new();
    for c in &matched {
        if c.skeleton {
            ignored.push(c.text.clone());
        } else {
            usable.push(c);
        }
    }
    usable.sort_by(|a, b| {
        a.approx.cmp(&b.approx).then_with(|| {
            b.weight
                .total_cmp(&a.weight)
                .then_with(|| a.text.cmp(&b.text))
        })
    });
    usable.truncate(MAX_ANCHORS);
    let top = usable.first().map_or(0.0, |c| c.weight);
    let normalized = |w: f64| if top > 0.0 { w / top } else { 1.0 };
    let anchors: Vec<Anchor> = usable
        .iter()
        .map(|c| Anchor {
            word: c.text.clone(),
            score: normalized(c.weight),
            approx: c.approx,
        })
        .collect();

    let mut scores: BTreeMap<i64, f64> = BTreeMap::new();
    let mut named = BTreeSet::new();
    for id in structural
        .symbols
        .keys()
        .chain(&changed_ids)
        .chain(anchored.keys())
    {
        scores.insert(*id, 1.0);
    }
    for (id, item) in &relevant {
        let entry = scores.entry(*id).or_insert(0.0);
        *entry = entry.max(item.score);
    }
    for c in &usable {
        let mut stmt = store.conn.prepare(
            "SELECT symbol_id, slot FROM occurrence WHERE word_id = ?1 AND symbol_id IS NOT NULL",
        )?;
        let rows = stmt.query_map([c.id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (symbol, slot) = row?;
            named.insert(symbol);
            *scores.entry(symbol).or_insert(0.0) += normalized(c.weight) * slot_bonus(&slot);
        }
    }
    for c in &usable {
        let share = normalized(c.weight);
        // Docstring de teste com a âncora: o teste sobe pelo peso da ponte de teste.
        let docs: Vec<i64> = store
            .conn
            .prepare(
                "SELECT owner_id FROM occurrence
                 WHERE word_id = ?1 AND owner_kind = 'symbol' AND source = 'test_doc'",
            )?
            .query_map([c.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for symbol in docs {
            *scores.entry(symbol).or_insert(0.0) += share * weights.test;
        }
        // Módulo cujo arquivo importa a âncora: sinal fraco, pelo peso da ponte de import.
        let modules: Vec<i64> = store
            .conn
            .prepare(
                "SELECT DISTINCT s.id FROM occurrence o
                 JOIN import i ON o.owner_kind = 'import' AND o.owner_id = i.rowid
                 JOIN symbol s ON s.file_id = i.file_id AND s.kind = 'module'
                 WHERE o.word_id = ?1 ORDER BY s.id",
            )?
            .query_map([c.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for symbol in modules {
            *scores.entry(symbol).or_insert(0.0) += share * weights.import * WEAK_SIGNAL;
        }
    }
    // D1: quem carrega a âncora (pontuação direta) vem antes de quem só chega por salto; o salto
    // é amortecido pelo grau do vizinho (D5), para um hub não encabeçar a lista.
    let direct = scores;
    let mut hop: BTreeMap<i64, f64> = BTreeMap::new();
    for (&id, &score) in &direct {
        for n in neighbors(store, id)? {
            *hop.entry(n).or_insert(0.0) += HOP_FACTOR * score;
        }
    }
    let degrees = link_degrees(store)?;
    let mut first: Vec<(i64, f64)> = Vec::new();
    let mut second: Vec<(i64, f64)> = Vec::new();
    for (&id, &score) in &direct {
        let damped = hop.get(&id).map_or(0.0, |h| h / degree_of(&degrees, id));
        first.push((id, score + damped));
    }
    for (&id, &h) in &hop {
        if !direct.contains_key(&id) {
            second.push((id, h / degree_of(&degrees, id)));
        }
    }
    let (mut first_production, mut first_tests) = rank_tier(store, first)?;
    // Âncoras na ordem da interseção; o resto, na ordem da pontuação (ordenação estável).
    let priority = |id: i64| match structural.symbols.get(&id) {
        Some(&"simbolo") => (0, 0),
        _ if anchored.contains_key(&id) => (1, anchor_rank[&id]),
        _ if relevant.contains_key(&id) => (2, 0),
        Some(_) => (3, 0),
        None if changed_ids.contains(&id) => (4, 0),
        None => (5, 0),
    };
    for tier in [&mut first_production, &mut first_tests] {
        // Estável: dentro de cada grupo, a pontuação já ordenou (relevantes pela soma dos pesos).
        tier.sort_by_key(|r| (priority(r.0), !named.contains(&r.0), r.2.kind == "module"));
    }
    let (second_production, second_tests) = rank_tier(store, second)?;
    // Produção primeiro: os testes não disputam as vagas do código (D7), têm as suas.
    let mut ranked = pick_slots(
        first_production,
        second_production,
        MAX_SYMBOLS,
        NEIGHBOR_SLOTS,
    );
    ranked.extend(pick_slots(first_tests, second_tests, MAX_TEST_SYMBOLS, 0));

    let mut outline = Vec::new();
    for (id, _, addr) in ranked
        .iter()
        .filter(|r| !r.2.in_tests())
        .take(OUTLINE_SYMBOLS)
    {
        let links = outline_for(store, *id)?;
        if !links.is_empty() {
            outline.push(Outline {
                symbol: addr.short.clone(),
                links,
            });
        }
    }
    let ids: Vec<i64> = ranked.iter().map(|(id, _, _)| *id).collect();
    let ranked_addrs: Vec<SymbolAddr> = ranked.iter().map(|(_, _, a)| a.clone()).collect();
    let pattern = match anchors.first() {
        Some(a) => {
            let info = word_pattern(store, &a.word)?;
            Some(Pattern {
                word: a.word.clone(),
                verbs: info.verbs,
                roles: info.roles,
                layers: info.layers,
            })
        }
        None => None,
    };
    let anchor_ids: Vec<i64> = usable.iter().map(|c| c.id).collect();
    let mut tagged = Vec::with_capacity(ranked.len());
    for (id, score, addr) in ranked {
        let mut tags: Vec<String> = bridge_tags(&store.conn, id, &anchor_ids)?
            .into_iter()
            .map(str::to_string)
            .collect();
        if let Some(tag) = structural.symbols.get(&id) {
            tags.insert(0, (*tag).to_string());
        }
        if let Some(tag) = anchored.get(&id) {
            tags.insert(0, tag.clone());
        }
        if changed_ids.contains(&id) {
            tags.insert(0, "alterado".to_string());
        }
        if let Some(item) = relevant.get(&id) {
            let weights: Vec<String> = item
                .strategies
                .iter()
                .map(|w| format!("{} {:.2}", w.strategy, w.weight))
                .collect();
            tags.insert(0, format!("relevante: {}", weights.join(" · ")));
        }
        let in_tests = addr.in_tests();
        tagged.push(ScoredSymbol {
            addr,
            score,
            tags,
            in_tests,
        });
    }
    let notes_lines = related_notes(notes, &ranked_addrs, &anchors, summary);
    let ler = brief_read_spots(store, &tagged, &anchors)?;
    Ok(Brief {
        request: request.to_string(),
        anchors,
        ignored,
        writers: writers_of_touched(store, &ids)?,
        symbols: tagged,
        outline,
        pattern,
        notes: notes_lines,
        unchanged: None,
        ler,
    })
}

/// Símbolos de produção lidos como trecho no `brief`.
const READ_SYMBOLS: usize = 5;

/// `ler:` do `brief`: os 5 primeiros símbolos de produção da lista; o motivo diz quais palavras
/// do pedido o nome carrega, ou `vizinho` quando chegou só por vizinhança.
fn brief_read_spots(
    store: &SqliteStore,
    symbols: &[ScoredSymbol],
    anchors: &[Anchor],
) -> Result<Vec<ReadSpot>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
         JOIN symbol s ON o.owner_kind = 'symbol' AND o.owner_id = s.id
         WHERE s.qname = ?1 AND o.source = 'name' ORDER BY o.pos",
    )?;
    let mut spots: Vec<ReadSpot> = Vec::new();
    for s in symbols.iter().filter(|s| !s.in_tests) {
        let names: Vec<String> = stmt
            .query_map([&s.addr.qname], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut carried: Vec<&str> = anchors
            .iter()
            .map(|a| a.word.as_str())
            .filter(|w| names.iter().any(|n| n == w))
            .collect();
        carried.dedup();
        let motivo = if s.tags.iter().any(|t| t == "simbolo") {
            "simbolo explicito".to_string()
        } else if s.tags.iter().any(|t| t.starts_with("ancora ")) {
            "ancora do pedido".to_string()
        } else if s.tags.iter().any(|t| t.starts_with("relevante: ")) {
            "relevante ao simbolo".to_string()
        } else if s.tags.iter().any(|t| t == "arquivo") {
            "arquivo explicito".to_string()
        } else if s.tags.iter().any(|t| t == "alterado") {
            "alterado na feature".to_string()
        } else if carried.is_empty() {
            "vizinho".to_string()
        } else {
            format!("carrega {}", carried.join(", "))
        };
        let spot = structural_read_spot(store, &s.addr, None, motivo)?;
        if spots
            .iter()
            .any(|seen| seen.path == spot.path && seen.start <= spot.end && spot.start <= seen.end)
        {
            continue;
        }
        spots.push(spot);
        if spots.len() == READ_SYMBOLS {
            break;
        }
    }
    Ok(spots)
}

/// Notas dos nós listados e das palavras âncora, uma linha cada; `summary` corta o texto em 120 caracteres.
fn related_notes(
    notes: &[Note],
    symbols: &[SymbolAddr],
    anchors: &[Anchor],
    summary: bool,
) -> Vec<String> {
    let qnames: Vec<String> = symbols.iter().map(|s| s.qname.clone()).collect();
    let files: Vec<String> = symbols.iter().map(|s| s.path.clone()).collect();
    let words: Vec<String> = anchors.iter().map(|a| a.word.clone()).collect();
    notes_for(notes, &qnames, &words, &files)
        .iter()
        .map(|n| if summary { n.summary() } else { n.full_line() })
        .collect()
}

/// Remove do `Brief` o que já foi entregue à `task`, registra o que ficou e preenche
/// `unchanged`; com `full` devolve tudo, e só registra.
pub fn apply_delta(
    store: &SqliteStore,
    task: &str,
    brief: &mut Brief,
    full: bool,
) -> Result<(), CoreError> {
    let request = brief.request.clone();
    let original = brief.symbols.clone();
    let fingerprints: BTreeMap<String, (String, String)> = store
        .conn
        .prepare("SELECT qname, sig_hash, body_hash FROM symbol")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    let (symbols, a) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.symbols),
        |s| {
            let fingerprint = fingerprints.get(&s.addr.qname);
            format!(
                "{request}|S|{}|{}:{}-{}|{fingerprint:?}",
                s.addr.qname, s.addr.path, s.addr.line_start, s.addr.line_end,
            )
        },
        full,
    )?;
    // `ler:` acompanha os símbolos: só ficam os trechos de símbolos ainda listados (TASK-7, D4).
    brief.ler.retain(|spot| {
        let owner = original
            .iter()
            .filter(|s| {
                s.addr.path == spot.path
                    && s.addr.line_start <= spot.start
                    && s.addr.line_end >= spot.end
            })
            .min_by_key(|s| s.addr.line_end - s.addr.line_start);
        owner.is_some_and(|s| symbols.iter().any(|kept| kept.addr.qname == s.addr.qname))
    });
    let (outline, b) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.outline),
        |o| format!("{request}|O|{}|{}", o.symbol, o.links.join(";")),
        full,
    )?;
    let (writers, c) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.writers),
        |w| format!("{request}|W|{}|{}", w.attr, w.writers.join(",")),
        full,
    )?;
    let (notes, d) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.notes),
        |n| format!("{request}|N|{n}"),
        full,
    )?;
    brief.notes = notes;
    brief.symbols = symbols;
    brief.outline = outline;
    brief.writers = writers;
    brief.unchanged = Some(a + b + c + d);
    Ok(())
}
