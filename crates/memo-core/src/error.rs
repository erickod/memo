//! Erros de domínio do núcleo.

#[derive(Debug)]
pub enum CoreError {
    Io(String),
    Store(String),
    Source(String),
}
