use memo_mcp::{Backend, Server, ToolResult};
use serde_json::{Value, json};
use std::cell::RefCell;

/// Backend de teste: guarda o argv recebido e devolve um resultado fixo.
struct Fake {
    seen: RefCell<Vec<Vec<String>>>,
    result: ToolResult,
}

impl Backend for &Fake {
    fn run(&self, argv: &[String]) -> ToolResult {
        self.seen.borrow_mut().push(argv.to_vec());
        self.result.clone()
    }
}

fn fake(text: &str, is_error: bool) -> Fake {
    Fake {
        seen: RefCell::new(Vec::new()),
        result: ToolResult {
            text: text.to_string(),
            is_error,
        },
    }
}

fn call(server: &Server<&Fake>, request: Value) -> Value {
    let reply = server.handle(&request.to_string()).expect("a response");
    serde_json::from_str(&reply).unwrap()
}

fn tool_call(name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":name,"arguments":arguments}})
}

fn argv(fake: &Fake) -> Vec<String> {
    fake.seen
        .borrow()
        .last()
        .cloned()
        .expect("backend was called")
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// GIVEN an initialize request asking for protocol version '2025-03-26'
/// WHEN  it is handled
/// THEN  the result echoes that version, declares tools and names the server 'memo'
#[test]
fn test_given_initialize_when_handled_then_server_info_capabilities_and_echoed_version() {
    let f = fake("", false);
    let server = Server::new(&f, None);
    let r = call(
        &server,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
        "params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}),
    );
    assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
    assert!(r["result"]["capabilities"]["tools"].is_object());
    assert_eq!(r["result"]["serverInfo"]["name"], "memo");
    assert_eq!(r["id"], 1);
}

/// GIVEN the notification 'notifications/initialized' (no id)
/// WHEN  it is handled
/// THEN  there is no response
#[test]
fn test_given_initialized_notification_when_handled_then_no_response() {
    let f = fake("", false);
    let server = Server::new(&f, None);
    let line = json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string();
    assert_eq!(server.handle(&line), None);
}

/// GIVEN a ping request
/// WHEN  it is handled
/// THEN  the result is an empty object
#[test]
fn test_given_ping_when_handled_then_empty_result() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
    );
    assert_eq!(r["result"], json!({}));
}

/// GIVEN a request for the method 'resources/list'
/// WHEN  it is handled
/// THEN  the error code is -32601
#[test]
fn test_given_unknown_method_when_handled_then_error_32601() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        json!({"jsonrpc":"2.0","id":3,"method":"resources/list"}),
    );
    assert_eq!(r["error"]["code"], -32601);
}

/// GIVEN a line that is not JSON, followed by a valid ping
/// WHEN  both are handled
/// THEN  the first gets -32700 with a null id and the second still works
#[test]
fn test_given_invalid_json_when_handled_then_error_32700_and_server_continues() {
    let f = fake("", false);
    let server = Server::new(&f, None);
    let bad: Value = serde_json::from_str(&server.handle("{ nope").unwrap()).unwrap();
    assert_eq!(bad["error"]["code"], -32700);
    assert_eq!(bad["id"], Value::Null);
    let ok = call(&server, json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    assert_eq!(ok["result"], json!({}));
}

/// GIVEN a JSON array (a batch)
/// WHEN  it is handled
/// THEN  the error code is -32600
#[test]
fn test_given_batch_array_when_handled_then_error_32600() {
    let f = fake("", false);
    let reply =
        Server::new(&f, None).handle("[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]");
    let r: Value = serde_json::from_str(&reply.unwrap()).unwrap();
    assert_eq!(r["error"]["code"], -32600);
}

/// GIVEN a tools/list request
/// WHEN  it is handled
/// THEN  six tools come back (brief, query, note_add, note_show, note_list, status), each with a
///       description of at most two lines and an object inputSchema
#[test]
fn test_given_tools_list_when_handled_then_four_tools_with_short_descriptions_and_schemas() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/list"}),
    );
    let tools = r["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        vec![
            "brief",
            "query",
            "note_add",
            "note_show",
            "note_list",
            "status"
        ]
    );
    for tool in tools {
        let description = tool["description"].as_str().unwrap();
        assert!(
            !description.is_empty() && description.lines().count() <= 2,
            "{description}"
        );
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
}

