//! Sugestões de aliases entre palavras (somente leitura): pares que provavelmente nomeiam o mesmo
//! conceito, para o humano aprovar com uma nota `glossario`. Nada é gravado (D17, D21).

use crate::config::BridgeWeights;
use crate::error::CoreError;
use crate::notes::Note;
use crate::store::SqliteStore;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AliasPair {
    pub first: String,
    pub second: String,
    /// `prefixo` ou `vizinhança`.
    pub reason: &'static str,
    pub similarity: f64,
}

/// Pares sugeridos, sem os que uma nota `glossario` ativa já cobre.
pub fn suggest(
    _store: &SqliteStore,
    _weights: &BridgeWeights,
    _glossary: &[Note],
) -> Result<Vec<AliasPair>, CoreError> {
    todo!("etapa 9")
}
