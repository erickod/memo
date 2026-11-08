mod common;

use common::Repo;
use serde_json::Value;
use std::process::{Command, Output};

fn memo(repo: &Repo, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_memo"))
        .arg("--repo")
        .arg(repo.path())
        .args(args)
        .output()
        .unwrap()
}

fn json(out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

/// GIVEN a base and a derived feature with an unrelated changed symbol
/// WHEN brief runs with task context and an explicit target
/// THEN the explicit target precedes the changed symbol and full bypasses session suppression.
#[test]
fn test_given_feature_when_brief_then_explicit_and_changed_code_are_ranked() {
    let repo = Repo::empty();
    repo.write(
        "m.py",
        "def target():\n    return 1\n\ndef changed():\n    return 0\n",
    );
    repo.commit_all();
    assert!(memo(&repo, &["index"]).status.success());
    repo.write(
        "m.py",
        "def target():\n    return 1\n\ndef changed():\n    return 2\n",
    );
    assert!(memo(&repo, &["derive", "feature-rank"]).status.success());
    let first = json(memo(
        &repo,
        &["--task", "feature-rank", "--json", "brief", "m.target"],
    ));
    let symbols = first["data"]["symbols"].as_array().unwrap();
    assert_eq!(symbols[0]["addr"]["qname"], "m.target");
    assert_eq!(symbols[1]["addr"]["qname"], "m.changed");
    assert!(
        symbols[1]["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "alterado")
    );
    let repeated = json(memo(
        &repo,
        &["--task", "feature-rank", "--json", "brief", "m.target"],
    ));
    assert!(repeated["data"]["symbols"].as_array().unwrap().is_empty());
    let full = json(memo(
        &repo,
        &[
            "--task",
            "feature-rank",
            "--full",
            "--json",
            "brief",
            "m.target",
        ],
    ));
    assert_eq!(full["data"]["symbols"], first["data"]["symbols"]);
    assert_eq!(full["data"]["ler"], first["data"]["ler"]);
}

/// GIVEN more than ten approximate readers and one exact reader sorted last by name
/// WHEN the default text groups the list
/// THEN the exact reader remains visible above the grouped candidates.
#[test]
fn test_given_grouped_readers_when_rendered_then_exact_result_is_not_hidden() {
    let repo = Repo::empty();
    repo.write("models.py", "class Record:\n    state: str = 'new'\n");
    let mut source = String::new();
    for n in 0..12 {
        source.push_str(&format!("def read_{n}(x):\n    return x.state\n\n"));
    }
    repo.write("a.py", &source);
    repo.write(
        "z.py",
        "from models import Record\n\ndef exact():\n    return Record.state\n",
    );
    repo.commit_all();
    let out = memo(&repo, &["query", "readers", "Record.state"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.find("exact").unwrap() < text.find("read_0").unwrap(),
        "{text}"
    );
    assert!(text.contains("+3 em:"), "{text}");
}

/// GIVEN two indexed files with the same basename
/// WHEN brief receives only that basename
/// THEN ambiguity is reported with both candidates and exit code two.
#[test]
fn test_given_ambiguous_path_when_brief_then_candidates_and_exit_two() {
    let repo = Repo::empty();
    for path in ["a/entities.py", "b/entities.py"] {
        repo.write(path, "class Record:\n    pass\n");
    }
    repo.commit_all();
    let out = memo(&repo, &["brief", "entities.py"]);
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("a/entities.py") && text.contains("b/entities.py"),
        "{text}"
    );
}
