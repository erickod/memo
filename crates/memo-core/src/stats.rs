//! Pesos das palavras, recalculados a cada indexação que escreve (spec §5.6).

use crate::config::Config;
use crate::error::CoreError;
use rusqlite::{Transaction, params};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const DAMPING: f64 = 0.85;
const ITERATIONS: usize = 20;

#[derive(Default)]
struct Raw {
    uso: i64,
    files: BTreeSet<i64>,
    symbols: i64,
    neighbors: BTreeSet<i64>,
}

/// PageRank no grafo de co-ocorrência (peso = nº de símbolos em comum), sem palavras esqueleto.
fn pagerank(nodes: &[i64], weights: &BTreeMap<(i64, i64), f64>) -> BTreeMap<i64, f64> {
    let n = nodes.len();
    if n == 0 {
        return BTreeMap::new();
    }
    let mut out_weight: BTreeMap<i64, f64> = nodes.iter().map(|&w| (w, 0.0)).collect();
    for (&(a, _), &w) in weights {
        *out_weight.entry(a).or_insert(0.0) += w;
    }
    let base = 1.0 / n as f64;
    let mut rank: BTreeMap<i64, f64> = nodes.iter().map(|&w| (w, base)).collect();
    for _ in 0..ITERATIONS {
        let dangling: f64 = nodes
            .iter()
            .filter(|w| out_weight[*w] == 0.0)
            .map(|w| rank[w])
            .sum();
        let mut next: BTreeMap<i64, f64> = nodes
            .iter()
            .map(|&w| (w, (1.0 - DAMPING) * base + DAMPING * dangling * base))
            .collect();
        for (&(a, b), &w) in weights {
            let share = rank[&a] * w / out_weight[&a];
            *next.entry(b).or_insert(0.0) += DAMPING * share;
        }
        rank = next;
    }
    rank
}

/// Recalcula `word_stats`: uso, alcance, grau, idf, especificidade, centralidade, esqueleto.
///
/// `N_símbolos` é o total de símbolos do grafo.
pub fn recompute(tx: &Transaction, config: &Config) -> Result<(), CoreError> {
    tx.execute("DELETE FROM word_stats", [])?;
    let n_symbols: i64 = tx.query_row("SELECT COUNT(*) FROM symbol", [], |r| r.get(0))?;

    let mut by_symbol: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    let mut raw: BTreeMap<i64, Raw> = BTreeMap::new();
    let mut stmt = tx.prepare(
        "SELECT o.symbol_id, o.word_id, s.file_id FROM occurrence o
         JOIN symbol s ON s.id = o.symbol_id ORDER BY o.symbol_id, o.pos",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (symbol, word, file) = row?;
        let entry = raw.entry(word).or_default();
        entry.uso += 1;
        entry.files.insert(file);
        by_symbol.entry(symbol).or_default().insert(word);
    }
    for words in by_symbol.values() {
        for &w in words {
            let entry = raw.entry(w).or_default();
            entry.symbols += 1;
            entry
                .neighbors
                .extend(words.iter().copied().filter(|&o| o != w));
        }
    }

    let mut texts: BTreeMap<i64, String> = BTreeMap::new();
    let mut stmt = tx.prepare("SELECT id, text FROM word")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (id, text) = row?;
        texts.insert(id, text);
    }
    let skeleton: HashSet<i64> = texts
        .iter()
        .filter(|(_, t)| config.skeleton.contains(t))
        .map(|(&id, _)| id)
        .collect();

    let idf: BTreeMap<i64, f64> = raw
        .iter()
        .map(|(&w, r)| (w, (n_symbols as f64 / (1.0 + r.symbols as f64)).ln()))
        .collect();
    let (lo, hi) = idf
        .values()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));

    let mut weights: BTreeMap<(i64, i64), f64> = BTreeMap::new();
    for words in by_symbol.values() {
        let kept: Vec<i64> = words
            .iter()
            .copied()
            .filter(|w| !skeleton.contains(w))
            .collect();
        for &a in &kept {
            for &b in &kept {
                if a != b {
                    *weights.entry((a, b)).or_insert(0.0) += 1.0;
                }
            }
        }
    }
    let nodes: Vec<i64> = raw
        .keys()
        .copied()
        .filter(|w| !skeleton.contains(w))
        .collect();
    let central = pagerank(&nodes, &weights);

    for (word, r) in &raw {
        let idf_w = idf[word];
        let especificidade = if hi > lo {
            (idf_w - lo) / (hi - lo)
        } else {
            1.0
        };
        let is_skeleton = skeleton.contains(word) || especificidade < config.min_specificity;
        tx.execute(
            "INSERT INTO word_stats(word_id, uso, alcance, grau, idf, especificidade, centralidade, esqueleto)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                word,
                r.uso,
                r.files.len() as i64,
                r.neighbors.len() as i64,
                idf_w,
                especificidade,
                central.get(word).copied().unwrap_or(0.0),
                is_skeleton
            ],
        )?;
    }
    Ok(())
}
