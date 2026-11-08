mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::{index_with, index_with_notes};
use memo_core::notes::NoteDir;
use memo_core::source::WorkTree;
use memo_core::store::{EventRow, SqliteStore};

pub const PATH: &str = "app/billing/application/usecases/calculate_limit_usecase.py";
pub const EXECUTE_Q: &str =
    "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute";

fn setup() -> (Repo, SqliteStore) {
    let repo = Repo::fixture();
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = WorkTree::new(repo.path());
    index_with(&tree, &mut store, &Config::default()).unwrap();
    (repo, store)
}

fn edit(repo: &Repo, from: &str, to: &str) {
    let source = std::fs::read_to_string(repo.path().join(PATH)).unwrap();
    assert!(source.contains(from), "fixture changed: {from}");
    repo.write(PATH, &source.replace(from, to));
}

fn reindex(repo: &Repo, store: &mut SqliteStore) -> memo_core::index::IndexReport {
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    index_with_notes(
        &WorkTree::new(repo.path()),
        store,
        &Config::default(),
        &notes,
    )
    .unwrap()
}

fn of_kind(events: &[EventRow], kind: &str) -> Vec<EventRow> {
    events.iter().filter(|e| e.kind == kind).cloned().collect()
}

/// GIVEN the fixture indexed for the first time
/// WHEN  the events are read
/// THEN  every event is in batch 'b-1' and the execute method has SYMBOL_CREATED
#[test]
fn test_given_first_index_when_events_then_symbol_created_in_one_batch() {
    let (_r, store) = setup();
    let events = store.events().unwrap();
    assert!(events.iter().all(|e| e.batch == "b-1"));
    let created = of_kind(&events, "SYMBOL_CREATED");
    assert!(
        created
            .iter()
            .any(|e| e.subject.as_deref() == Some(EXECUTE_Q))
    );
    assert!(events.windows(2).all(|w| w[0].seq < w[1].seq));
}

/// GIVEN execute gains a parameter 'extra=0'
/// WHEN  the repository is reindexed
/// THEN  one SYMBOL_SIGNATURE_CHANGED event names execute and its payload carries both hashes
#[test]
fn test_given_changed_signature_when_reindexed_then_signature_event_with_hashes() {
    let (repo, mut store) = setup();
    edit(&repo, "rate: int)", "rate: int, extra=0)");
    reindex(&repo, &mut store);
    let events = of_kind(&store.events().unwrap(), "SYMBOL_SIGNATURE_CHANGED");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subject.as_deref(), Some(EXECUTE_Q));
    let payload = events[0].payload.clone().unwrap();
    assert!(
        payload.contains("before") && payload.contains("after"),
        "{payload}"
    );
}

/// GIVEN execute changes 'amount * rate' to 'amount + rate'
/// WHEN  the repository is reindexed
/// THEN  execute has one SYMBOL_BODY_CHANGED (its class and module change too) and no SIGNATURE one
#[test]
fn test_given_changed_body_only_when_reindexed_then_body_changed_event_only() {
    let (repo, mut store) = setup();
    edit(&repo, "amount * rate", "amount + rate");
    reindex(&repo, &mut store);
    let events = store.events().unwrap();
    let body: Vec<EventRow> = of_kind(&events, "SYMBOL_BODY_CHANGED")
        .into_iter()
        .filter(|e| e.subject.as_deref() == Some(EXECUTE_Q))
        .collect();
    assert_eq!(body.len(), 1);
    assert!(of_kind(&events, "SYMBOL_SIGNATURE_CHANGED").is_empty());
}

/// GIVEN only the comment inside execute changes
/// WHEN  the repository is reindexed
/// THEN  no symbol event is added
#[test]
fn test_given_comment_change_when_reindexed_then_no_symbol_events() {
    let (repo, mut store) = setup();
    let before = store.events().unwrap().len();
    edit(
        &repo,
        "comment kept stable for hash tests",
        "another comment",
    );
    reindex(&repo, &mut store);
    let added: Vec<EventRow> = store.events().unwrap().into_iter().skip(before).collect();
    assert!(
        added.iter().all(|e| !e.kind.starts_with("SYMBOL_")),
        "{added:?}"
    );
}

/// GIVEN a new first line is added to the file, shifting every line number
/// WHEN  the repository is reindexed
/// THEN  no SYMBOL_* event is recorded (line is an attribute, not identity)
#[test]
fn test_given_line_shift_only_when_reindexed_then_no_symbol_events() {
    let (repo, mut store) = setup();
    let before = store.events().unwrap().len();
    let source = std::fs::read_to_string(repo.path().join(PATH)).unwrap();
    repo.write(PATH, &format!("# shifted\n{source}"));
    reindex(&repo, &mut store);
    let added: Vec<EventRow> = store.events().unwrap().into_iter().skip(before).collect();
    assert!(
        added.iter().all(|e| !e.kind.starts_with("SYMBOL_")),
        "{added:?}"
    );
}

