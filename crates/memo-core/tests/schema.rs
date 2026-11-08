mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use rusqlite::Connection;

/// GIVEN the fixture indexed into a file-backed store
/// WHEN  the occurrence rows of symbol names (source 'name') are read
/// THEN  every one has owner_kind 'symbol' and owner_id equal to its symbol_id
#[test]
fn test_given_symbol_word_when_stored_then_occurrence_has_owner_kind_and_name_source() {
    let repo = Repo::fixture();
    let db = repo.path().join(".git/schema-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();

    let conn = Connection::open(&db).unwrap();
    let kinds: Vec<(String, String)> = conn
        .prepare("SELECT DISTINCT owner_kind, source FROM occurrence WHERE source = 'name'")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(kinds, vec![("symbol".to_string(), "name".to_string())]);
    let mismatched: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM occurrence WHERE source = 'name' AND owner_id != symbol_id",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mismatched, 0);
}

/// GIVEN a store at schema_version '1' (occurrence without owner_kind) and a note file on disk
/// WHEN  the store is opened again and the repository is indexed
/// THEN  the old graph is discarded, schema_version is '6', indexing succeeds and the note file is intact
#[test]
fn test_given_old_schema_version_when_opened_then_db_discarded_and_notes_kept() {
    let repo = Repo::fixture();
    let db = repo.path().join(".git/schema-old.db");
    let note = repo.path().join(".git/memo-notes/n-keep0000.md");
    std::fs::create_dir_all(note.parent().unwrap()).unwrap();
    std::fs::write(&note, "note body").unwrap();
    {
        let mut store = SqliteStore::open(&db).unwrap();
        index(&WorkTree::new(repo.path()), &mut store).unwrap();
    }
    {
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "DROP TABLE occurrence;
             CREATE TABLE occurrence(symbol_id INTEGER NOT NULL, word_id INTEGER NOT NULL,
               pos INTEGER NOT NULL, slot TEXT NOT NULL, segment TEXT, PRIMARY KEY(symbol_id, pos));
             UPDATE meta SET value = '1' WHERE key = 'schema_version';",
        )
        .unwrap();
    }

    let mut store = SqliteStore::open(&db).unwrap();
    assert_eq!(store.counts().unwrap(), (0, 0));
    assert_eq!(
        store.get_meta("schema_version").unwrap().as_deref(),
        Some("6")
    );
    let report = index(&WorkTree::new(repo.path()), &mut store).unwrap();
    assert!(report.wrote && report.files_with_error.is_empty());
    assert_eq!(std::fs::read_to_string(&note).unwrap(), "note body");
}