/// GIVEN a brief call with text and task
/// WHEN  it is handled
/// THEN  the backend runs `--task lend-1 brief <text>` and the text comes back as content
#[test]
fn test_given_brief_call_when_handled_then_argv_is_brief_with_text_and_task() {
    let f = fake("saida do brief", false);
    let r = call(
        &Server::new(&f, None),
        tool_call("brief", json!({"text":"calcular limite","task":"lend-1"})),
    );
    assert_eq!(
        argv(&f),
        strs(&["--task", "lend-1", "brief", "--", "calcular limite"])
    );
    assert_eq!(
        r["result"]["content"][0],
        json!({"type":"text","text":"saida do brief"})
    );
    assert_eq!(r["result"]["isError"], false);
}

/// GIVEN query calls for 'callers' with depth 2 and for 'symbol' without depth
/// WHEN  they are handled
/// THEN  the argv carries kind and target, and '--depth 2' only for callers
#[test]
fn test_given_query_call_when_handled_then_argv_maps_kind_target_depth_task() {
    let f = fake("x", false);
    let server = Server::new(&f, None);
    call(
        &server,
        tool_call(
            "query",
            json!({"kind":"callers","target":"A.run","depth":2,"task":"t"}),
        ),
    );
    assert_eq!(
        argv(&f),
        strs(&[
            "--task", "t", "query", "callers", "--depth", "2", "--", "A.run"
        ])
    );
    call(
        &server,
        tool_call("query", json!({"kind":"symbol","target":"A.run","depth":3})),
    );
    assert_eq!(argv(&f), strs(&["query", "symbol", "--", "A.run"]));
}

/// GIVEN a note_add call with confirm true
/// WHEN  it is handled
/// THEN  the argv uses origin 'agente' and the confirm flag, with the text last
#[test]
fn test_given_note_add_call_when_handled_then_argv_uses_agent_origin_and_confirm() {
    let f = fake("ok", false);
    call(
        &Server::new(&f, None),
        tool_call(
            "note_add",
            json!({
        "tipo":"regra","ancora":"sym:A.run","texto":"Regra.","confirm":true,"task":"t"}),
        ),
    );
    assert_eq!(
        argv(&f),
        strs(&[
            "--task",
            "t",
            "note",
            "add",
            "--tipo",
            "regra",
            "--ancora",
            "sym:A.run",
            "--origem",
            "agente",
            "--confirm",
            "--",
            "Regra."
        ])
    );
}

/// GIVEN a status call
/// WHEN  it is handled
/// THEN  the backend runs `status`
#[test]
fn test_given_status_call_when_handled_then_argv_is_status() {
    let f = fake("s", false);
    call(&Server::new(&f, None), tool_call("status", json!({})));
    assert_eq!(argv(&f), strs(&["status"]));
}

/// GIVEN a backend that reports an error with text 'nao encontrado'
/// WHEN  a tool is called
/// THEN  the result has isError true and keeps the text
#[test]
fn test_given_backend_error_when_tool_called_then_is_error_true_with_text() {
    let f = fake("nao encontrado", true);
    let r = call(&Server::new(&f, None), tool_call("status", json!({})));
    assert_eq!(r["result"]["isError"], true);
    assert_eq!(r["result"]["content"][0]["text"], "nao encontrado");
}

/// GIVEN a call to a tool named 'nope'
/// WHEN  it is handled
/// THEN  the error code is -32602 and the backend is not called
#[test]
fn test_given_unknown_tool_when_called_then_error_32602() {
    let f = fake("", false);
    let r = call(&Server::new(&f, None), tool_call("nope", json!({})));
    assert_eq!(r["error"]["code"], -32602);
    assert!(f.seen.borrow().is_empty());
}

/// GIVEN a brief call without the required 'text'
/// WHEN  it is handled
/// THEN  the error code is -32602 and the backend is not called
#[test]
fn test_given_missing_required_argument_when_called_then_error_32602() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        tool_call("brief", json!({"task":"t"})),
    );
    assert_eq!(r["error"]["code"], -32602);
    assert!(f.seen.borrow().is_empty());
}

/// GIVEN a query call with kind 'drop'
/// WHEN  it is handled
/// THEN  the error code is -32602 because the kind is not one of the query kinds
#[test]
fn test_given_bad_query_kind_when_called_then_error_32602() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        tool_call("query", json!({"kind":"drop","target":"x"})),
    );
    assert_eq!(r["error"]["code"], -32602);
}

/// GIVEN a server created with the repo '/work/repo'
/// WHEN  a tool is called
/// THEN  the argv starts with '--repo /work/repo'
#[test]
fn test_given_repo_option_when_tool_called_then_argv_starts_with_repo_flag() {
    let f = fake("x", false);
    call(
        &Server::new(&f, Some("/work/repo".into())),
        tool_call("status", json!({})),
    );
    assert_eq!(argv(&f), strs(&["--repo", "/work/repo", "status"]));
}

