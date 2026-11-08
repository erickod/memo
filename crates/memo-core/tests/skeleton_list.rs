mod common;

use common::Repo;
use memo_core::brief::brief;
use memo_core::config::Config;
use memo_core::error::CoreError;
use memo_core::index::index_with;
use memo_core::notes::{NewNote, NoteDir};
use memo_core::query::word_stats;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

/// Palavra frequente ligada a tudo: 50 módulos, cada um com `get_<palavra distinta>`.
fn get_like(repo: &Repo) {
    for i in 0..50 {
        repo.write(
            &format!("app/g/g{}.py", token(i)),
            &format!("def get_{}():\n    return 1\n", token(i + 200)),
        );
    }
}

fn build(config: &str) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    get_like(&repo);
    let config = Config::from_toml(config).unwrap();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    (repo, store)
}

fn stats(store: &SqliteStore, word: &str) -> memo_core::query::WordStats {
    word_stats(store, word).unwrap().unwrap()
}

fn note_on_get() -> NewNote {
    NewNote {
        tipo: "regra".into(),
        ancora: "word:get".into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: "Get é o acessor padrão.".into(),
        task: None,
        confirm: false,
        substitui: None,
        aliases: Vec::new(),
    }
}

/// GIVEN the get_like fixture (50 modules, get_<distinct word>) and skeleton = []
/// WHEN  the repository is indexed
/// THEN  word_stats for get has esqueleto false
#[test]
fn test_given_frequent_low_coherence_word_outside_list_when_indexed_then_not_esqueleto() {
    let (_r, store) = build("skeleton = []\n");
    let get = stats(&store, "get");
    assert!(get.especificidade < 0.35 && get.coerencia < 0.30, "{get:?}");
    assert!(!get.esqueleto, "{get:?}");
}

/// GIVEN the get_like fixture and skeleton = ["get"]
/// WHEN  the repository is indexed
/// THEN  word_stats for get has esqueleto true
#[test]
fn test_given_word_in_skeleton_list_when_indexed_then_esqueleto_true() {
    let (_r, store) = build("skeleton = [\"get\"]\n");
    assert!(stats(&store, "get").esqueleto);
}

/// GIVEN the get_like fixture, skeleton = [] and min_specificity = min_coherence = 1.0
/// WHEN  the repository is indexed
/// THEN  word_stats for get has esqueleto false (the thresholds no longer decide)
#[test]
fn test_given_min_thresholds_at_one_when_indexed_then_word_outside_list_not_esqueleto() {
    let (_r, store) = build("skeleton = []\nmin_specificity = 1.0\nmin_coherence = 1.0\n");
    assert!(!stats(&store, "get").esqueleto);
}

/// GIVEN the get_like fixture and skeleton = []
/// WHEN  the repository is indexed
/// THEN  word_stats for get keeps coerencia below 0.30 and especificidade below 0.35
#[test]
fn test_given_low_coherence_word_when_indexed_then_coerencia_and_especificidade_stored() {
    let (_r, store) = build("skeleton = []\n");
    let get = stats(&store, "get");
    assert!(get.coerencia > 0.0 && get.coerencia < 0.30, "{get:?}");
    assert!(get.especificidade < 0.35, "{get:?}");
}

/// GIVEN the get_like fixture, skeleton = [] and the request "get"
/// WHEN  brief is computed
/// THEN  get is the anchor, the anchor list is not empty and symbols are listed
#[test]
fn test_given_low_coherence_word_outside_list_when_brief_then_word_is_anchor() {
    let (_r, store) = build("skeleton = []\n");
    let b = brief(&store, "get").unwrap();
    assert_eq!(b.anchors.first().map(|a| a.word.as_str()), Some("get"));
    assert!(!b.symbols.is_empty());
}

/// GIVEN the get_like fixture, skeleton = ["get"] and the request "get"
/// WHEN  brief is computed
/// THEN  anchors are empty, get appears as ignored and no symbol is listed
#[test]
fn test_given_word_in_skeleton_list_when_brief_then_no_anchor_and_word_ignored() {
    let (_r, store) = build("skeleton = [\"get\"]\n");
    let b = brief(&store, "get").unwrap();
    assert!(b.anchors.is_empty() && b.symbols.is_empty());
    assert_eq!(b.ignored, vec!["get"]);
}

/// GIVEN the get_like fixture and skeleton = ["get"]
/// WHEN  a note with anchor word:get is added
/// THEN  it fails with ancora_generica (exit 4 in the CLI)
#[test]
fn test_given_word_in_skeleton_list_when_note_with_word_anchor_then_ancora_generica_error() {
    let (repo, store) = build("skeleton = [\"get\"]\n");
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let err = notes
        .add(&store, note_on_get(), "2026-11-08T10:00:00Z")
        .unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::Validation {
                code: "ancora_generica",
                ..
            }
        ),
        "{err:?}"
    );
}

/// GIVEN the get_like fixture and skeleton = []
/// WHEN  a note with anchor word:get is added
/// THEN  the note is stored as proposta
#[test]
fn test_given_word_outside_list_when_note_with_word_anchor_then_note_is_accepted() {
    let (repo, store) = build("skeleton = []\n");
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let added = notes
        .add(&store, note_on_get(), "2026-11-08T10:00:00Z")
        .unwrap();
    assert_eq!(added.note.estado, "proposta");
}