/// GIVEN execute is renamed to 'compute' with the same signature and body
/// WHEN  the repository is reindexed
/// THEN  there is one SYMBOL_RENAMED with payload de/para and no CREATED or DESTROYED for it
#[test]
fn test_given_renamed_function_same_body_when_reindexed_then_renamed_event_with_payload() {
    let (repo, mut store) = setup();
    let before = store.events().unwrap().len();
    edit(&repo, "def execute(", "def compute(");
    reindex(&repo, &mut store);
    let added: Vec<EventRow> = store.events().unwrap().into_iter().skip(before).collect();
    let renamed = of_kind(&added, "SYMBOL_RENAMED");
    assert_eq!(renamed.len(), 1);
    let payload = renamed[0].payload.clone().unwrap();
    assert!(
        payload.contains("\"de\"") && payload.contains("\"para\""),
        "{payload}"
    );
    assert!(payload.contains(EXECUTE_Q) && payload.contains("CalculateLimitUseCase.compute"));
    let touched = |kind: &str| {
        of_kind(&added, kind)
            .iter()
            .any(|e| e.subject.as_deref() == Some(EXECUTE_Q))
    };
    assert!(!touched("SYMBOL_DESTROYED") && !touched("SYMBOL_CREATED"));
}

/// GIVEN execute is renamed to 'compute' and its body also changes
/// WHEN  the repository is reindexed
/// THEN  the old name has SYMBOL_DESTROYED and the new name SYMBOL_CREATED, with no rename
#[test]
fn test_given_renamed_function_with_body_change_when_reindexed_then_destroyed_and_created() {
    let (repo, mut store) = setup();
    let before = store.events().unwrap().len();
    edit(&repo, "def execute(", "def compute(");
    edit(&repo, "amount * rate", "amount + rate");
    reindex(&repo, &mut store);
    let added: Vec<EventRow> = store.events().unwrap().into_iter().skip(before).collect();
    assert!(of_kind(&added, "SYMBOL_RENAMED").is_empty());
    let subjects = |kind: &str| -> Vec<String> {
        of_kind(&added, kind)
            .into_iter()
            .filter_map(|e| e.subject)
            .collect()
    };
    assert!(subjects("SYMBOL_DESTROYED").contains(&EXECUTE_Q.to_string()));
    assert!(
        subjects("SYMBOL_CREATED")
            .iter()
            .any(|s| s.ends_with("CalculateLimitUseCase.compute"))
    );
}

/// GIVEN two functions with identical signature and body are both renamed
/// WHEN  the repository is reindexed
/// THEN  the pairing is ambiguous, so no SYMBOL_RENAMED is recorded
#[test]
fn test_given_ambiguous_rename_pair_when_reindexed_then_no_rename_event() {
    let repo = Repo::empty();
    repo.write("m.py", "def a():\n    return 1\n\ndef b():\n    return 1\n");
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = WorkTree::new(repo.path());
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let before = store.events().unwrap().len();
    repo.write("m.py", "def c():\n    return 1\n\ndef d():\n    return 1\n");
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let added: Vec<EventRow> = store.events().unwrap().into_iter().skip(before).collect();
    assert!(of_kind(&added, "SYMBOL_RENAMED").is_empty(), "{added:?}");
    assert_eq!(of_kind(&added, "SYMBOL_DESTROYED").len(), 2);
}

/// GIVEN a file defining the word 'zebra' that is later removed
/// WHEN  the repository is reindexed
/// THEN  WORD_CREATED for 'zebra' exists at first and WORD_DESTROYED after removal
#[test]
fn test_given_word_lifecycle_when_reindexed_then_word_created_and_destroyed() {
    let repo = Repo::empty();
    repo.write("z.py", "def zebra():\n    pass\n");
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = WorkTree::new(repo.path());
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let created = of_kind(&store.events().unwrap(), "WORD_CREATED");
    assert!(
        created
            .iter()
            .any(|e| e.subject.as_deref() == Some("zebra"))
    );
    repo.write("z.py", "def other():\n    pass\n");
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let destroyed = of_kind(&store.events().unwrap(), "WORD_DESTROYED");
    assert!(
        destroyed
            .iter()
            .any(|e| e.subject.as_deref() == Some("zebra"))
    );
}

/// GIVEN a rename of execute to 'compute' with unchanged body
/// WHEN  the report summary line is built
/// THEN  it reads 'eventos: +0 símbolos, ~2 alterados, 1 renomeado, 0 notas stale'
///       (the enclosing class and module bodies change when a method is renamed)
#[test]
fn test_given_rename_when_summary_then_line_matches_spec_format() {
    let (repo, mut store) = setup();
    edit(&repo, "def execute(", "def compute(");
    let report = reindex(&repo, &mut store);
    assert_eq!(
        report.events.line(),
        "eventos: +0 símbolos, ~2 alterados, 1 renomeado, 0 notas stale"
    );
}

/// GIVEN the word 'stripe', which exists only through the import 'from stripe.api import Charge'
/// WHEN  the import is removed and the repository is reindexed
/// THEN  WORD_CREATED for 'stripe' exists at first and WORD_DESTROYED for it after the removal
#[test]
fn test_given_word_only_in_import_when_import_removed_then_word_destroyed_event() {
    let repo = Repo::empty();
    repo.write(
        "m.py",
        "from stripe.api import Charge\n\ndef pay():\n    pass\n",
    );
    let mut store = SqliteStore::open_in_memory().unwrap();
    let tree = WorkTree::new(repo.path());
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let created = of_kind(&store.events().unwrap(), "WORD_CREATED");
    assert!(
        created
            .iter()
            .any(|e| e.subject.as_deref() == Some("stripe"))
    );
    repo.write("m.py", "def pay():\n    pass\n");
    index_with(&tree, &mut store, &Config::default()).unwrap();
    let destroyed = of_kind(&store.events().unwrap(), "WORD_DESTROYED");
    assert!(
        destroyed
            .iter()
            .any(|e| e.subject.as_deref() == Some("stripe"))
    );
}
