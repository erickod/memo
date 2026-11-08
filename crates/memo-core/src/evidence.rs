//! Evidência de que um uso (`raw_ref`) se refere ao alvo do `impact`, além de conter o nome dele.
//!
//! Função pura: recebe palavras e endereços já lidos do banco e devolve o nível. A camada
//! relacional dá as palavras; aqui é inferência, ajustável sem reconstruir a base.

use serde::Serialize;

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
    /// Alvo método: o nome de cada classe da família (dona, bases, subclasses, Protocols que ela
    /// implementa), comparado como identificador inteiro (TASK-6, D18).
    Family(&'a [String]),
    /// Alvo sem família (função, classe, módulo): o módulo do alvo e o do uso, e os alvos dos
    /// imports do arquivo do uso.
    Module {
        target: &'a str,
        target_module: &'a str,
        use_module: &'a str,
        imports: &'a [String],
    },
}

/// Identificadores de um texto de uso (`Repo(session).run` → `Repo`, `session`, `run`).
fn identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
}

/// Algum texto cita uma classe da família pelo nome inteiro: `AlphaBeta` não cita `Alpha`.
fn cites_class(texts: &[&str], family: &[String]) -> bool {
    texts
        .iter()
        .flat_map(|t| identifiers(t))
        .any(|id| family.iter().any(|class| class == id))
}

/// Classifica um uso. `use_texts` são os textos do próprio uso (e dos usos da mesma linha);
/// `symbol_texts`, os dos outros usos e do tipo do símbolo que o contém.
pub fn classify(use_texts: &[&str], symbol_texts: &[&str], anchor: &Anchor) -> Evidence {
    match anchor {
        Anchor::Family(family) => {
            if cites_class(use_texts, family) {
                Evidence::Forte
            } else if cites_class(symbol_texts, family) {
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
