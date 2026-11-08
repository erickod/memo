//! Pontes entre palavras: o que liga duas palavras por símbolo, teste, import, chamada, arquivo
//! ou contexto. São derivadas de `occurrence`, `edge`, `import` e `file`, sem tabela própria (D19).

use crate::config::BridgeWeights;
use crate::error::CoreError;
use crate::store::SqliteStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BridgeKind {
    Symbol,
    Test,
    Import,
    Call,
    File,
    Context,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bridge {
    pub other: String,
    pub kind: BridgeKind,
    pub weight: f64,
}

/// Vizinhas de uma palavra por tipo de ponte, da mais pesada para a mais leve.
pub fn word_bridges(
    _store: &SqliteStore,
    _word: &str,
    _weights: &BridgeWeights,
) -> Result<Vec<Bridge>, CoreError> {
    todo!("etapa 3")
}
