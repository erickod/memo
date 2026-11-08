//! Representatividade de uma palavra em arquivos: frente ao repositório, a cada contexto e a cada
//! vizinha (TASK-3, D7). Derivada na consulta; não decide nada, só informa.

use crate::error::CoreError;
use crate::store::SqliteStore;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Piso de arquivos em comum para uma vizinha entrar na ficha.
pub const MIN_SHARED_FILES: usize = 3;
/// Vizinhas mostradas (o mesmo teto da coerência).
pub const MAX_NEIGHBORS: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Share {
    pub files: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextShare {
    pub context: String,
    pub files: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NeighborShare {
    pub word: String,
    pub shared: usize,
    /// `P(palavra | vizinha)`: dos arquivos da vizinha, a fração que tem a palavra.
    pub word_given_neighbor: f64,
    /// `P(vizinha | palavra)`: dos arquivos da palavra, a fração que tem a vizinha.
    pub neighbor_given_word: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Representativeness {
    pub repository: Share,
    pub contexts: Vec<ContextShare>,
    pub neighbors: Vec<NeighborShare>,
}

/// Quantos arquivos têm a palavra, e como isso se divide por contexto e por vizinha. A unidade é
/// o arquivo: em símbolos, duas palavras raramente dividem o mesmo nome. Só nomes de símbolos
/// contam; um arquivo sem contexto entra no repositório e em nenhum contexto.
pub fn representativeness(
    store: &SqliteStore,
    word_id: i64,
) -> Result<Representativeness, CoreError> {
    let conn = &store.conn;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM file", [], |r| r.get(0))?;
    let mut by_word: HashMap<i64, BTreeSet<i64>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT s.file_id, o.word_id FROM occurrence o JOIN symbol s ON s.id = o.owner_id
         WHERE o.owner_kind = 'symbol' AND o.source = 'name' AND s.file_id IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (file, word) = row?;
        by_word.entry(word).or_default().insert(file);
    }
    let empty = BTreeSet::new();
    let files = by_word.get(&word_id).unwrap_or(&empty);

    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    let mut with_word: BTreeMap<String, usize> = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT id, context FROM file WHERE context IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (file, context) = row?;
        *sizes.entry(context.clone()).or_insert(0) += 1;
        if files.contains(&file) {
            *with_word.entry(context).or_insert(0) += 1;
        }
    }
    let mut contexts: Vec<ContextShare> = with_word
        .into_iter()
        .map(|(context, n)| ContextShare {
            total: sizes[&context],
            files: n,
            context,
        })
        .collect();
    contexts.sort_by(|a, b| {
        let (ra, rb) = (a.files * b.total, b.files * a.total);
        rb.cmp(&ra).then_with(|| a.context.cmp(&b.context))
    });

    let mut shared: HashMap<i64, usize> = HashMap::new();
    for (word, other_files) in &by_word {
        if *word != word_id {
            let n = files.intersection(other_files).count();
            if n >= MIN_SHARED_FILES {
                shared.insert(*word, n);
            }
        }
    }
    let texts: HashMap<i64, String> = conn
        .prepare("SELECT id, text FROM word")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut ranked: Vec<(&String, i64, usize)> =
        shared.iter().map(|(id, n)| (&texts[id], *id, *n)).collect();
    ranked.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
    let neighbors = ranked
        .into_iter()
        .take(MAX_NEIGHBORS)
        .map(|(text, id, n)| NeighborShare {
            word: text.clone(),
            shared: n,
            word_given_neighbor: n as f64 / by_word[&id].len() as f64,
            neighbor_given_word: n as f64 / files.len() as f64,
        })
        .collect();
    Ok(Representativeness {
        repository: Share {
            files: files.len(),
            total: total as usize,
        },
        contexts,
        neighbors,
    })
}
