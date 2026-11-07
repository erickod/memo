//! Palavras e slots de um nome (spec §5.2).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Action,
    Object,
    Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    Given,
    When,
    Then,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordOcc {
    pub text: String,
    pub slot: Slot,
    pub segment: Option<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Attribute,
    Callable,
    Test,
}

/// Quebra um nome por snake_case, CamelCase e dígitos, em minúsculas.
pub fn split_name(_name: &str) -> Vec<String> {
    todo!("spec §5.2 passo 1")
}

/// Palavras de um nome com slot (vocabulário semente) e `segment` em testes.
pub fn classify_name(_name: &str, _kind: NameKind) -> Vec<WordOcc> {
    todo!("spec §5.2 passos 2 a 4")
}
