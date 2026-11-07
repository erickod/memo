//! Deltas por task: só o que é novo ou mudou desde a última chamada (spec §8.4).

use crate::error::CoreError;
use crate::store::SqliteStore;

/// Separa os itens ainda não vistos pela `task` para `cmd`; registra os que ficaram.
/// Com `full`, devolve todos e apenas registra. Devolve também quantos foram omitidos.
pub fn filter_new<T>(
    _store: &SqliteStore,
    _task: &str,
    _cmd: &str,
    _items: Vec<T>,
    _key: impl Fn(&T) -> String,
    _full: bool,
) -> Result<(Vec<T>, usize), CoreError> {
    todo!("spec §8.4")
}
