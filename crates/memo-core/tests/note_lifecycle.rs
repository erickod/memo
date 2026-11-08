mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::index_with_notes;
use memo_core::notes::{NewNote, NoteDir};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

const PATH: &str = "app/billing/application/usecases/calculate_limit_usecase.py";
const NOW: &str = "2026-11-07T14:05:00Z";

struct Env {
    repo: Repo,
    store: SqliteStore,
    notes: NoteDir,
    config: Config,
}

fn env(config: &str) -> Env {
    let repo = Repo::fixture();
    let config = Config::from_toml(config).unwrap();
    let notes = NoteDir::new(&repo.path().join(".git/memo/notes"));
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with_notes(&WorkTree::new(repo.path()), &mut store, &config, &notes).unwrap();
    Env {
        repo,
        store,
        notes,
        config,
    }
}

impl Env {
    fn add(&self, ancora: &str, confirm: bool) -> String {
        let new = NewNote {
            tipo: "regra".into(),
            ancora: ancora.into(),
            secundarias: Vec::new(),
            origem: "humano".into(),
            text: "Limite é inteiro.".into(),
            task: None,
            confirm,
            substitui: None,
            aliases: Vec::new(),
        };
        self.notes.add(&self.store, new, NOW).unwrap().note.id
    }

    fn edit(&self, from: &str, to: &str) {
        let source = std::fs::read_to_string(self.repo.path().join(PATH)).unwrap();
        assert!(source.contains(from), "fixture changed: {from}");
        self.repo.write(PATH, &source.replace(from, to));
    }

    fn reindex(&mut self) -> memo_core::index::IndexReport {
        let tree = WorkTree::new(self.repo.path());
        index_with_notes(&tree, &mut self.store, &self.config, &self.notes).unwrap()
    }

    fn kinds(&self) -> Vec<String> {
        self.store
            .events()
            .unwrap()
            .into_iter()
            .map(|e| e.kind)
            .collect()
    }
}

const EXECUTE: &str = "sym:CalculateLimitUseCase.execute";

/// GIVEN a confirmada note on execute and execute renamed to 'compute' with the same body
/// WHEN  the repository is reindexed
/// THEN  the note is re-anchored on '...compute', stays confirmada and is not stale
#[test]
fn test_given_renamed_function_same_body_when_indexed_then_symbol_renamed_note_reanchored() {
    let mut e = env("");
    let id = e.add(EXECUTE, true);
    e.edit("def execute(", "def compute(");
    e.reindex();
    let note = e.notes.get(&id).unwrap();
    assert!(
        note.ancora.ends_with("CalculateLimitUseCase.compute"),
        "{}",
        note.ancora
    );
    assert_eq!(note.estado, "confirmada");
}

/// GIVEN a confirmada note on execute and a changed signature
/// WHEN  the repository is reindexed
/// THEN  the note is 'stale' and keeps 'confirmada' as estado_anterior
#[test]
fn test_given_note_when_signature_changes_then_stale_with_estado_anterior() {
    let mut e = env("");
    let id = e.add(EXECUTE, true);
    e.edit("rate: int)", "rate: int, extra=0)");
    e.reindex();
    let note = e.notes.get(&id).unwrap();
    assert_eq!(note.estado, "stale");
    assert_eq!(note.estado_anterior.as_deref(), Some("confirmada"));
}

/// GIVEN a stale note after a signature change
/// WHEN  'recheck' runs
/// THEN  the note is confirmada again, estado_anterior is cleared and the hashes are current
#[test]
fn test_given_changed_signature_when_indexed_then_note_stale_and_recheck_restores() {
    let mut e = env("");
    let id = e.add(EXECUTE, true);
    let old = e.notes.get(&id).unwrap();
    e.edit("rate: int)", "rate: int, extra=0)");
    e.reindex();
    let fresh = e.notes.recheck(&e.store, &id).unwrap();
    assert_eq!(fresh.estado, "confirmada");
    assert_eq!(fresh.estado_anterior, None);
    assert_ne!(fresh.ancora_sig, old.ancora_sig);
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "confirmada");
}

