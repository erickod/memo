//! Evidência de que um uso (`raw_ref`) se refere ao alvo do `impact`, além de conter o nome dele.
//!
//! Função pura: recebe palavras e endereços já lidos do banco e devolve o nível. A camada
//! relacional dá as palavras; aqui é inferência, ajustável sem reconstruir a base.

use serde::Serialize;
use std::collections::BTreeSet;

/// Nível de evidência de um uso, do mais forte ao mais fraco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    /// O próprio uso traz a evidência (classe da família, ou módulo do alvo).
    Forte,
    /// A evidência está em outro uso ou no tipo do mesmo símbolo.
    Media,
    /// Só o nome do alvo; nenhuma evidência.
    SoNome,
}

/// O que conta como evidência para o alvo.
pub enum Anchor<'a> {
    /// Alvo método: as palavras de nome de cada classe da família (dona, bases, subclasses,
    /// Protocols que ela implementa).
    Family(&'a [BTreeSet<String>]),
    /// Alvo sem família (função, classe, módulo): o módulo do alvo e o do uso, e os alvos dos
    /// imports do arquivo do uso.
    Module {
        target: &'a str,
        target_module: &'a str,
        use_module: &'a str,
        imports: &'a [String],
    },
}

/// Classifica um uso. `use_words` são as palavras do próprio uso; `symbol_words`, as dos outros
/// usos e do tipo do símbolo que o contém.
pub fn classify(
    use_words: &BTreeSet<String>,
    symbol_words: &BTreeSet<String>,
    anchor: &Anchor,
) -> Evidence {
    match anchor {
        Anchor::Family(family) => {
            // Uma classe conta quando todas as palavras do nome dela estão no texto.
            let has_class = |words: &BTreeSet<String>| {
                family
                    .iter()
                    .any(|class| !class.is_empty() && class.is_subset(words))
            };
            if has_class(use_words) {
                Evidence::Forte
            } else if has_class(symbol_words) {
                Evidence::Media
            } else {
                Evidence::SoNome
            }
        }
        Anchor::Module {
            target,
            target_module,
            use_module,
            imports,
        } => {
            let imported = imports.iter().any(|i| i == target || i == target_module);
            if use_module == target_module || imported {
                Evidence::Forte
            } else {
                Evidence::SoNome
            }
        }
    }
}
