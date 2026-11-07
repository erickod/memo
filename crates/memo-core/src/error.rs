//! Erros de domínio do núcleo.

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store error: {0}")]
    Store(#[from] rusqlite::Error),
    #[error("source error: {0}")]
    Source(String),
}