/// GIVEN requests with a numeric id and a string id
/// WHEN  they are handled
/// THEN  each response echoes its own id with the same type
#[test]
fn test_given_ids_when_responded_then_id_echoed_numeric_and_string() {
    let f = fake("", false);
    let server = Server::new(&f, None);
    assert_eq!(
        call(&server, json!({"jsonrpc":"2.0","id":42,"method":"ping"}))["id"],
        42
    );
    assert_eq!(
        call(&server, json!({"jsonrpc":"2.0","id":"abc","method":"ping"}))["id"],
        "abc"
    );
}

/// GIVEN two request lines on the input
/// WHEN  serve reads them until the input ends
/// THEN  two response lines are written, in order
#[test]
fn test_given_two_lines_when_serve_runs_then_two_responses_in_order() {
    let f = fake("", false);
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";
    let mut out = Vec::new();
    Server::new(&f, None)
        .serve(input.as_bytes(), &mut out)
        .unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines.iter().map(|l| l["id"].clone()).collect::<Vec<_>>(),
        vec![json!(1), json!(2)]
    );
}

/// GIVEN a brief call whose text is '--task=evil'
/// WHEN  it is handled
/// THEN  the argv has '--' right before the text, so the CLI cannot read it as a flag
#[test]
fn test_given_dash_prefixed_brief_text_when_mcp_called_then_argv_has_double_dash_separator() {
    let f = fake("x", false);
    call(
        &Server::new(&f, None),
        tool_call("brief", json!({"text":"--task=evil"})),
    );
    assert_eq!(argv(&f), strs(&["brief", "--", "--task=evil"]));
}

/// GIVEN a query call whose target is '--json'
/// WHEN  it is handled
/// THEN  the target comes after '--' and stays positional
#[test]
fn test_given_dash_prefixed_query_target_when_mcp_called_then_target_stays_positional() {
    let f = fake("x", false);
    call(
        &Server::new(&f, None),
        tool_call("query", json!({"kind":"symbol","target":"--json"})),
    );
    assert_eq!(argv(&f), strs(&["query", "symbol", "--", "--json"]));
}

/// GIVEN brief and query calls with no_tests true, and a brief call with no_tests false
/// WHEN  they are handled
/// THEN  the argv carries the global flag '--no-tests' before the subcommand only for the true ones
#[test]
fn test_given_no_tests_argument_when_mcp_brief_called_then_flag_passed_to_cli() {
    let f = fake("x", false);
    let server = Server::new(&f, None);
    call(
        &server,
        tool_call("brief", json!({"text":"a b","task":"t","no_tests":true})),
    );
    assert_eq!(
        argv(&f),
        strs(&["--task", "t", "--no-tests", "brief", "--", "a b"])
    );
    call(
        &server,
        tool_call(
            "query",
            json!({"kind":"callers","target":"A.run","no_tests":true}),
        ),
    );
    assert_eq!(
        argv(&f),
        strs(&["--no-tests", "query", "callers", "--", "A.run"])
    );
    call(
        &server,
        tool_call("brief", json!({"text":"a b","no_tests":false})),
    );
    assert_eq!(argv(&f), strs(&["brief", "--", "a b"]));
}

/// GIVEN a tools/list request
/// WHEN  it is handled
/// THEN  note_show requires an id and note_list takes only an optional task (TASK-7, D1)
#[test]
fn test_given_tools_list_when_handled_then_has_note_tools() {
    let f = fake("", false);
    let r = call(
        &Server::new(&f, None),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/list"}),
    );
    let tools = r["result"]["tools"].as_array().unwrap();
    let tool = |name: &str| tools.iter().find(|t| t["name"] == name).cloned().unwrap();
    assert_eq!(tool("note_show")["inputSchema"]["required"], json!(["id"]));
    assert!(tool("note_list")["inputSchema"].get("required").is_none());
}

/// GIVEN a note_show call with an id and a note_list call with a task
/// WHEN  they are handled
/// THEN  the backend runs `note show <id>` and `--task <t> note list` (TASK-7, D1)
#[test]
fn test_given_note_show_and_list_calls_when_handled_then_argv_maps_to_cli() {
    let f = fake("n-abc regra proposta", false);
    let server = Server::new(&f, None);
    call(&server, tool_call("note_show", json!({"id": "n-abc"})));
    assert_eq!(argv(&f), strs(&["note", "show", "--", "n-abc"]));
    call(&server, tool_call("note_list", json!({"task": "lend-1"})));
    assert_eq!(argv(&f), strs(&["--task", "lend-1", "note", "list"]));
}
