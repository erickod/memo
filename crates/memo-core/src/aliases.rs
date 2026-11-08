//! Sugestões de aliases entre palavras (somente leitura): pares que provavelmente nomeiam o mesmo
//! conceito, para o humano aprovar com uma nota `glossario`. Nada é gravado (D17, D21).
//!
//! Dois motivos, nesta ordem de apresentação: `prefixo` (prefixo comum de pelo menos 5 caracteres
//! e até 3 caracteres de resto em cada palavra) e `vizinhança` (Jaccard das vizinhas de pelo menos
//! 0,6, com pelo menos 3 vizinhas em cada palavra). As vizinhas são as do lote de pontes de
//! símbolo, teste, import e chamada. Palavras `esqueleto` ficam de fora, e duas palavras que
//! aparecem juntas (vizinhas diretas) não são sugeridas por vizinhança: colaboram, não são
//! sinônimos.

use crate::bridges::core_neighborhoods;
use crate::config::BridgeWeights;
use crate::error::CoreError;
use crate::notes::Note;
use crate::store::SqliteStore;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};

const MIN_PREFIX: usize = 5;
const MAX_REST: usize = 3;
const MIN_JACCARD: f64 = 0.6;
const MIN_NEIGHBORS: usize = 3;
/// Vizinha compartilhada por mais palavras que isto não serve para achar pares (evita o
/// quadrado de uma vizinha onipresente); o Jaccard final é calculado sobre as vizinhas inteiras.
const MAX_POSTING: usize = 50;
const MIN_SHARED: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AliasPair {
    pub first: String,
    pub second: String,
    /// `prefixo` ou `vizinhança`.
    pub reason: &'static str,
    pub similarity: f64,
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

fn prefix_pair(a: &str, b: &str) -> Option<f64> {
    let prefix = common_prefix(a, b);
    let (len_a, len_b) = (a.chars().count(), b.chars().count());
    let rest = (len_a - prefix).max(len_b - prefix);
    (prefix >= MIN_PREFIX && rest <= MAX_REST).then(|| prefix as f64 / len_a.max(len_b) as f64)
}

/// Pares `(a, b)` já cobertos por uma nota `glossario` ativa (palavra de âncora e alias).
fn covered(glossary: &[Note]) -> HashSet<(String, String)> {
    let mut set = HashSet::new();
    for note in glossary
        .iter()
        .filter(|n| n.tipo == "glossario" && n.estado != "orfa" && n.estado != "substituida")
    {
        let Some(word) = note.ancora.strip_prefix("word:") else {
            continue;
        };
        for alias in &note.aliases {
            let alias = alias.to_lowercase();
            set.insert((word.to_string(), alias.clone()));
            set.insert((alias, word.to_string()));
        }
    }
    set
}

fn jaccard(a: &BTreeSet<i64>, b: &BTreeSet<i64>, x: i64, y: i64) -> f64 {
    let a: BTreeSet<i64> = a.iter().copied().filter(|n| *n != y).collect();
    let b: BTreeSet<i64> = b.iter().copied().filter(|n| *n != x).collect();
    if a.len() < MIN_NEIGHBORS || b.len() < MIN_NEIGHBORS {
        return 0.0;
    }
    let shared = a.intersection(&b).count();
    shared as f64 / (a.len() + b.len() - shared) as f64
}

/// Pares sugeridos, sem os que uma nota `glossario` ativa já cobre. Ordem: motivo (prefixo antes
/// de vizinhança), semelhança decrescente, ordem alfabética.
pub fn suggest(
    store: &SqliteStore,
    weights: &BridgeWeights,
    glossary: &[Note],
) -> Result<Vec<AliasPair>, CoreError> {
    let words: Vec<(i64, String)> = store
        .conn
        .prepare(
            "SELECT w.id, w.text FROM word w JOIN word_stats s ON s.word_id = w.id
             WHERE s.esqueleto = 0 ORDER BY w.text",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let text_of: BTreeMap<i64, &str> = words.iter().map(|(id, t)| (*id, t.as_str())).collect();
    let done = covered(glossary);
    let mut pairs: BTreeMap<(String, String), AliasPair> = BTreeMap::new();
    let mut push = |first: &str, second: &str, reason: &'static str, similarity: f64| {
        let key = (first.to_string(), second.to_string());
        if done.contains(&key) || pairs.contains_key(&key) {
            return;
        }
        pairs.insert(
            key,
            AliasPair {
                first: first.to_string(),
                second: second.to_string(),
                reason,
                similarity,
            },
        );
    };

    // Prefixo: agrupa pelas 5 primeiras letras e compara dentro do grupo.
    let mut groups: BTreeMap<String, Vec<&(i64, String)>> = BTreeMap::new();
    for word in &words {
        if word.1.chars().count() >= MIN_PREFIX {
            groups
                .entry(word.1.chars().take(MIN_PREFIX).collect())
                .or_default()
                .push(word);
        }
    }
    for group in groups.values() {
        for (i, a) in group.iter().enumerate() {
            for b in &group[i + 1..] {
                if let Some(similarity) = prefix_pair(&a.1, &b.1) {
                    push(&a.1, &b.1, "prefixo", similarity);
                }
            }
        }
    }

    // Vizinhança: pares que dividem vizinhas raras, depois o Jaccard sobre as vizinhas inteiras.
    let neighborhoods = core_neighborhoods(&store.conn, weights)?;
    let sets: BTreeMap<i64, BTreeSet<i64>> = neighborhoods
        .iter()
        .filter(|(id, _)| text_of.contains_key(id))
        .map(|(id, row)| (*id, row.keys().copied().collect()))
        .filter(|(_, set): &(i64, BTreeSet<i64>)| set.len() >= MIN_NEIGHBORS)
        .collect();
    let mut postings: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for (word, set) in &sets {
        for neighbor in set {
            postings.entry(*neighbor).or_default().push(*word);
        }
    }
    let mut shared: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for list in postings.values().filter(|l| l.len() <= MAX_POSTING) {
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                *shared.entry((*a, *b)).or_insert(0) += 1;
            }
        }
    }
    for ((a, b), count) in shared {
        if count < MIN_SHARED {
            continue;
        }
        // Palavras que aparecem juntas (vizinhas diretas) colaboram, não são sinônimos.
        if sets[&a].contains(&b) {
            continue;
        }
        let similarity = jaccard(&sets[&a], &sets[&b], a, b);
        if similarity >= MIN_JACCARD {
            let (ta, tb) = (text_of[&a], text_of[&b]);
            let (first, second) = if ta <= tb { (ta, tb) } else { (tb, ta) };
            push(first, second, "vizinhança", similarity);
        }
    }

    let mut out: Vec<AliasPair> = pairs.into_values().collect();
    out.sort_by(|a, b| {
        (a.reason != "prefixo")
            .cmp(&(b.reason != "prefixo"))
            .then_with(|| b.similarity.total_cmp(&a.similarity))
            .then_with(|| a.first.cmp(&b.first))
            .then_with(|| a.second.cmp(&b.second))
    });
    Ok(out)
}
