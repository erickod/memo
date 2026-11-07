//! Persistência do grafo em SQLite (spec §4).

use crate::error::CoreError;
use std::path::Path;

pub struct SqliteStore;

impl SqliteStore {
    pub fn open(_path: &Path) -> Result<Self, CoreError> {
        todo!("spec §4")
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        todo!("spec §4")
    }

    /// Total de linhas alteradas na conexão; não muda quando nada é escrito.
    pub fn total_changes(&self) -> u64 {
        todo!("spec §4")
    }

    pub fn event_count(&self) -> Result<u64, CoreError> {
        todo!("spec §4")
    }

    pub fn symbol_exists(&self, _qname: &str) -> Result<bool, CoreError> {
        todo!("spec §4")
    }

    /// Texto canônico do grafo (sem ids), usado para comparar bytes entre execuções.
    pub fn dump(&self) -> Result<String, CoreError> {
        todo!("spec §4")
    }
}
