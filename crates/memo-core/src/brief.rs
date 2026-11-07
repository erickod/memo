//! `brief`: âncoras, símbolos, contorno, padrão, escritores e notas para um pedido (spec §8.5).

use crate::error::CoreError;
use crate::query::SymbolAddr;
use crate::store::SqliteStore;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anchor {
    pub word: String,
    /// Peso normalizado pelo maior (o primeiro vale 1,0).
    pub score: f64,
    /// Casou só por prefixo (`~`).
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoredSymbol {
    pub addr: SymbolAddr,
    pub score: f64,
}

/// Uma linha do contorno: arestas de chamada de um símbolo listado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outline {
    pub symbol: String,
    /// `->` chama, `<-` é chamado; nome curto, `~` e linha.
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriterLine {
    pub attr: String,
    pub writers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Brief {
    pub request: String,
    pub anchors: Vec<Anchor>,
    pub ignored: Vec<String>,
    pub symbols: Vec<ScoredSymbol>,
    pub outline: Vec<Outline>,
    pub pattern: Option<Pattern>,
    pub writers: Vec<WriterLine>,
    pub notes: Vec<String>,
    /// Itens omitidos por já terem sido entregues nesta task (spec §8.4).
    pub unchanged: Option<usize>,
}

/// Minúsculas, sem acentos, separado por não letras, sem stopwords pt e en.
pub fn tokenize(_text: &str) -> Vec<String> {
    todo!("spec §8.5 passo 1")
}

pub fn brief(_store: &SqliteStore, _request: &str) -> Result<Brief, CoreError> {
    todo!("spec §8.5")
}

/// Remove do `Brief` o que já foi entregue à `task`, registra o que ficou e preenche
/// `unchanged`; com `full` devolve tudo, e só registra.
pub fn apply_delta(
    _store: &SqliteStore,
    _task: &str,
    _brief: &mut Brief,
    _full: bool,
) -> Result<(), CoreError> {
    todo!("spec §8.4")
}
