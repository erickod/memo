//! Consultas sobre o grafo (spec §8.2).

use crate::error::CoreError;
use crate::store::SqliteStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolAddr {
    pub qname: String,
    pub kind: String,
    pub path: String,
    pub line_start: i64,
    pub line_end: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInfo {
    pub addr: SymbolAddr,
    pub sig_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordInfo {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
    pub symbols: Vec<SymbolAddr>,
    pub neighbors: Vec<String>,
}

/// Aceita `qname` completo ou sufixo único; sufixo ambíguo devolve `Ambiguous`.
pub fn symbol_info(_store: &SqliteStore, _target: &str) -> Result<SymbolInfo, CoreError> {
    todo!("spec §8.2 query symbol")
}

pub fn word_info(_store: &SqliteStore, _word: &str) -> Result<WordInfo, CoreError> {
    todo!("spec §8.2 query word")
}
