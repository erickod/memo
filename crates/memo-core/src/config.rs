//! Configuração do repositório, `memo.toml` (spec §6.1).

use crate::error::CoreError;
use crate::words::Vocabulary;
use std::path::Path;

const SKELETON: &[&str] = &[
    "get", "data", "utils", "helper", "base", "manager", "common", "process", "service",
];
const WRITE_METHODS: &[&str] = &[
    "save", "add", "insert", "update", "delete", "publish", "send", "commit", "flush",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub main_branch: Option<String>,
    pub vocabulary: Vocabulary,
    pub skeleton: Vec<String>,
    pub write_methods: Vec<String>,
    pub entrypoint_decorators: Vec<String>,
    pub min_specificity: f64,
    pub stale_on: Vec<String>,
    /// Segmento da raiz do contexto: o que vem depois dele é o `context`.
    pub context_root: String,
    /// Segmentos de caminho que nomeiam a camada.
    pub layers: Vec<String>,
}

fn own(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            main_branch: None,
            vocabulary: Vocabulary::default(),
            skeleton: own(SKELETON),
            write_methods: own(WRITE_METHODS),
            entrypoint_decorators: Vec::new(),
            min_specificity: 0.35,
            stale_on: own(&["sig", "body"]),
            context_root: "app".to_string(),
            layers: own(&["domain", "application", "infra"]),
        }
    }
}

impl Config {
    /// Lê o TOML; chaves ausentes mantêm as sementes e listas presentes substituem a semente.
    pub fn from_toml(_text: &str) -> Result<Self, CoreError> {
        todo!("spec §6.1 memo.toml")
    }

    /// Lê `<dir>/memo.toml`; sem o arquivo, devolve as sementes.
    pub fn load(_dir: &Path) -> Result<Self, CoreError> {
        todo!("spec §6.1 memo.toml")
    }
}
