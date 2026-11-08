//! Palavras e slots de um nome (spec §5.2).

const ROLES: &[&str] = &[
    "service",
    "usecase",
    "repository",
    "handler",
    "mapper",
    "schema",
    "dto",
    "input",
    "output",
    "protocol",
    "exception",
    "client",
    "event",
    "command",
    "route",
];

const VERBS: &[&str] = &[
    "get",
    "set",
    "create",
    "calculate",
    "validate",
    "update",
    "delete",
    "find",
    "list",
    "handle",
    "execute",
    "build",
    "parse",
    "publish",
    "consume",
    "save",
    "load",
    "send",
];

/// Vocabulário que classifica palavras em slots (spec §5.2); sementes embutidas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    pub roles: Vec<String>,
    pub verbs: Vec<String>,
}

impl Default for Vocabulary {
    fn default() -> Self {
        let own = |list: &[&str]| list.iter().map(|s| (*s).to_string()).collect();
        Self {
            roles: own(ROLES),
            verbs: own(VERBS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Action,
    Object,
    Role,
    /// Segmento de contexto delimitado (o que vem depois da raiz, ex.: `orders`).
    Context,
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

fn is_dunder(name: &str) -> bool {
    name.len() > 4 && name.starts_with("__") && name.ends_with("__")
}

#[derive(Clone, Copy, PartialEq)]
enum CharClass {
    Lower,
    Upper,
    Digit,
}

fn char_class(c: char) -> CharClass {
    if c.is_ascii_digit() {
        CharClass::Digit
    } else if c.is_uppercase() {
        CharClass::Upper
    } else {
        CharClass::Lower
    }
}

fn split_part(part: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = part.chars().collect();
    let mut start = 0;
    for i in 1..chars.len() {
        let (prev, cur) = (char_class(chars[i - 1]), char_class(chars[i]));
        let next_is_lower = chars
            .get(i + 1)
            .is_some_and(|&n| char_class(n) == CharClass::Lower);
        let boundary = match (prev, cur) {
            (CharClass::Lower, CharClass::Upper) => true,
            (CharClass::Upper, CharClass::Upper) => next_is_lower,
            (a, b) => a != b && (a == CharClass::Digit || b == CharClass::Digit),
        };
        if boundary {
            out.push(chars[start..i].iter().collect::<String>().to_lowercase());
            start = i;
        }
    }
    if start < chars.len() {
        out.push(chars[start..].iter().collect::<String>().to_lowercase());
    }
}

/// Quebra um nome por snake_case, CamelCase e dígitos, em minúsculas.
///
/// Siglas ficam juntas (`HTTPServer` vira `http`, `server`); nomes dunder não geram palavras.
pub fn split_name(name: &str) -> Vec<String> {
    if is_dunder(name) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for part in name.split('_').filter(|p| !p.is_empty()) {
        split_part(part, &mut out);
    }
    out
}

/// Junta tokens vizinhos quando a forma unida é um papel (`use`, `case` vira `usecase`).
fn join_roles(tokens: Vec<String>, vocab: &Vocabulary) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some(tok) = iter.next() {
        if let Some(next) = iter.peek() {
            let joined = format!("{tok}{next}");
            if vocab.roles.contains(&joined) {
                out.push(joined);
                iter.next();
                continue;
            }
        }
        out.push(tok);
    }
    out
}

fn slot_of(word: &str, kind: NameKind, vocab: &Vocabulary) -> Slot {
    let has = |list: &[String]| list.iter().any(|w| w == word);
    if has(&vocab.roles) {
        Slot::Role
    } else if kind != NameKind::Attribute && has(&vocab.verbs) {
        Slot::Action
    } else {
        Slot::Object
    }
}

fn segment_marker(word: &str) -> Option<Segment> {
    match word {
        "given" => Some(Segment::Given),
        "when" => Some(Segment::When),
        "then" => Some(Segment::Then),
        _ => None,
    }
}

/// Palavras de um nome com slot (vocabulário semente) e `segment` em testes.
pub fn classify_name(name: &str, kind: NameKind) -> Vec<WordOcc> {
    classify_name_with(name, kind, &Vocabulary::default())
}

/// Como `classify_name`, com o vocabulário do repositório (`memo.toml`).
pub fn classify_name_with(name: &str, kind: NameKind, vocab: &Vocabulary) -> Vec<WordOcc> {
    let mut tokens = split_name(name);
    if kind == NameKind::Test && tokens.first().is_some_and(|t| t == "test") {
        tokens.remove(0);
    }
    let mut segment = None;
    let mut out = Vec::new();
    for word in join_roles(tokens, vocab) {
        if kind == NameKind::Test
            && let Some(marker) = segment_marker(&word)
        {
            segment = Some(marker);
            continue;
        }
        out.push(WordOcc {
            slot: slot_of(&word, kind, vocab),
            text: word,
            segment,
        });
    }
    out
}
