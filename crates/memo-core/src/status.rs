//! Resumo de base, features, notas e tasks promovíveis (spec §8.2, `status`).

use crate::error::CoreError;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

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
}

/// Lê `<memo_dir>/base.db`, `features/*.db` e `notes/`; `merged` são os branches mergeados.
pub fn status(_memo_dir: &Path, _merged: &[String]) -> Result<Status, CoreError> {
    todo!("spec §8.2 status")
}
