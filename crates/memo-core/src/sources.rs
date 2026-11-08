//! Palavras que não vêm de nomes de símbolos: diretórios, imports e docstrings de teste.
//!
//! Funções puras: recebem o caminho ou o texto e a configuração e devolvem palavras.

use crate::config::Config;
use crate::words::{NameKind, Segment, Slot, WordOcc, classify_name_with};
use std::collections::HashSet;

fn root_segments(config: &Config) -> Vec<&str> {
    config
        .context_root
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Palavras de um segmento (`value_objects` vira `value`, `objects`); com `context`, o slot
/// de todas é `context`.
fn segment_words(segment: &str, config: &Config, context: bool) -> Vec<WordOcc> {
    let mut words = classify_name_with(segment, NameKind::Callable, &config.vocabulary);
    if context {
        for word in &mut words {
            word.slot = Slot::Context;
        }
    }
    words
}

fn is_layer(segment: &str, config: &Config) -> bool {
    config.layers.iter().any(|l| l == segment)
}

/// Palavras dos diretórios de um arquivo. Com a raiz de contexto no início do caminho
/// (`context_root`, que pode ser um prefixo como `backend/easy_lease`), a raiz sai, o segmento
/// seguinte é o contexto e os de camada saem; sem a raiz, valem todos os diretórios.
pub fn path_words(path: &str, config: &Config) -> Vec<WordOcc> {
    let parts: Vec<&str> = path.split('/').collect();
    let dirs = &parts[..parts.len().saturating_sub(1)];
    let root = root_segments(config);
    let rooted = !root.is_empty() && dirs.len() > root.len() && dirs.starts_with(&root);
    let (rest, has_context) = if rooted {
        (&dirs[root.len()..], true)
    } else {
        (dirs, false)
    };
    let mut out = Vec::new();
    for (i, segment) in rest.iter().enumerate() {
        if has_context && i > 0 && is_layer(segment, config) {
            continue;
        }
        out.extend(segment_words(segment, config, has_context && i == 0));
    }
    out
}

/// Palavras de um import (`app.orders.domain.order_status.OrderStatus`): nome importado e
/// caminho. A raiz do pacote (último segmento de `context_root`) e as camadas saem, o segmento
/// seguinte à raiz é o contexto. Alvo que não começa pela raiz (externo) usa todos os segmentos.
pub fn import_words(target: &str, config: &Config) -> Vec<WordOcc> {
    let segments: Vec<&str> = target.split('.').filter(|s| !s.is_empty()).collect();
    let package = root_segments(config).last().copied().unwrap_or("");
    let internal = segments.len() > 1 && segments[0] == package;
    let mut out = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        if internal && (i == 0 || (i > 1 && is_layer(segment, config))) {
            continue;
        }
        out.extend(segment_words(segment, config, internal && i == 1));
    }
    out
}

fn marker(chunk: &str) -> Option<Segment> {
    match chunk.to_lowercase().as_str() {
        "given" => Some(Segment::Given),
        "when" => Some(Segment::When),
        "then" => Some(Segment::Then),
        _ => None,
    }
}

/// Palavras de uma docstring de teste. `GIVEN`, `WHEN` e `THEN` são marcadores de segmento,
/// não palavras; só conta a palavra que já existe como palavra de algum identificador (`known`).
pub fn doc_words(text: &str, known: &HashSet<String>, config: &Config) -> Vec<WordOcc> {
    let mut segment = None;
    let mut out = Vec::new();
    for chunk in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if chunk.is_empty() {
            continue;
        }
        if let Some(found) = marker(chunk) {
            segment = Some(found);
            continue;
        }
        for mut word in classify_name_with(chunk, NameKind::Callable, &config.vocabulary) {
            if known.contains(&word.text) {
                word.segment = segment;
                out.push(word);
            }
        }
    }
    out
}
