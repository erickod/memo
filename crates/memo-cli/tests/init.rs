mod common;

use common::Repo;
use std::process::{Command, Output};

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

fn read(repo: &Repo, rel: &str) -> String {
    std::fs::read_to_string(repo.path().join(rel)).unwrap()
}

fn committed() -> Repo {
    Repo::fixture()
}

/// GIVEN a committed repository
/// WHEN  'memo init claude --mcp' runs twice
/// THEN  both exit 0 and CLAUDE.md still has exactly one memo block (A13)
#[test]
fn test_given_init_twice_when_run_then_instruction_block_not_duplicated() {
    let repo = committed();
    assert_eq!(
        memo(&repo, &["init", "claude", "--mcp"]).status.code(),
        Some(0)
    );
    let first = (read(&repo, "CLAUDE.md"), read(&repo, ".mcp.json"));
    assert_eq!(
        memo(&repo, &["init", "claude", "--mcp"]).status.code(),
        Some(0)
    );
    assert_eq!((read(&repo, "CLAUDE.md"), read(&repo, ".mcp.json")), first);
    assert_eq!(
        read(&repo, "CLAUDE.md")
            .matches("<!-- memo:begin -->")
            .count(),
        1
    );
}

/// GIVEN a committed repository
/// WHEN  'memo init claude --mcp --print' runs
/// THEN  the golden plan is printed and nothing is written: no memo dir, no CLAUDE.md, no .mcp.json (A13)
#[test]
fn test_given_init_print_when_run_then_no_file_written() {
    let repo = committed();
    let o = memo(&repo, &["init", "claude", "--mcp", "--print"]);
    assert_eq!(o.status.code(), Some(0));
    insta::assert_snapshot!(out(&o));
    for rel in [".git/memo", "CLAUDE.md", ".mcp.json"] {
        assert!(!repo.path().join(rel).exists(), "{rel} was written");
    }
}

/// GIVEN a committed repository
/// WHEN  'memo init claude' runs, memo.toml is edited by hand and init runs again
/// THEN  memo.toml first holds the seeds and the manual edit survives the second init
#[test]
fn test_given_init_when_run_then_memo_toml_written_with_seeds_and_never_overwritten() {
    let repo = committed();
    memo(&repo, &["init", "claude"]);
    assert!(read(&repo, ".git/memo/memo.toml").contains("min_specificity = 0.35"));
    repo.write(".git/memo/memo.toml", "min_specificity = 0.5\n");
    memo(&repo, &["init", "claude"]);
    assert_eq!(
        read(&repo, ".git/memo/memo.toml"),
        "min_specificity = 0.5\n"
    );
}

/// GIVEN a committed repository on branch main with an uncommitted edit in the work tree
/// WHEN  'memo init claude' runs and then 'memo status'
/// THEN  the base is indexed from the ref: 'ref=main' with a commit, and the edit is not in it
#[test]
fn test_given_init_when_run_then_base_indexed_from_main_ref() {
    let repo = committed();
    let path = "app/billing/application/usecases/calculate_limit_usecase.py";
    let source = read(&repo, path);
    repo.write(path, &format!("{source}\n\ndef uncommitted():\n    pass\n"));
    assert_eq!(memo(&repo, &["init", "claude"]).status.code(), Some(0));
    let status = out(&memo(&repo, &["status"]));
    assert!(
        status.contains("ref=main") && !status.contains("commit=-"),
        "{status}"
    );
    let found = memo(&repo, &["--no-index", "query", "symbol", "uncommitted"]);
    assert_eq!(found.status.code(), Some(2));
}

/// GIVEN a committed repository
/// WHEN  'memo init codex --mcp' runs
/// THEN  AGENTS.md has the block and .codex/config.toml registers 'memo serve'
#[test]
fn test_given_init_codex_mcp_when_run_then_agents_md_and_codex_config_written() {
    let repo = committed();
    assert_eq!(
        memo(&repo, &["init", "codex", "--mcp"]).status.code(),
        Some(0)
    );
    assert!(read(&repo, "AGENTS.md").contains("<!-- memo:begin -->"));
    assert!(read(&repo, ".codex/config.toml").contains("[mcp_servers.memo]"));
}

/// GIVEN the harness ids 'agy' and 'zed'
/// WHEN  'memo init' runs for each
/// THEN  both exit 1 with a clear message on stderr and write nothing
#[test]
fn test_given_init_agy_or_unknown_harness_when_run_then_exit_1_and_nothing_written() {
    let repo = committed();
    for id in ["agy", "zed"] {
        let o = memo(&repo, &["init", id]);
        assert_eq!(o.status.code(), Some(1), "{id}");
        assert!(!String::from_utf8_lossy(&o.stderr).is_empty(), "{id}");
    }
    assert!(!repo.path().join(".git/memo").exists());
}

/// GIVEN a committed repository and the command run from another directory
/// WHEN  'memo init <repo> claude' runs with the repo as a positional argument
/// THEN  CLAUDE.md is created inside that repository
#[test]
fn test_given_init_positional_repo_when_run_elsewhere_then_installs_in_that_repo() {
    let repo = committed();
    let elsewhere = std::env::temp_dir();
    let o = Command::new(env!("CARGO_BIN_EXE_memo"))
        .current_dir(&elsewhere)
        .args(["init", repo.path().to_str().unwrap(), "claude"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(repo.path().join("CLAUDE.md").exists());
}

/// GIVEN a repository with no commits at all
/// WHEN  'memo init claude' runs
/// THEN  it still exits 0, installs the block and says the base is deferred
#[test]
fn test_given_repo_without_main_branch_when_init_then_installs_and_defers_base() {
    let repo = Repo::fixture_uncommitted();
    let o = memo(&repo, &["init", "claude"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(out(&o).contains("base: adiada"), "{}", out(&o));
    assert!(repo.path().join("CLAUDE.md").exists());
}
