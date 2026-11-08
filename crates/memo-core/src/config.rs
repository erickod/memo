//! Configuração do repositório, `memo.toml` (spec §6.1).

use crate::error::CoreError;
use crate::words::Vocabulary;
use serde::Deserialize;
use std::path::Path;

const SKELETON: &[&str] = &[
    "get", "data", "utils", "helper", "base", "manager", "common", "process", "service",
];
const WRITE_METHODS: &[&str] = &[
    "save", "add", "insert", "update", "delete", "publish", "send", "commit", "flush",
];

/// Pesos das pontes entre palavras (D18); valores iniciais, editáveis em `[bridges]`.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeWeights {
    pub symbol: f64,
    pub test: f64,
    pub import: f64,
    pub import_external: f64,
    pub call: f64,
    pub file: f64,
    pub context: f64,
    /// Fator aplicado a uma ponte aproximada (`~`).
    pub approximate: f64,
}

impl Default for BridgeWeights {
    fn default() -> Self {
        Self {
            symbol: 1.0,
            test: 0.8,
            import: 0.6,
            import_external: 0.3,
            call: 0.5,
            file: 0.3,
            context: 0.1,
            approximate: 0.5,
        }
    }
}

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
    pub bridges: BridgeWeights,
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
            bridges: BridgeWeights::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBridges {
    symbol: Option<f64>,
    test: Option<f64>,
    import: Option<f64>,
    import_external: Option<f64>,
    call: Option<f64>,
    file: Option<f64>,
    context: Option<f64>,
    approximate: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    bridges: Option<RawBridges>,
    main_branch: Option<String>,
    roles: Option<Vec<String>>,
    verbs: Option<Vec<String>>,
    skeleton: Option<Vec<String>>,
    write_methods: Option<Vec<String>>,
    entrypoint_decorators: Option<Vec<String>>,
    min_specificity: Option<f64>,
    stale_on: Option<Vec<String>>,
    context_root: Option<String>,
    layers: Option<Vec<String>>,
}

impl Config {
    /// Lê o TOML; chaves ausentes mantêm as sementes e listas presentes substituem a semente.
    pub fn from_toml(text: &str) -> Result<Self, CoreError> {
        let raw: Raw = toml::from_str(text).map_err(|e| CoreError::Config(e.to_string()))?;
        let mut config = Self::default();
        config.main_branch = raw.main_branch.or(config.main_branch);
        config.vocabulary.roles = raw.roles.unwrap_or(config.vocabulary.roles);
        config.vocabulary.verbs = raw.verbs.unwrap_or(config.vocabulary.verbs);
        config.skeleton = raw.skeleton.unwrap_or(config.skeleton);
        config.write_methods = raw.write_methods.unwrap_or(config.write_methods);
        config.entrypoint_decorators = raw
            .entrypoint_decorators
            .unwrap_or(config.entrypoint_decorators);
        config.min_specificity = raw.min_specificity.unwrap_or(config.min_specificity);
        config.stale_on = raw.stale_on.unwrap_or(config.stale_on);
        config.context_root = raw.context_root.unwrap_or(config.context_root);
        config.layers = raw.layers.unwrap_or(config.layers);
        if let Some(b) = raw.bridges {
            let w = &mut config.bridges;
            w.symbol = b.symbol.unwrap_or(w.symbol);
            w.test = b.test.unwrap_or(w.test);
            w.import = b.import.unwrap_or(w.import);
            w.import_external = b.import_external.unwrap_or(w.import_external);
            w.call = b.call.unwrap_or(w.call);
            w.file = b.file.unwrap_or(w.file);
            w.context = b.context.unwrap_or(w.context);
            w.approximate = b.approximate.unwrap_or(w.approximate);
        }
        let w = &config.bridges;
        let all = [
            w.symbol,
            w.test,
            w.import,
            w.import_external,
            w.call,
            w.file,
            w.context,
            w.approximate,
        ];
        if all.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(CoreError::Config(
                "[bridges] weights must be finite and not negative".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&config.min_specificity) {
            return Err(CoreError::Config(
                "min_specificity must be between 0 and 1".to_string(),
            ));
        }
        if let Some(bad) = config
            .stale_on
            .iter()
            .find(|v| !matches!(v.as_str(), "sig" | "body"))
        {
            return Err(CoreError::Config(format!(
                "stale_on accepts \"sig\" and \"body\", got \"{bad}\""
            )));
        }
        Ok(config)
    }

    /// O `memo.toml` com as sementes, para o `init` gravar.
    pub fn to_toml(&self) -> String {
        let list = |values: &[String]| {
            let items: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
            format!("[{}]", items.join(", "))
        };
        let mut out = String::from(
            "# memo.toml: configuração do repositório. Listas presentes substituem as sementes.\n",
        );
        if let Some(branch) = &self.main_branch {
            out.push_str(&format!("main_branch = {branch:?}\n"));
        }
        out.push_str(&format!("roles = {}\n", list(&self.vocabulary.roles)));
        out.push_str(&format!("verbs = {}\n", list(&self.vocabulary.verbs)));
        out.push_str(&format!("skeleton = {}\n", list(&self.skeleton)));
        out.push_str(&format!("write_methods = {}\n", list(&self.write_methods)));
        out.push_str(&format!(
            "entrypoint_decorators = {}\n",
            list(&self.entrypoint_decorators)
        ));
        out.push_str(&format!("min_specificity = {:?}\n", self.min_specificity));
        out.push_str(&format!("stale_on = {}\n", list(&self.stale_on)));
        out.push_str(&format!("context_root = {:?}\n", self.context_root));
        out.push_str(&format!("layers = {}\n", list(&self.layers)));
        let w = &self.bridges;
        out.push_str("\n[bridges]\n");
        for (key, value) in [
            ("symbol", w.symbol),
            ("test", w.test),
            ("import", w.import),
            ("import_external", w.import_external),
            ("call", w.call),
            ("file", w.file),
            ("context", w.context),
            ("approximate", w.approximate),
        ] {
            out.push_str(&format!("{key} = {value:?}\n"));
        }
        out
    }

    /// Lê `<dir>/memo.toml`; sem o arquivo, devolve as sementes.
    pub fn load(dir: &Path) -> Result<Self, CoreError> {
        match std::fs::read_to_string(dir.join("memo.toml")) {
            Ok(text) => Self::from_toml(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
}
