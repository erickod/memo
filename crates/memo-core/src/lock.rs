//! Lock exclusivo por banco, com espera limitada (spec §6, D6 e D10 da TASK).

use crate::error::CoreError;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(20);

/// Espera padrão pelo lock: 30 s (D10); `MEMO_LOCK_TIMEOUT_MS` a substitui.
pub fn default_timeout() -> Duration {
    std::env::var("MEMO_LOCK_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
}

/// Guarda do lock; solta ao ser descartada.
pub struct DbLock {
    _file: File,
}

impl DbLock {
    /// Espera até `timeout` pelo lock de `path`; estourar devolve `CoreError::Locked`.
    pub fn acquire(path: &Path, timeout: Duration) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout {
                        return Err(CoreError::Locked(format!(
                            "{} is busy (waited {} ms)",
                            path.display(),
                            timeout.as_millis()
                        )));
                    }
                    std::thread::sleep(POLL);
                }
                Err(TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}
