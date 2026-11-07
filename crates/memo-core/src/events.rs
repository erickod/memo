//! Eventos de indexação e de notas, em lote (spec §10).

/// Contagens do lote, para o resumo de uma linha ao fim de `index`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventSummary {
    pub created: usize,
    pub changed: usize,
    pub renamed: usize,
    pub destroyed: usize,
    pub notes_stale: usize,
    pub notes_orphaned: usize,
}

impl EventSummary {
    /// `eventos: +12 símbolos, ~5 alterados, 1 renomeado, 2 notas stale`.
    pub fn line(&self) -> String {
        todo!("spec §10 resumo")
    }
}
