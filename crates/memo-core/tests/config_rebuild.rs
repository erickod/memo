mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::{IndexReport, index_with, index_with_notes};
use memo_core::notes::{NewNote, NoteDir, is_generic_anchor};
use memo_core::query::word_stats;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

fn get_like(repo: &Repo) {
    for i in 0..50 {
        repo.write(
            &format!("app/g/g{}.py", token(i)),
            &format!("def get_{}():\n    return 1\n", token(i + 200)),
        );
    }
}

fn index(repo: &Repo, store: &mut SqliteStore, toml: &str) -> IndexReport {
    let config = Config::from_toml(toml).unwrap();
    index_with(&WorkTree::new(repo.path()), store, &config).unwrap()
}

fn esqueleto(store: &SqliteStore, word: &str) -> bool {
    word_stats(store, word).unwrap().unwrap().esqueleto
}

fn file_store(repo: &Repo) -> (SqliteStore, PathBuf) {
    let path = repo.path().join("base.db");
    (SqliteStore::open(&path).unwrap(), path)
}

fn context_of(db: &Path, path: &str) -> Option<String> {
    Connection::open(db)
        .unwrap()
        .query_row("SELECT context FROM file WHERE path = ?1", [path], |r| {
            r.get(0)
        })
        .unwrap()
}

fn note(anchor: &str, text: &str) -> NewNote {
    NewNote {
        tipo: "regra".into(),
        ancora: anchor.into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: text.into(),
        task: None,
        confirm: false,
        substitui: None,
        aliases: Vec::new(),
    }
}

/// GIVEN a repository indexed with context_root = "app" and no file changed
/// WHEN  context_root changes to "app/orders" and index runs again
/// THEN  file contexts follow the new root and the report says the index was rebuilt
#[test]
fn test_given_indexed_repo_when_context_root_changes_then_next_index_rebuilds_contexts() {
    let repo = Repo::empty();
    repo.write("app/orders/sub/m.py", "def place():\n    return 1\n");
    let (mut store, db) = file_store(&repo);
    index(&repo, &mut store, "context_root = \"app\"\n");
    assert_eq!(
        context_of(&db, "app/orders/sub/m.py").as_deref(),
        Some("orders")
    );
    let report = index(&repo, &mut store, "context_root = \"app/orders\"\n");
    assert!(report.rebuilt, "{report:?}");
    assert_eq!(
        context_of(&db, "app/orders/sub/m.py").as_deref(),
        Some("sub")
    );
}

/// GIVEN a repository indexed with skeleton = []
/// WHEN  skeleton becomes ["get"] with no file changed and index runs again
/// THEN  word_stats for get has esqueleto true
#[test]
fn test_given_indexed_repo_when_skeleton_changes_then_esqueleto_follows_without_file_change() {
    let repo = Repo::empty();
    get_like(&repo);
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&repo, &mut store, "skeleton = []\n");
    assert!(!esqueleto(&store, "get"));
    index(&repo, &mut store, "skeleton = [\"get\"]\n");
    assert!(esqueleto(&store, "get"));
}

/// GIVEN a repository indexed once
/// WHEN  index runs again with the same files and the same config
/// THEN  the report says nothing was written and the config hash is unchanged
#[test]
fn test_given_unchanged_files_and_config_when_indexed_then_nothing_is_written() {
    let repo = Repo::empty();
    get_like(&repo);
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&repo, &mut store, "skeleton = []\n");
    let hash = store.get_meta("config_hash").unwrap();
    assert!(hash.is_some());
    let changes = store.total_changes();
    let report = index(&repo, &mut store, "skeleton = []\n");
    assert!(!report.wrote && !report.rebuilt, "{report:?}");
    assert_eq!(store.total_changes(), changes);
    assert_eq!(store.get_meta("config_hash").unwrap(), hash);
}

/// GIVEN a repository indexed once
/// WHEN  only min_coherence changes and index runs again
/// THEN  nothing is written and the index is not rebuilt
#[test]
fn test_given_min_coherence_changed_when_indexed_then_index_is_not_rebuilt() {
    let repo = Repo::empty();
    get_like(&repo);
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&repo, &mut store, "min_coherence = 0.30\n");
    let report = index(&repo, &mut store, "min_coherence = 0.80\n");
    assert!(!report.wrote && !report.rebuilt, "{report:?}");
}

