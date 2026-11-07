//! Extração estática de Python (spec §5).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolFact {
    pub qname: String,
    pub sig_hash: String,
    pub body_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveFact {
    pub src_qname: String,
    /// Nome do parâmetro.
    pub attr: String,
    /// `None` quando o parâmetro não tem anotação (spec §5.3).
    pub dst_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub symbols: Vec<SymbolFact>,
    pub receives: Vec<ReceiveFact>,
}

#[derive(Debug)]
pub struct ExtractError(pub String);

/// Lê um arquivo Python (`module` é o módulo pontuado) e devolve fatos.
pub fn extract_python(_module: &str, _source: &str) -> Result<FileFacts, ExtractError> {
    todo!("spec §5.1 a §5.3")
}
