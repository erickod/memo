//! Representatividade de uma palavra em arquivos: frente ao repositório, a cada contexto e a cada
//! vizinha (TASK-3, D7). Derivada na consulta; não decide nada, só informa.

use crate::error::CoreError;
use crate::store::SqliteStore;
use serde::Serialize;

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

pub fn representativeness(
    _store: &SqliteStore,
    _word_id: i64,
) -> Result<Representativeness, CoreError> {
    Ok(Representativeness::default())
}