/// GIVEN two notes in the notes directory and an indexed repository
/// WHEN  the skeleton changes and the index is rebuilt
/// THEN  the note files are byte-identical and no NOTE_* event was added
#[test]
fn test_given_config_change_when_rebuilt_then_notes_and_note_events_are_untouched() {
    let repo = Repo::empty();
    get_like(&repo);
    let mut store = SqliteStore::open_in_memory().unwrap();
    let before = Config::from_toml("skeleton = []\n").unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &before).unwrap();
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    notes
        .add(
            &store,
            note("word:get", "Get é o acessor."),
            "2026-11-08T10:00:00Z",
        )
        .unwrap();
    notes
        .add(
            &store,
            note(&format!("word:{}", token(200)), "Palavra distinta."),
            "2026-11-08T10:01:00Z",
        )
        .unwrap();
    let dir = repo.path().join(".git/memo/notes");
    let snapshot = |dir: &std::path::Path| -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let files = snapshot(&dir);
    assert_eq!(files.len(), 2);
    let baseline = store.events().unwrap().len();
    let after = Config::from_toml("skeleton = [\"get\"]\n").unwrap();
    let report = index_with_notes(&WorkTree::new(repo.path()), &mut store, &after, &notes).unwrap();
    assert!(report.rebuilt, "{report:?}");
    assert_eq!(snapshot(&dir), files);
    let kinds: Vec<String> = store
        .events()
        .unwrap()
        .into_iter()
        .skip(baseline)
        .map(|e| e.kind)
        .collect();
    assert!(kinds.iter().all(|k| !k.starts_with("NOTE_")), "{kinds:?}");
}

/// GIVEN a derived database with schema_version 2 and no config hash in meta
/// WHEN  index runs
/// THEN  the database is rebuilt and meta now holds the config hash
#[test]
fn test_given_database_without_config_hash_when_indexed_then_it_is_rebuilt_and_hash_stored() {
    let repo = Repo::empty();
    get_like(&repo);
    let (mut store, db) = file_store(&repo);
    index(&repo, &mut store, "skeleton = []\n");
    Connection::open(&db)
        .unwrap()
        .execute("DELETE FROM meta WHERE key = 'config_hash'", [])
        .unwrap();
    let report = index(&repo, &mut store, "skeleton = []\n");
    assert!(report.rebuilt, "{report:?}");
    assert!(store.get_meta("config_hash").unwrap().is_some());
}

/// GIVEN an active note anchored on word:get, indexed with skeleton = []
/// WHEN  get is added to skeleton and the repository is indexed again
/// THEN  the index is rebuilt, the note stays active with the generic anchor mark and no NOTE_* event is recorded
#[test]
fn test_given_note_on_word_when_word_enters_skeleton_list_then_marked_generic_without_event() {
    let repo = Repo::empty();
    get_like(&repo);
    let mut store = SqliteStore::open_in_memory().unwrap();
    let before = Config::from_toml("skeleton = []\n").unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &before).unwrap();
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let added = notes
        .add(
            &store,
            note("word:get", "Get é o acessor."),
            "2026-11-08T10:00:00Z",
        )
        .unwrap()
        .note;
    assert!(!is_generic_anchor(&store, &added).unwrap());
    let baseline = store.events().unwrap().len();
    let after = Config::from_toml("skeleton = [\"get\"]\n").unwrap();
    let report = index_with_notes(&WorkTree::new(repo.path()), &mut store, &after, &notes).unwrap();
    assert!(report.rebuilt, "{report:?}");
    let current = notes.get(&added.id).unwrap();
    assert_eq!(current.estado, "proposta");
    assert!(is_generic_anchor(&store, &current).unwrap());
    let kinds: Vec<String> = store
        .events()
        .unwrap()
        .into_iter()
        .skip(baseline)
        .map(|e| e.kind)
        .collect();
    assert!(kinds.iter().all(|k| !k.starts_with("NOTE_")), "{kinds:?}");
}