/// GIVEN stale_on = ["sig"] and a body-only change
/// WHEN  the repository is reindexed
/// THEN  the note stays fresh
#[test]
fn test_given_body_only_change_when_stale_on_sig_then_note_stays_fresh() {
    let mut e = env("stale_on = [\"sig\"]\n");
    let id = e.add(EXECUTE, true);
    e.edit("amount * rate", "amount + rate");
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "confirmada");
}

/// GIVEN a note on execute and execute deleted together with its body
/// WHEN  the repository is reindexed
/// THEN  the note is 'orfa'
#[test]
fn test_given_removed_symbol_without_pair_when_reindexed_then_note_orfa() {
    let mut e = env("");
    let id = e.add(EXECUTE, true);
    e.edit("    def execute(self, amount, rate: int) -> int:", "    def gone(self):\n        return 0\n\n    def execute_removed(self, amount, rate: int) -> int:");
    e.edit("limit = amount * rate", "limit = amount - rate");
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
}

/// GIVEN a note anchored on the word 'checkout' and the checkout file deleted
/// WHEN  the repository is reindexed
/// THEN  the note is 'orfa' because the word lost its last occurrence
#[test]
fn test_given_word_anchor_loses_all_occurrences_when_reindexed_then_note_orfa() {
    let mut e = env("min_specificity = 0.0\nskeleton = []\n");
    let id = e.add("word:checkout", false);
    std::fs::remove_file(
        e.repo
            .path()
            .join("app/billing/application/usecases/checkout_usecase.py"),
    )
    .unwrap();
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
}

/// GIVEN an edge note execute->LimitRepository.save and the call removed from execute
/// WHEN  the repository is reindexed
/// THEN  the note is 'orfa' when the endpoints kept their signatures
#[test]
fn test_given_edge_anchor_removed_when_reindexed_then_orfa() {
    let mut e = env("");
    let id = e.add(
        "edge:CalculateLimitUseCase.execute->LimitRepository.save",
        false,
    );
    e.edit("self._repo.save(limit)", "pass");
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
}

/// GIVEN an edge note and the call removed while execute also gains a parameter
/// WHEN  the repository is reindexed
/// THEN  the note is 'stale' because one endpoint changed signature
#[test]
fn test_given_edge_gone_and_endpoint_signature_changed_when_reindexed_then_stale() {
    let mut e = env("");
    let id = e.add(
        "edge:CalculateLimitUseCase.execute->LimitRepository.save",
        false,
    );
    e.edit("self._repo.save(limit)", "pass");
    e.edit("rate: int)", "rate: int, extra=0)");
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "stale");
}

/// GIVEN a file note on the checkout file and the file deleted
/// WHEN  the repository is reindexed
/// THEN  the note is 'orfa'
#[test]
fn test_given_file_anchor_deleted_when_reindexed_then_note_orfa() {
    let mut e = env("");
    let id = e.add(
        "file:app/billing/application/usecases/checkout_usecase.py",
        false,
    );
    std::fs::remove_file(
        e.repo
            .path()
            .join("app/billing/application/usecases/checkout_usecase.py"),
    )
    .unwrap();
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
}

/// GIVEN an orfa note
/// WHEN  the repository is reindexed again with no further change
/// THEN  the note file still exists with state 'orfa' (never deleted by reindexing)
#[test]
fn test_given_orfa_note_when_reindexed_again_then_file_kept_and_state_unchanged() {
    let mut e = env("");
    let id = e.add(
        "file:app/billing/application/usecases/checkout_usecase.py",
        false,
    );
    std::fs::remove_file(
        e.repo
            .path()
            .join("app/billing/application/usecases/checkout_usecase.py"),
    )
    .unwrap();
    e.reindex();
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
    assert_eq!(e.notes.list().unwrap().len(), 1);
}

