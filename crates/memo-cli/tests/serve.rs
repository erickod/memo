mod common;

use common::Repo;
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn memo_serve(repo: &Repo, requests: &[Value]) -> (i32, Vec<Value>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_memo"))
        .env("MEMO_NOW", "2026-11-07T14:05:00Z")
        .arg("--repo")
        .arg(repo.path())
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let replies = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (output.status.code().unwrap(), replies)
}

fn cli(repo: &Repo, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_memo"))
        .env("MEMO_NOW", "2026-11-07T14:05:00Z")
        .arg("--repo")
        .arg(repo.path())
        .args(args)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
}

/// GIVEN the fixture
/// WHEN  brief and query symbol are called through 'memo serve'
/// THEN  each tool text is identical to the output of the matching CLI command
#[test]
fn test_given_tool_call_when_serve_then_text_equals_cli_output() {
    let repo = Repo::fixture();
    let (code, replies) = memo_serve(
        &repo,
        &[
            call(1, "brief", json!({"text":"calculate limit"})),
            call(
                2,
                "query",
                json!({"kind":"symbol","target":"CalculateLimitUseCase.execute"}),
            ),
        ],
    );
    assert_eq!(code, 0);
    let text = |i: usize| {
        replies[i]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(text(0), cli(&repo, &["brief", "calculate limit"]));
    assert_eq!(
        text(1),
        cli(&repo, &["query", "symbol", "CalculateLimitUseCase.execute"])
    );
    assert_eq!(replies[0]["result"]["isError"], false);
}

/// GIVEN the fixture
/// WHEN  query symbol is called for a symbol that does not exist
/// THEN  the result has isError true (the CLI would exit 2)
#[test]
fn test_given_tool_error_when_serve_then_is_error_true() {
    let repo = Repo::fixture();
    let (_, replies) = memo_serve(
        &repo,
        &[call(1, "query", json!({"kind":"symbol","target":"NoSuch"}))],
    );
    assert_eq!(replies[0]["result"]["isError"], true);
}

/// GIVEN the fixture
/// WHEN  note_add is called through MCP and the note is shown with the CLI
/// THEN  the note exists with origin 'agente'
#[test]
fn test_given_note_add_via_mcp_when_called_then_note_created_with_origin_agente() {
    let repo = Repo::fixture();
    let (_, replies) = memo_serve(
        &repo,
        &[call(
            1,
            "note_add",
            json!({
        "tipo":"regra","ancora":"sym:CalculateLimitUseCase.execute","texto":"Limite é inteiro."}),
        )],
    );
    let text = replies[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    let id = text.split_whitespace().nth(1).unwrap().to_string();
    assert!(cli(&repo, &["note", "show", &id]).contains("origem: agente"));
}

/// GIVEN a full handshake: initialize, initialized notification and tools/list
/// WHEN  the input ends
/// THEN  the process exits 0 with exactly two responses (the notification has none)
#[test]
fn test_given_handshake_when_serve_then_two_responses_and_exit_0() {
    let repo = Repo::fixture();
    let (code, replies) = memo_serve(
        &repo,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ],
    );
    assert_eq!(code, 0);
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1]["result"]["tools"].as_array().unwrap().len(), 6);
}

fn tool_text(reply: &Value) -> String {
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

/// GIVEN a note with 1.200 characters added through the CLI
/// WHEN  note_show and note_list are called through 'memo serve'
/// THEN  note_show returns the whole text and note_list equals `memo note list` (TASK-7, D1)
#[test]
fn test_given_long_note_when_note_show_via_mcp_then_full_text_and_list_matches_cli() {
    let repo = Repo::fixture();
    let long = "x".repeat(1_200);
    let added = cli(
        &repo,
        &[
            "note",
            "add",
            "--tipo",
            "regra",
            "--ancora",
            "sym:CalculateLimitUseCase.execute",
            "--origem",
            "humano",
            &long,
        ],
    );
    let id = added.split_whitespace().nth(1).unwrap().to_string();
    let (_, replies) = memo_serve(
        &repo,
        &[
            call(1, "note_show", json!({ "id": id })),
            call(2, "note_list", json!({})),
        ],
    );
    assert!(tool_text(&replies[0]).contains(&long));
    assert_eq!(tool_text(&replies[1]), cli(&repo, &["note", "list"]));
}

/// GIVEN no note with the id 'n-missing0'
/// WHEN  note_show is called through 'memo serve'
/// THEN  the result has isError true (TASK-7, D1)
#[test]
fn test_given_unknown_note_id_when_note_show_via_mcp_then_is_error() {
    let repo = Repo::fixture();
    let (_, replies) = memo_serve(
        &repo,
        &[call(1, "note_show", json!({ "id": "n-missing0" }))],
    );
    assert_eq!(replies[0]["result"]["isError"], true);
}
