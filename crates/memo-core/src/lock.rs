//! Lock exclusivo por banco, com espera limitada (spec §6, D6 e D10 da TASK).

use crate::error::CoreError;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

/// Espera padrão pelo lock: 30 s (D10); `MEMO_LOCK_TIMEOUT_MS` a substitui.
pub fn default_timeout() -> Duration {
    todo!("D10")
}

/// Guarda do lock; solta ao ser descartada.
pub struct DbLock {
    _file: File,
}

impl DbLock {
    /// Espera até `timeout` pelo lock de `path`; estourar devolve `CoreError::Locked`.
    pub fn acquire(_path: &Path, _timeout: Duration) -> Result<Self, CoreError> {
        todo!("D6")
    }
}
