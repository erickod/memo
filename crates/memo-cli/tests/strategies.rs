mod common;

use common::Repo;
use std::process::{Command, Output};

fn memo(repo: &Repo, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_memo"))
        .arg("--repo")
        .arg(repo.path())
        .args(args)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

/// Endpoint → service → `Repo.run`, `run` defined in four more classes (generic name), a few
/// unresolved `x.run()` uses, and a test that requests the route.
fn flow_repo() -> Repo {
    let repo = Repo::empty();
    repo.write(
        "api.py",
        "import svc\n\n@router.get('/items')\ndef list_items():\n    return svc.list_items()\n",
    );
    repo.write(
        "svc.py",
        "from repo import Repo\n\ndef list_items():\n    return Repo().run()\n",
    );
    repo.write("repo.py", "class Repo:\n    def run(self):\n        pass\n");
    let others: String = (0..4)
        .map(|i| format!("class C{i}:\n    def run(self):\n        pass\n\n"))
        .collect();
    repo.write("others.py", &others);
    repo.write(
        "uses.py",
        "def a(x):\n    x.run()\n\ndef b(y):\n    y.run()\n",
    );
    repo.write(
        "tests/test_api.py",
        "def test_list(client):\n    client.get('/items')\n",
    );
    repo.commit_all();
    repo
}

/// GIVEN the flow repository
/// WHEN impact runs on repo.Repo.run with default flags
/// THEN the output shows the weights line, the service and endpoint first with their strategy
///      weights, the generic name-only uses as one count line, and the hidden tests by kind.
#[test]
fn test_given_generic_target_when_impact_then_weights_relevant_and_summaries() {
    let out = stdout(&memo(&flow_repo(), &["query", "impact", "repo.Repo.run"]));
    assert!(out.contains("nome: run · 5 definições"), "{out}");
    assert!(out.contains("genérico"), "{out}");
    assert!(
        out.contains("pesos: vizinhança 1.00 · caminho 1.00 · conexão 0.50 · nome 0.25"),
        "{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let at = lines.iter().position(|l| *l == "relevantes:").unwrap();
    assert!(
        lines[at + 1].contains("F list_items") && lines[at + 1].contains("svc.py"),
        "{out}"
    );
    assert!(lines[at + 1].contains("[viz 1.00 · cam 1.00"), "{out}");
    assert!(lines[at + 2].contains("entrada GET /items"), "{out}");
    assert!(
        out.contains("só nome omitidos: nome genérico (run, 5 definições); --all lista"),
        "{out}"
    );
    assert!(
        out.contains("testes ocultos: 0 exatos · 1 via HTTP"),
        "{out}"
    );
}

/// GIVEN the flow repository
/// WHEN impact runs with --all
/// THEN the name-only uses are listed instead of summarized.
#[test]
fn test_given_all_flag_when_impact_generic_target_then_name_only_listed() {
    let out = stdout(&memo(
        &flow_repo(),
        &["--all", "query", "impact", "repo.Repo.run"],
    ));
    assert!(!out.contains("só nome omitidos"), "{out}");
    assert!(out.contains("só nome"), "{out}");
}

/// GIVEN `list_items` defined in the endpoint and in the service, linked by an exact call
/// WHEN impact and brief receive the bare name
/// THEN both succeed: impact prints the group caller first, brief tags both as explicit symbols.
#[test]
fn test_given_linked_homonyms_when_impact_and_brief_then_group_not_ambiguity() {
    let repo = flow_repo();
    let out = memo(&repo, &["query", "impact", "list_items"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("grupo: api.list_items -> svc.list_items (ligadas por chamada exata)"),
        "{text}"
    );
    assert_eq!(text.matches("impact list_items").count(), 2, "{text}");
    let json: serde_json::Value = serde_json::from_str(&stdout(&memo(
        &repo,
        &["--json", "query", "impact", "list_items"],
    )))
    .unwrap();
    assert_eq!(json["data"]["grupo"][0], "api.list_items");
    let brief = stdout(&memo(&repo, &["brief", "list_items"]));
    assert_eq!(brief.matches("[simbolo]").count(), 2, "{brief}");
}

/// GIVEN `[strategies]` with path weight 0 in memo.toml
/// WHEN impact runs
/// THEN the weights line shows it and no relevant item carries a `cam` weight.
#[test]
fn test_given_strategies_config_when_impact_then_weights_from_config() {
    let repo = flow_repo();
    repo.write(".git/memo/memo.toml", "[strategies]\npath = 0.0\n");
    let out = stdout(&memo(&repo, &["query", "impact", "repo.Repo.run"]));
    assert!(out.contains("caminho 0.00"), "{out}");
    assert!(!out.contains("cam 1.00"), "{out}");
}

/// GIVEN the flow repository
/// WHEN query tests runs on the repository method
/// THEN the HTTP test is listed under 'testes via HTTP' with the path and the endpoint.
#[test]
fn test_given_http_test_when_query_tests_then_via_http_section() {
    let out = stdout(&memo(&flow_repo(), &["query", "tests", "repo.Repo.run"]));
    let at = out.find("testes via HTTP:").expect(&out);
    assert!(out[at..].contains("T test_list"), "{out}");
    assert!(out[at..].contains("/items -> list_items"), "{out}");
}
