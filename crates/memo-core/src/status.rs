//! Resumo de base, features, notas e tasks promovíveis (spec §8.2, `status`).

use crate::error::CoreError;
use crate::notes::{NoteDir, is_generic_anchor};
use crate::store::SqliteStore;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

const STATES: [&str; 5] = ["proposta", "confirmada", "stale", "orfa", "substituida"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphInfo {
    pub files: i64,
    pub symbols: i64,
    pub indexed_ref: Option<String>,
    pub indexed_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub base: Option<GraphInfo>,
    pub features: Vec<(String, GraphInfo)>,
    /// Notas por estado.
    pub notes: BTreeMap<String, usize>,
    /// Tasks cujo branch já foi mergeado na principal.
    pub promotable: Vec<String>,
    /// Notas ativas ancoradas numa palavra que virou `esqueleto` (marca `âncora genérica`).
    pub generic_anchors: Vec<String>,
}

fn info(path: &Path) -> Result<GraphInfo, CoreError> {
    let store = SqliteStore::open(path)?;
    let (files, symbols) = store.counts()?;
    Ok(GraphInfo {
        files,
        symbols,
        indexed_ref: store.get_meta("indexed_ref")?,
        indexed_commit: store.get_meta("indexed_commit")?,
    })
}

/// Lê `<memo_dir>/base.db`, `features/*.db` e `notes/`; `merged` são os branches mergeados.
pub fn status(memo_dir: &Path, merged: &[String]) -> Result<Status, CoreError> {
    let base_path = memo_dir.join("base.db");
    let base = base_path.exists().then(|| info(&base_path)).transpose()?;
    let mut features = Vec::new();
    if let Ok(entries) = std::fs::read_dir(memo_dir.join("features")) {
        let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let task = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".db"));
            if let Some(task) = task {
                features.push((task.to_string(), info(&path)?));
            }
        }
    }
    let mut notes: BTreeMap<String, usize> = STATES.iter().map(|s| ((*s).to_string(), 0)).collect();
    for note in NoteDir::new(&memo_dir.join("notes")).list()? {
        *notes.entry(note.estado).or_insert(0) += 1;
    }
    let mut generic_anchors = Vec::new();
    if base.is_some() {
        let graph = SqliteStore::open(&base_path)?;
        for note in NoteDir::new(&memo_dir.join("notes")).list()? {
            if is_generic_anchor(&graph, &note)? {
                generic_anchors.push(note.id);
            }
        }
    }
    let promotable = features
        .iter()
        .map(|(task, _)| task.clone())
        .filter(|task| merged.contains(task))
        .collect();
    Ok(Status {
        base,
        features,
        notes,
        promotable,
        generic_anchors,
    })
}
