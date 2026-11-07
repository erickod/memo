mod common;

use common::Repo;
use memo_core::lock::DbLock;
use std::process::{Command, Output};
use std::time::Duration;

const PY: &str = "app/billing/application/usecases/calculate_limit_usecase.py";

fn memo(repo: &Repo, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_memo"))
        .env("MEMO_NOW", "2026-11-07T14:05:00Z")
        .arg("--repo")
        .arg(repo.path())
        .args(args)
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn append_py(repo: &Repo, extra: &str) {
    let source = std::fs::read_to_string(repo.path().join(PY)).unwrap();
    repo.write(PY, &format!("{source}\n\n{extra}\n"));
}

fn db_bytes(repo: &Repo) -> Vec<u8> {
    std::fs::read(repo.path().join(".git/memo/base.db")).unwrap()
}

fn committed() -> Repo {
    let repo = Repo::fixture();
    repo.commit_all();
    repo
}

/// GIVEN a committed fixture, a base indexed from the work tree and a feature derived for 't1'
/// WHEN  a function is added to the work tree and queried with '--task t1' and, apart, in the base
/// THEN  the feature finds it, the base (--no-index) does not, and base.db is byte-identical (A11)
#[test]
fn test_given_derive_when_file_edited_then_only_feature_changes_base_db_identical() {
    let repo = committed();
    memo(&repo, &["index"]);
    assert_eq!(memo(&repo, &["derive", "t1"]).status.code(), Some(0));
    let base_before = db_bytes(&repo);
    append_py(&repo, "def only_in_feature():\n    pass");
    let feature = memo(
        &repo,
        &["--task", "t1", "query", "symbol", "only_in_feature"],
    );
    assert_eq!(feature.status.code(), Some(0), "{}", out(&feature));
    let base = memo(&repo, &["--no-index", "query", "symbol", "only_in_feature"]);
    assert_eq!(base.status.code(), Some(2));
    assert_eq!(db_bytes(&repo), base_before);
}

/// GIVEN a task id with no derived feature
/// WHEN  a query runs with '--task ghost'
/// THEN  it still answers from the base (exit 0)
#[test]
fn test_given_task_without_feature_when_query_then_reads_base() {
    let repo = committed();
    let o = memo(
        &repo,
        &[
            "--task",
            "ghost",
            "query",
            "symbol",
            "CalculateLimitUseCase.execute",
        ],
    );
    assert_eq!(o.status.code(), Some(0));
}

/// GIVEN a feature 't1' already derived
/// WHEN  'derive t1' runs again
/// THEN  it refuses with exit 1 and does not touch the feature
#[test]
fn test_given_existing_feature_when_derive_again_then_usage_error_exit_1() {
    let repo = committed();
    memo(&repo, &["derive", "t1"]);
    assert_eq!(memo(&repo, &["derive", "t1"]).status.code(), Some(1));
}

/// GIVEN a committed repo
/// WHEN  'memo index --ref main' runs and then 'memo status'
/// THEN  status shows 'ref=main' for the base
#[test]
fn test_given_index_ref_when_run_then_status_shows_ref_and_commit() {
    let repo = committed();
    assert_eq!(
        memo(&repo, &["index", "--ref", "main"]).status.code(),
        Some(0)
    );
    let status = out(&memo(&repo, &["status"]));
    assert!(status.contains("ref=main"), "{status}");
    assert!(!status.contains("commit=-"), "{status}");
}

/// GIVEN an indexed repository
/// WHEN  'memo index --full' runs
/// THEN  every file is re-extracted (alterados equals arquivos) and no symbol event is added
#[test]
fn test_given_index_full_when_run_then_all_files_reextracted_without_events() {
    let repo = committed();
    memo(&repo, &["index"]);
    let o = out(&memo(&repo, &["index", "--full"]));
    let line = o.lines().next().unwrap();
    let field = |name: &str| -> String {
        line.split_whitespace()
            .find_map(|f| f.strip_prefix(&format!("{name}=")))
            .unwrap()
            .to_string()
    };
    assert_eq!(field("arquivos"), field("alterados"), "{o}");
    assert!(
        o.contains("eventos: +0 símbolos, ~0 alterados, 0 renomeados, 0 notas stale"),
        "{o}"
    );
}

fn add_task_note(repo: &Repo, task: &str, ancora: &str) -> String {
    let o = memo(
        repo,
        &[
            "--task", task, "note", "add", "--tipo", "regra", "--ancora", ancora, "--origem",
            "humano", "Regra.",
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    out(&o).split_whitespace().nth(1).unwrap().to_string()
}

/// GIVEN a task 't1' with a note on a base symbol and a note on a feature-only symbol
/// WHEN  'memo promote t1' runs (main already holds the first symbol only)
/// THEN  the golden report counts 1 promoted and 1 orphan, and 'note list' shows scope repo for the first
#[test]
fn test_given_promote_when_task_notes_then_report_and_scope_repo() {
    let repo = committed();
    memo(&repo, &["derive", "t1"]);
    append_py(&repo, "def only_in_feature():\n    pass");
    let kept = add_task_note(&repo, "t1", "sym:CalculateLimitUseCase.execute");
    add_task_note(&repo, "t1", "sym:only_in_feature");
    let o = memo(&repo, &["promote", "t1"]);
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    insta::assert_snapshot!(out(&o));
    let shown = out(&memo(&repo, &["note", "show", &kept]));
    assert!(shown.contains("escopo: repo"), "{shown}");
}

/// GIVEN a derived task 't1' with one task note
/// WHEN  'memo drop t1' runs, then again
/// THEN  the first exits 0 and removes feature and note; the second exits 2
#[test]
fn test_given_drop_when_run_then_feature_and_task_notes_removed() {
    let repo = committed();
    memo(&repo, &["derive", "t1"]);
    let id = add_task_note(&repo, "t1", "sym:CalculateLimitUseCase.execute");
    assert_eq!(memo(&repo, &["drop", "t1"]).status.code(), Some(0));
    assert!(!repo.path().join(".git/memo/features/t1.db").exists());
    assert_eq!(memo(&repo, &["note", "show", &id]).status.code(), Some(2));
    assert_eq!(memo(&repo, &["drop", "t1"]).status.code(), Some(2));
}

/// GIVEN a derived task, a branch merged into main and a note
/// WHEN  'memo status' runs
/// THEN  the golden output lists base, features, notes by state, promotable tasks and coverage
#[test]
fn test_given_status_when_run_then_golden_lists_base_features_notes() {
    let repo = committed();
    memo(&repo, &["index"]);
    memo(&repo, &["derive", "t1"]);
    add_task_note(&repo, "t1", "sym:CalculateLimitUseCase.execute");
    let o = memo(&repo, &["status"]);
    assert_eq!(o.status.code(), Some(0));
    insta::assert_snapshot!(out(&o));
}

/// GIVEN the base.db lock held by another holder and a 200 ms wait limit
/// WHEN  'memo query' runs
/// THEN  it exits 3 (index unavailable)
#[test]
fn test_given_lock_held_when_memo_runs_then_exit_3() {
    let repo = committed();
    memo(&repo, &["index"]);
    let _held = DbLock::acquire(
        &repo.path().join(".git/memo/base.db.lock"),
        Duration::from_millis(100),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_memo"))
        .env("MEMO_LOCK_TIMEOUT_MS", "200")
        .arg("--repo")
        .arg(repo.path())
        .args(["query", "symbol", "CalculateLimitUseCase.execute"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
}

/// GIVEN a fresh repository and two 'memo index' processes started together
/// WHEN  both finish
/// THEN  both exit 0 and the execute method has exactly one SYMBOL_CREATED event
#[test]
fn test_given_two_concurrent_indexes_when_run_then_serialized_no_lost_events() {
    let repo = committed();
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_memo"))
            .arg("--repo")
            .arg(repo.path())
            .arg("index")
            .spawn()
            .unwrap()
    };
    let (mut a, mut b) = (spawn(), spawn());
    assert!(a.wait().unwrap().success() && b.wait().unwrap().success());
    let store =
        memo_core::store::SqliteStore::open(&repo.path().join(".git/memo/base.db")).unwrap();
    let created = store
        .events()
        .unwrap()
        .into_iter()
        .filter(|e| {
            e.kind == "SYMBOL_CREATED"
                && e.subject
                    .as_deref()
                    .is_some_and(|s| s.ends_with("CalculateLimitUseCase.execute"))
        })
        .count();
    assert_eq!(created, 1);
}

/// GIVEN a derived task 't1' and a branch 't1' already merged into main
/// WHEN  'memo status' runs
/// THEN  the promotable line is 'promovíveis: t1'
#[test]
fn test_given_merged_branch_when_status_then_task_listed_as_promotable() {
    let repo = committed();
    memo(&repo, &["derive", "t1"]);
    let made = Command::new("git")
        .args(["branch", "t1"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert!(made.success());
    let status = out(&memo(&repo, &["status"]));
    assert!(status.contains("promovíveis: t1"), "{status}");
}
