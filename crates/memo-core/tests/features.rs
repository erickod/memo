mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::error::CoreError;
use memo_core::index::{index, index_with};
use memo_core::lock::DbLock;
use memo_core::notes::{NewNote, NoteDir};
use memo_core::source::{GitRef, WorkTree};
use memo_core::store::SqliteStore;
use std::time::Duration;

const NOW: &str = "2026-11-07T14:05:00Z";
const PATH: &str = "app/billing/application/usecases/calculate_limit_usecase.py";
const EXECUTE_Q: &str =
    "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute";

fn lock_path(repo: &Repo) -> std::path::PathBuf {
    repo.path().join("base.db.lock")
}

/// GIVEN a lock held on 'base.db.lock'
/// WHEN  a second acquire waits 100 ms
/// THEN  it fails with CoreError::Locked
#[test]
fn test_given_lock_held_when_second_acquire_then_timeout_error() {
    let repo = Repo::empty();
    let _held = DbLock::acquire(&lock_path(&repo), Duration::from_millis(100)).unwrap();
    let err = DbLock::acquire(&lock_path(&repo), Duration::from_millis(100))
        .err()
        .unwrap();
    assert!(matches!(err, CoreError::Locked(_)), "{err:?}");
}

/// GIVEN a lock that was held and then dropped
/// WHEN  it is acquired again
/// THEN  the acquire succeeds
#[test]
fn test_given_lock_released_when_acquired_again_then_ok() {
    let repo = Repo::empty();
    drop(DbLock::acquire(&lock_path(&repo), Duration::from_millis(100)).unwrap());
    assert!(DbLock::acquire(&lock_path(&repo), Duration::from_millis(100)).is_ok());
}

/// GIVEN the fixture committed on 'main' and a new function added only in the work tree
/// WHEN  the graph is built from the ref 'main'
/// THEN  it holds committed symbols and not the uncommitted one
#[test]
fn test_given_git_ref_source_when_indexed_then_graph_matches_committed_tree() {
    let repo = Repo::fixture();
    repo.commit_all();
    let source = std::fs::read_to_string(repo.path().join(PATH)).unwrap();
    repo.write(PATH, &format!("{source}\n\ndef uncommitted():\n    pass\n"));
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = GitRef::new(repo.path(), "main").unwrap();
    index_with(&tree, &mut store, &Config::default()).unwrap();
    assert!(store.symbol_exists(EXECUTE_Q).unwrap());
    assert!(
        !store
            .symbol_exists("app.billing.application.usecases.calculate_limit_usecase.uncommitted")
            .unwrap()
    );
    assert_eq!(tree.commit().unwrap().len(), 40);
}

/// GIVEN a ref source already indexed
/// WHEN  it is indexed again with no commit in between
/// THEN  nothing is written
#[test]
fn test_given_ref_source_unchanged_when_reindexed_then_no_writes() {
    let repo = Repo::fixture();
    repo.commit_all();
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = GitRef::new(repo.path(), "main").unwrap();
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let changes = store.total_changes();
    let report = index_with(&tree, &mut store, &Config::default()).unwrap();
    assert!(!report.wrote);
    assert_eq!(store.total_changes(), changes);
}

/// GIVEN the fixture indexed into a file-backed store
/// WHEN  the store is backed up to another file and opened
/// THEN  the canonical graph dumps are identical
#[test]
fn test_given_store_backup_when_copied_then_graph_dump_identical() {
    let repo = Repo::fixture();
    let db = repo.path().join(".git/base.db");
    let copy = repo.path().join(".git/copy.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    store.backup_to(&copy).unwrap();
    assert_eq!(
        SqliteStore::open(&copy).unwrap().dump().unwrap(),
        store.dump().unwrap()
    );
}

/// GIVEN a store with meta key 'indexed_ref' set
/// WHEN  it is read back and an absent key is read
/// THEN  the value is returned and the absent key is None
#[test]
fn test_given_meta_when_set_then_get_returns_value_and_none_for_absent() {
    let store = SqliteStore::open_in_memory().unwrap();
    store.set_meta("indexed_ref", "main").unwrap();
    assert_eq!(
        store.get_meta("indexed_ref").unwrap().as_deref(),
        Some("main")
    );
    assert_eq!(store.get_meta("nope").unwrap(), None);
}

fn task_note(ancora: &str, task: &str) -> NewNote {
    NewNote {
        tipo: "regra".into(),
        ancora: ancora.into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: "Regra da task.".into(),
        task: Some(task.into()),
        confirm: false,
        substitui: None,
        aliases: Vec::new(),
    }
}

/// GIVEN two task notes of 'lend-1': one anchored on a symbol in the base and one on a symbol only in the feature
/// WHEN  the task is promoted against the base graph
/// THEN  the first becomes scope 'repo' and stays proposta, the second becomes 'orfa'
#[test]
fn test_given_task_promoted_when_base_reindexed_then_notes_scope_repo() {
    let repo = Repo::fixture();
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let mut feature = SqliteStore::open_in_memory().unwrap();
    let source = std::fs::read_to_string(repo.path().join(PATH)).unwrap();
    repo.write(
        PATH,
        &format!("{source}\n\ndef only_in_feature():\n    pass\n"),
    );
    index(&WorkTree::new(repo.path()), &mut feature).unwrap();
    let kept = notes
        .add(
            &feature,
            task_note("sym:CalculateLimitUseCase.execute", "lend-1"),
            NOW,
        )
        .unwrap()
        .note;
    let lost = notes
        .add(&feature, task_note("sym:only_in_feature", "lend-1"), NOW)
        .unwrap()
        .note;

    repo.write(PATH, &source);
    let mut base = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut base).unwrap();
    let report = notes.promote(&base, "lend-1").unwrap();

    assert_eq!(report.promoted.len(), 1);
    assert_eq!(report.orphaned.len(), 1);
    let kept = notes.get(&kept.id).unwrap();
    assert_eq!(
        (kept.escopo.as_str(), kept.estado.as_str()),
        ("repo", "proposta")
    );
    assert_eq!(notes.get(&lost.id).unwrap().estado, "orfa");
}

/// GIVEN notes scoped to 'lend-1', to 'lend-2' and to the repo
/// WHEN  the task 'lend-1' is dropped
/// THEN  two notes remain and the count removed is 1
#[test]
fn test_given_dropped_task_when_notes_dropped_then_only_task_notes_removed() {
    let repo = Repo::fixture();
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let mut graph = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut graph).unwrap();
    for task in ["lend-1", "lend-2"] {
        notes
            .add(
                &graph,
                task_note("sym:CalculateLimitUseCase.execute", task),
                NOW,
            )
            .unwrap();
    }
    let mut repo_note = task_note("sym:CalculateLimitUseCase.execute", "x");
    repo_note.task = None;
    notes.add(&graph, repo_note, NOW).unwrap();
    assert_eq!(notes.drop_task("lend-1").unwrap(), 1);
    assert_eq!(notes.list().unwrap().len(), 2);
}
