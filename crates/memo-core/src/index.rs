//! Indexação incremental (spec §6.3).

use crate::error::CoreError;
use crate::source::SourceTree;
use crate::store::SqliteStore;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub files_total: usize,
    pub files_changed: usize,
    pub files_removed: usize,
    pub files_with_error: Vec<String>,
    pub wrote: bool,
}

pub fn index(_tree: &dyn SourceTree, _store: &mut SqliteStore) -> Result<IndexReport, CoreError> {
    todo!("spec §6.3")
}