/// GIVEN one note going stale and one going orfa in the same reindexing
/// WHEN  the events are read
/// THEN  NOTE_STALE and NOTE_ORPHANED are recorded and the summary counts 1 stale note
#[test]
fn test_given_note_transitions_when_reindexed_then_note_events_recorded() {
    let mut e = env("");
    e.add(EXECUTE, true);
    e.add(
        "file:app/billing/application/usecases/checkout_usecase.py",
        false,
    );
    std::fs::remove_file(
        e.repo
            .path()
            .join("app/billing/application/usecases/checkout_usecase.py"),
    )
    .unwrap();
    e.edit("rate: int)", "rate: int, extra=0)");
    let report = e.reindex();
    let kinds = e.kinds();
    assert!(kinds.contains(&"NOTE_STALE".to_string()));
    assert!(kinds.contains(&"NOTE_ORPHANED".to_string()));
    assert_eq!(report.events.notes_stale, 1);
}

/// GIVEN a repository with notes already reindexed
/// WHEN  it is reindexed again without any change
/// THEN  nothing is written and no event is added
#[test]
fn test_given_unchanged_repo_when_reindexed_with_notes_then_no_writes_no_events() {
    let mut e = env("");
    e.add(EXECUTE, true);
    e.reindex();
    let (changes, events) = (e.store.total_changes(), e.store.events().unwrap().len());
    let report = e.reindex();
    assert!(!report.wrote);
    assert_eq!(e.store.total_changes(), changes);
    assert_eq!(e.store.events().unwrap().len(), events);
}

/// GIVEN a note added with confirm, then a second note superseding it
/// WHEN  the events are read
/// THEN  NOTE_ADDED, NOTE_CONFIRMED and NOTE_SUPERSEDED were recorded
#[test]
fn test_given_note_flow_when_events_read_then_added_confirmed_superseded_recorded() {
    let e = env("");
    let old = e.add(EXECUTE, true);
    let new = NewNote {
        tipo: "regra".into(),
        ancora: EXECUTE.into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: "Nova regra.".into(),
        task: None,
        confirm: false,
        substitui: Some(old),
        aliases: Vec::new(),
    };
    e.notes.add(&e.store, new, NOW).unwrap();
    let kinds = e.kinds();
    for kind in ["NOTE_ADDED", "NOTE_CONFIRMED", "NOTE_SUPERSEDED"] {
        assert!(
            kinds.contains(&kind.to_string()),
            "{kind} missing in {kinds:?}"
        );
    }
}

/// GIVEN a note word:checkout and 'checkout' left only in a call text (the module file removed)
/// WHEN  the repository is reindexed
/// THEN  the note is 'orfa': use words do not keep a word anchor alive (TASK-6, D16)
#[test]
fn test_given_word_anchor_only_in_use_text_when_reindexed_then_note_orfa() {
    let mut e = env("min_specificity = 0.0\nskeleton = []\n");
    let id = e.add("word:checkout", false);
    std::fs::remove_file(
        e.repo
            .path()
            .join("app/billing/application/usecases/checkout_usecase.py"),
    )
    .unwrap();
    e.edit(
        "        self._repo.save(limit)\n",
        "        self._repo.save(limit)\n        checkout_now()\n",
    );
    e.reindex();
    assert_eq!(e.notes.get(&id).unwrap().estado, "orfa");
}

/// GIVEN a word that only appears in a call text
/// WHEN  a note anchored on it is added
/// THEN  the anchor is not found (TASK-6, D16)
#[test]
fn test_given_word_anchor_only_in_use_text_when_added_then_not_found() {
    let mut e = env("min_specificity = 0.0\nskeleton = []\n");
    e.edit(
        "        self._repo.save(limit)\n",
        "        self._repo.save(limit)\n        zebra_call()\n",
    );
    e.reindex();
    let new = NewNote {
        tipo: "regra".into(),
        ancora: "word:zebra".into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: "Zebra.".into(),
        task: None,
        confirm: false,
        substitui: None,
        aliases: Vec::new(),
    };
    let got = e.notes.add(&e.store, new, NOW);
    assert!(
        matches!(got, Err(memo_core::error::CoreError::NotFound(_))),
        "{got:?}"
    );
}
