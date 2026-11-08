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

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

/// GIVEN the fixture repository and the unique suffix 'CalculateLimitUseCase.execute'
/// WHEN  'memo query symbol' runs
/// THEN  exit code is 0 and the output matches the golden file with address, signature and coverage
#[test]
fn test_given_unique_suffix_when_query_symbol_then_prints_address_and_signature() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "symbol", "CalculateLimitUseCase.execute"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture repository where 'execute' exists in five use cases
/// WHEN  'memo query symbol execute' runs
/// THEN  exit code is 2 and stdout lists the five candidate qnames
#[test]
fn test_given_ambiguous_suffix_when_queried_then_candidates_and_exit_2() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "symbol", "execute"]);
    assert_eq!(out.status.code(), Some(2));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture repository
/// WHEN  'memo query symbol NoSuchSymbol' runs
/// THEN  exit code is 2
#[test]
fn test_given_unknown_symbol_when_query_symbol_then_exit_2() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "symbol", "NoSuchSymbol"]);
    assert_eq!(out.status.code(), Some(2));
}

/// GIVEN the fixture repository where 'limit' is an object word of CalculateLimitUseCase
/// WHEN  'memo query word limit' runs
/// THEN  the output matches the golden file with verbs, roles, layers, symbols and neighbors
#[test]
fn test_given_word_when_query_word_then_verbs_roles_layers_symbols() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "word", "limit"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture repository
/// WHEN  'memo --json query symbol CalculateLimitUseCase.execute' runs
/// THEN  stdout is JSON with schema 1, cmd 'query symbol' and a data.qname field
#[test]
fn test_given_json_flag_when_query_symbol_then_schema_1_envelope() {
    let repo = Repo::fixture();
    let out = memo(
        &repo,
        &["--json", "query", "symbol", "CalculateLimitUseCase.execute"],
    );
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(json["schema"], 1);
    assert_eq!(json["cmd"], "query symbol");
    assert_eq!(
        json["data"]["qname"],
        "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute"
    );
}

/// GIVEN the fixture repository and '--max-chars 600'
/// WHEN  'memo query word limit' runs
/// THEN  stdout has at most 600 chars and its last line starts with '+' and ends with 'omitidos'
#[test]
fn test_given_small_max_chars_when_query_word_then_last_line_omitidos() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["--max-chars", "600", "query", "word", "limit"]);
    let text = stdout(&out);
    assert!(text.chars().count() <= 600, "{text}");
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with('+') && last.ends_with("omitidos"),
        "{last}"
    );
}

/// GIVEN the fixture where two use cases assign 'self._invoice.status'
/// WHEN  'memo query writers Invoice.status' runs
/// THEN  the golden output lists AnticipateInvoiceUseCase.execute and CancelInvoiceUseCase.execute
#[test]
fn test_given_two_usecases_writing_status_when_writers_then_both_listed() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "writers", "Invoice.status"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture where CalculateLimitUseCase.execute calls 'self._repo.save(limit)'
/// WHEN  'memo query readers CalculateLimitUseCase._repo' runs
/// THEN  the golden output lists CalculateLimitUseCase.execute
#[test]
fn test_given_attribute_read_in_method_when_readers_then_method_listed() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "readers", "CalculateLimitUseCase._repo"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture where two use cases receive 'invoice: Invoice' in __init__
/// WHEN  'memo query type Invoice' runs
/// THEN  the golden output lists both constructors as consumers
#[test]
fn test_given_typed_param_when_query_type_then_consumers_listed() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "type", "Invoice"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture where execute calls 'save' through Protocol LimitRepository
/// WHEN  'memo query callers SqlLimitRepository.save --depth 2' runs
/// THEN  the golden output lists execute (approximate, depth 1) and its callers at depth 2 with coverage
#[test]
fn test_given_protocol_consumer_when_callers_then_included_with_cobertura() {
    let repo = Repo::fixture();
    let out = memo(
        &repo,
        &[
            "query",
            "callers",
            "SqlLimitRepository.save",
            "--depth",
            "2",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture where CalculateLimitUseCase.execute calls Protocol and external names
/// WHEN  'memo query callees CalculateLimitUseCase.execute' runs
/// THEN  the golden output lists the Protocol method, the implementation (~) and external calls
#[test]
fn test_given_caller_when_query_callees_then_lists_internal_and_external() {
    let repo = Repo::fixture();
    let out = memo(
        &repo,
        &["query", "callees", "CalculateLimitUseCase.execute"],
    );
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture and the target SqlLimitRepository.save
/// WHEN  'memo query impact SqlLimitRepository.save' runs
/// THEN  the golden output has callers, protocol counterpart, tests and notes sections
#[test]
fn test_given_method_when_query_impact_then_sections_callers_consumers_protocol_tests() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "impact", "SqlLimitRepository.save"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture where a test calls execute through a constructed local
/// WHEN  'memo query tests CalculateLimitUseCase.execute' runs
/// THEN  the golden output lists the test under the approximate group
#[test]
fn test_given_method_when_query_tests_then_exact_and_word_overlap_approx() {
    let repo = Repo::fixture();
    let out = memo(&repo, &["query", "tests", "CalculateLimitUseCase.execute"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture
/// WHEN  'memo query callers SqlLimitRepository.save --depth 4' runs
/// THEN  exit code is 1 because depth is limited to 3
#[test]
fn test_given_depth_over_three_when_query_callers_then_usage_error_exit_1() {
    let repo = Repo::fixture();
    let out = memo(
        &repo,
        &[
            "query",
            "callers",
            "SqlLimitRepository.save",
            "--depth",
            "4",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
}

/// GIVEN a memo.toml with skeleton ["usecase"] and the fixture
/// WHEN  'memo query word limit' runs
/// THEN  the golden output lists 'usecase' under ignoradas, not under papéis
#[test]
fn test_given_skeleton_in_toml_when_query_word_then_ignored_listed_and_not_in_roles() {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", "skeleton = [\"usecase\"]\n");
    let out = memo(&repo, &["query", "word", "limit"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN a memo.toml with skeleton ["usecase"] and the fixture
/// WHEN  'memo query word usecase' runs
/// THEN  the output has the line 'esqueleto: sim'
#[test]
fn test_given_skeleton_word_when_query_word_then_esqueleto_line_marked() {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", "skeleton = [\"usecase\"]\n");
    let out = memo(&repo, &["query", "word", "usecase"]);
    assert!(stdout(&out).contains("esqueleto: sim"), "{}", stdout(&out));
}

/// GIVEN a memo.toml with 'roles = 3'
/// WHEN  'memo query word limit' runs
/// THEN  exit code is 1 (usage error)
#[test]
fn test_given_invalid_memo_toml_when_query_then_exit_1() {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", "roles = 3\n");
    let out = memo(&repo, &["query", "word", "limit"]);
    assert_eq!(out.status.code(), Some(1));
}

const OPEN_TOML: &str = "min_specificity = 0.0\nskeleton = []\n";

fn open_repo() -> Repo {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", OPEN_TOML);
    repo
}

/// GIVEN the fixture with min_specificity 0 and the request 'calculate limit'
/// WHEN  'memo brief' runs
/// THEN  the golden output has anchors, symbols, outline, pattern, writers, notes and coverage
#[test]
fn test_given_request_when_brief_then_golden_sections() {
    let out = memo(&open_repo(), &["brief", "calculate limit"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the same fixture and request
/// WHEN  'memo brief' runs twice
/// THEN  both stdout outputs are byte-identical
#[test]
fn test_given_same_brief_twice_when_run_then_bytes_identical() {
    let repo = open_repo();
    let a = memo(&repo, &["brief", "calculate limit"]);
    let b = memo(&repo, &["brief", "calculate limit"]);
    assert_eq!(a.stdout, b.stdout);
}

/// GIVEN the fixture and '--max-chars 700'
/// WHEN  'memo brief calculate limit' runs
/// THEN  stdout has at most 700 chars and its last line is '+N omitidos'
#[test]
fn test_given_output_over_max_chars_when_rendered_then_last_line_omitidos() {
    let out = memo(
        &open_repo(),
        &["--max-chars", "700", "brief", "calculate limit"],
    );
    let text = stdout(&out);
    assert!(text.chars().count() <= 700, "{text}");
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with('+') && last.ends_with("omitidos"),
        "{last}"
    );
}

/// GIVEN '--task lend-1' and a first brief already delivered
/// WHEN  the same brief runs again, then with '--full'
/// THEN  the second prints 'inalterados:' and no symbol lines; the third prints the symbols again
#[test]
fn test_given_task_flag_when_brief_twice_then_second_prints_inalterados() {
    let repo = open_repo();
    let args = ["--task", "lend-1", "brief", "calculate limit"];
    let first = stdout(&memo(&repo, &args));
    assert!(first.contains("símbolos:\n  "), "{first}");
    let second = stdout(&memo(&repo, &args));
    assert!(second.contains("inalterados: "), "{second}");
    assert!(!second.contains("calculate_limit_usecase.py"), "{second}");
    let full = stdout(&memo(
        &repo,
        &["--task", "lend-1", "--full", "brief", "calculate limit"],
    ));
    assert!(full.contains("calculate_limit_usecase.py"), "{full}");
}

/// GIVEN '--since-last' without '--task' and a first brief already delivered
/// WHEN  the same brief runs again
/// THEN  the second prints 'inalterados:' instead of the symbols
#[test]
fn test_given_since_last_without_task_when_brief_twice_then_delta_applies() {
    let repo = open_repo();
    let args = ["--since-last", "brief", "calculate limit"];
    memo(&repo, &args);
    let second = stdout(&memo(&repo, &args));
    assert!(second.contains("inalterados: "), "{second}");
}

/// GIVEN '--task lend-1' and a first 'query impact' already delivered
/// WHEN  the same impact query runs again
/// THEN  the second prints 'inalterados:' and no caller lines
#[test]
fn test_given_task_flag_when_impact_twice_then_second_prints_inalterados() {
    let repo = open_repo();
    let args = [
        "--task",
        "lend-1",
        "query",
        "impact",
        "SqlLimitRepository.save",
    ];
    let first = stdout(&memo(&repo, &args));
    assert!(first.contains("CalculateLimitUseCase.execute"), "{first}");
    let second = stdout(&memo(&repo, &args));
    assert!(second.contains("inalterados: "), "{second}");
    assert!(
        !second.contains("CalculateLimitUseCase.execute"),
        "{second}"
    );
}

const ADD: [&str; 8] = [
    "note",
    "add",
    "--tipo",
    "regra",
    "--ancora",
    "sym:CalculateLimitUseCase.execute",
    "--origem",
    "humano",
];

/// `globals` vão antes do subcomando (`--task`); `flags` depois de `note add` (`--confirm`).
fn add_note(repo: &Repo, globals: &[&str], flags: &[&str], text: &str) -> Output {
    let mut args: Vec<&str> = globals.to_vec();
    args.extend(ADD);
    args.extend(flags);
    args.push(text);
    memo(repo, &args)
}

/// GIVEN the fixture and a fixed clock
/// WHEN  'memo note add' runs for CalculateLimitUseCase.execute
/// THEN  exit code is 0 and the golden output shows the new proposta note
#[test]
fn test_given_note_add_when_run_then_golden_proposta_and_exit_0() {
    let out = add_note(&Repo::fixture(), &[], &[], "Limite é inteiro.");
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN a missing symbol anchor and, separately, a skeleton word anchor
/// WHEN  'memo note add' runs for each
/// THEN  the exit codes are 2 and 4 (A12)
#[test]
fn test_given_bad_anchors_when_note_add_then_exit_2_and_exit_4() {
    let repo = open_repo();
    repo.write(".git/memo/memo.toml", "skeleton = [\"usecase\"]\n");
    let missing = memo(
        &repo,
        &[
            "note",
            "add",
            "--tipo",
            "regra",
            "--ancora",
            "sym:NoSuchSymbol",
            "--origem",
            "humano",
            "x",
        ],
    );
    assert_eq!(missing.status.code(), Some(2));
    let generic = memo(
        &repo,
        &[
            "note",
            "add",
            "--tipo",
            "regra",
            "--ancora",
            "word:usecase",
            "--origem",
            "humano",
            "x",
        ],
    );
    assert_eq!(generic.status.code(), Some(4));
    assert!(
        stdout(&generic).contains("ancora_generica"),
        "{}",
        stdout(&generic)
    );
}

/// GIVEN a note added on CalculateLimitUseCase.execute
/// WHEN  'memo query symbol CalculateLimitUseCase.execute' runs
/// THEN  the golden output has the note line instead of 'notas: nenhuma'
#[test]
fn test_given_note_on_symbol_when_query_symbol_then_notas_listed() {
    let repo = Repo::fixture();
    add_note(&repo, &[], &["--confirm"], "Limite é inteiro.");
    let out = memo(&repo, &["query", "symbol", "CalculateLimitUseCase.execute"]);
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN a confirmed note on CalculateLimitUseCase.execute
/// WHEN  'memo brief calculate limit' runs
/// THEN  the notes line holds the note id, 'regra confirmada' and its text
#[test]
fn test_given_note_when_brief_then_notes_line_present() {
    let repo = open_repo();
    add_note(&repo, &[], &["--confirm"], "Limite é inteiro.");
    let out = stdout(&memo(&repo, &["brief", "calculate limit"]));
    assert!(
        out.contains("regra confirmada") && out.contains("Limite é inteiro."),
        "{out}"
    );
}

/// GIVEN a proposta note
/// WHEN  'note list', 'note confirm <id>' and 'note show <id>' run in sequence
/// THEN  list shows 'proposta', confirm exits 0, and show prints the state 'confirmada' and the text
#[test]
fn test_given_note_when_list_confirm_show_then_state_flow_visible() {
    let repo = Repo::fixture();
    let added = stdout(&add_note(&repo, &[], &[], "Limite é inteiro."));
    let id = added.split_whitespace().nth(1).unwrap().to_string();
    assert!(stdout(&memo(&repo, &["note", "list"])).contains("proposta"));
    assert_eq!(
        memo(&repo, &["note", "confirm", &id]).status.code(),
        Some(0)
    );
    let shown = stdout(&memo(&repo, &["note", "show", &id]));
    assert!(
        shown.contains("confirmada") && shown.contains("Limite é inteiro."),
        "{shown}"
    );
    assert_eq!(
        memo(&repo, &["note", "show", "n-nonexist"]).status.code(),
        Some(2)
    );
}

/// GIVEN a note added with '--task lend-1'
/// WHEN  'memo query symbol' runs without a task and with '--task lend-1'
/// THEN  the note appears only in the second output
#[test]
fn test_given_task_scoped_note_when_queried_without_task_then_hidden() {
    let repo = Repo::fixture();
    add_note(&repo, &["--task", "lend-1"], &[], "Só desta task.");
    let q = ["query", "symbol", "CalculateLimitUseCase.execute"];
    assert!(!stdout(&memo(&repo, &q)).contains("Só desta task."));
    let mut with_task = vec!["--task", "lend-1"];
    with_task.extend(q);
    assert!(stdout(&memo(&repo, &with_task)).contains("Só desta task."));
}

const PY: &str = "app/billing/application/usecases/calculate_limit_usecase.py";

fn edit_py(repo: &Repo, from: &str, to: &str) {
    let source = std::fs::read_to_string(repo.path().join(PY)).unwrap();
    assert!(source.contains(from), "fixture changed: {from}");
    repo.write(PY, &source.replace(from, to));
    repo.commit_all();
}

/// GIVEN the fixture and then execute renamed to 'compute' with the same body
/// WHEN  'memo index' runs after the first one
/// THEN  the golden output ends with the one-line events summary
#[test]
fn test_given_index_command_when_run_then_golden_summary_line() {
    let repo = Repo::fixture();
    assert_eq!(memo(&repo, &["index"]).status.code(), Some(0));
    edit_py(&repo, "def execute(", "def compute(");
    let out = memo(&repo, &["index"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN a confirmed note on execute and a signature change
/// WHEN  'memo query symbol' runs
/// THEN  the note appears as 'stale' (the marker is the state)
#[test]
fn test_given_signature_change_when_symbol_queried_then_note_shown_as_stale() {
    let repo = Repo::fixture();
    add_note(&repo, &[], &["--confirm"], "Limite é inteiro.");
    edit_py(&repo, "rate: int)", "rate: int, extra=0)");
    let out = stdout(&memo(
        &repo,
        &["query", "symbol", "CalculateLimitUseCase.execute"],
    ));
    assert!(out.contains("regra stale"), "{out}");
}

/// GIVEN a confirmed note and a signature change
/// WHEN  a query runs with '--no-index'
/// THEN  the note is still 'confirmada' (P4: only reindexing changes notes)
#[test]
fn test_given_no_index_flag_when_note_anchor_changed_then_state_unchanged() {
    let repo = Repo::fixture();
    add_note(&repo, &[], &["--confirm"], "Limite é inteiro.");
    memo(&repo, &["index"]);
    edit_py(&repo, "rate: int)", "rate: int, extra=0)");
    let out = stdout(&memo(
        &repo,
        &[
            "--no-index",
            "query",
            "symbol",
            "CalculateLimitUseCase.execute",
        ],
    ));
    assert!(out.contains("regra confirmada"), "{out}");
}

/// GIVEN a stale note
/// WHEN  'memo note recheck <id>' runs and the symbol is queried
/// THEN  recheck exits 0 and the note is 'confirmada' again
#[test]
fn test_given_stale_note_when_recheck_then_estado_anterior_restored() {
    let repo = Repo::fixture();
    let added = stdout(&add_note(&repo, &[], &["--confirm"], "Limite é inteiro."));
    let id = added.split_whitespace().nth(1).unwrap().to_string();
    edit_py(&repo, "rate: int)", "rate: int, extra=0)");
    memo(&repo, &["index"]);
    assert_eq!(
        memo(&repo, &["note", "recheck", &id]).status.code(),
        Some(0)
    );
    let out = stdout(&memo(
        &repo,
        &["query", "symbol", "CalculateLimitUseCase.execute"],
    ));
    assert!(out.contains("regra confirmada"), "{out}");
}

/// GIVEN a note on execute and execute renamed with the same body
/// WHEN  the repository is reindexed and the note is shown
/// THEN  the note is anchored on 'compute' and still 'confirmada' (A9)
#[test]
fn test_given_renamed_symbol_when_note_shown_then_reanchored_and_confirmada() {
    let repo = Repo::fixture();
    let added = stdout(&add_note(&repo, &[], &["--confirm"], "Limite é inteiro."));
    let id = added.split_whitespace().nth(1).unwrap().to_string();
    edit_py(&repo, "def execute(", "def compute(");
    memo(&repo, &["index"]);
    let shown = stdout(&memo(&repo, &["note", "show", &id]));
    assert!(shown.contains("CalculateLimitUseCase.compute"), "{shown}");
    assert!(shown.contains("estado: confirmada"), "{shown}");
}

/// GIVEN a committed fixture whose base was indexed from main, and a function added only in the work tree
/// WHEN  'memo query symbol only_here' runs without any derived feature
/// THEN  it exits 2: the base is the main ref graph, not the work tree (D12)
#[test]
fn test_given_ref_base_when_worktree_changed_without_feature_then_query_reads_ref_graph() {
    let repo = Repo::fixture();
    assert_eq!(memo(&repo, &["index"]).status.code(), Some(0));
    let source = std::fs::read_to_string(repo.path().join(PY)).unwrap();
    repo.write(PY, &format!("{source}\n\ndef only_here():\n    pass\n"));
    let o = memo(&repo, &["query", "symbol", "only_here"]);
    assert_eq!(o.status.code(), Some(2), "{}", stdout(&o));
}

/// GIVEN a feature derived for 't1' and a function added in the work tree afterwards
/// WHEN  'memo query symbol only_here' runs with '--task t1' and without it
/// THEN  only the feature finds it (exit 0 versus exit 2)
#[test]
fn test_given_derived_feature_when_worktree_changed_then_only_feature_sees_change() {
    let repo = Repo::fixture();
    assert_eq!(memo(&repo, &["derive", "t1"]).status.code(), Some(0));
    let source = std::fs::read_to_string(repo.path().join(PY)).unwrap();
    repo.write(PY, &format!("{source}\n\ndef only_here():\n    pass\n"));
    let with = memo(&repo, &["--task", "t1", "query", "symbol", "only_here"]);
    assert_eq!(with.status.code(), Some(0), "{}", stdout(&with));
    let without = memo(&repo, &["query", "symbol", "only_here"]);
    assert_eq!(without.status.code(), Some(2));
}

/// GIVEN a confirmed repo note on execute and an uncommitted signature change, with no feature
/// WHEN  a query runs and the note is shown
/// THEN  the note is still 'confirmada' because the work tree never reaches the base
#[test]
fn test_given_repo_notes_when_worktree_changed_without_feature_then_notes_not_stale() {
    let repo = Repo::fixture();
    let added = stdout(&add_note(&repo, &[], &["--confirm"], "Limite é inteiro."));
    let id = added.split_whitespace().nth(1).unwrap().to_string();
    let source = std::fs::read_to_string(repo.path().join(PY)).unwrap();
    repo.write(PY, &source.replace("rate: int)", "rate: int, extra=0)"));
    memo(&repo, &["query", "symbol", "CalculateLimitUseCase.execute"]);
    let shown = stdout(&memo(&repo, &["note", "show", &id]));
    assert!(shown.contains("estado: confirmada"), "{shown}");
}

/// GIVEN a file 'n-bad00000.md' with garbage in the notes directory
/// WHEN  'memo --json query symbol' runs
/// THEN  it exits 0 and cobertura.notas_com_erro is 1
#[test]
fn test_given_corrupted_note_file_when_queried_then_skipped_and_counted_in_cobertura() {
    let repo = Repo::fixture();
    repo.write(".git/memo/notes/n-bad00000.md", "garbage");
    let o = memo(
        &repo,
        &["--json", "query", "symbol", "CalculateLimitUseCase.execute"],
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json["cobertura"]["notas_com_erro"], 1);
}

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

fn write_modules(repo: &Repo, dir: &str, count: usize, defs: impl Fn(usize) -> Vec<String>) {
    for i in 0..count {
        let body: String = defs(i)
            .iter()
            .map(|d| format!("def {d}():\n    return 1\n\n"))
            .collect();
        repo.write(&format!("app/{dir}/m{}.py", token(i)), &body);
    }
    repo.commit_all();
}

/// GIVEN 'order' used in 40 modules next to the same four words, skeleton list empty
/// WHEN  'memo query word order' runs
/// THEN  the output has no 'esqueleto: sim' line (frequent but coherent)
#[test]
fn test_given_order_like_fixture_when_indexed_then_frequent_coherent_word_not_esqueleto() {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", "skeleton = []\n");
    write_modules(&repo, "o", 40, |_| {
        ["placed", "canceled", "confirmed", "failed"]
            .iter()
            .map(|s| format!("order_{s}"))
            .collect()
    });
    let out = stdout(&memo(&repo, &["query", "word", "order"]));
    assert!(
        out.contains("stats: uso=") && !out.contains("esqueleto: sim"),
        "{out}"
    );
}

/// GIVEN 'get' used in 50 modules, each next to a different word, skeleton list empty
/// WHEN  'memo query word get' runs
/// THEN  the output has the line 'esqueleto: sim' (frequent and incoherent)
#[test]
fn test_given_get_like_fixture_when_indexed_then_spread_generic_word_is_esqueleto() {
    let repo = Repo::fixture();
    repo.write(".git/memo/memo.toml", "skeleton = []\n");
    write_modules(&repo, "g", 50, |i| vec![format!("get_{}", token(i + 200))]);
    let out = stdout(&memo(&repo, &["query", "word", "get"]));
    assert!(out.contains("esqueleto: sim"), "{out}");
}

/// GIVEN the fixture with min_specificity 0 and the word 'limit', used in names, a test, imports and calls
/// WHEN  'memo query word limit' runs
/// THEN  the golden card lists neighbors per bridge type with weights, the coherence and tagged symbols
#[test]
fn test_given_word_when_query_word_then_card_lists_neighbors_by_bridge_type() {
    let out = memo(&open_repo(), &["query", "word", "limit"]);
    assert_eq!(out.status.code(), Some(0));
    insta::assert_snapshot!(stdout(&out));
}

/// GIVEN the fixture and the request 'calculate limit'
/// WHEN  'memo brief' runs
/// THEN  every production symbol line ends with its bridge tags in brackets, starting with 'nome'
#[test]
fn test_given_brief_when_run_then_symbols_show_bridge_tags() {
    let out = stdout(&memo(&open_repo(), &["brief", "calculate limit"]));
    let lines: Vec<&str> = out
        .lines()
        .skip_while(|l| *l != "símbolos:")
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .collect();
    assert!(!lines.is_empty(), "{out}");
    for line in &lines {
        assert!(line.ends_with(']') && line.contains(" ["), "{line}");
    }
    assert!(lines.iter().any(|l| l.contains("[nome")), "{out}");
}

/// GIVEN the fixture where a test calls CalculateLimitUseCase
/// WHEN  'memo brief calculate limit' runs
/// THEN  no 'T' line appears before the 'testes:' section, and that section is after 'escritores:'
#[test]
fn test_given_tests_in_results_when_brief_then_tests_listed_in_final_section() {
    let out = stdout(&memo(&open_repo(), &["brief", "calculate limit"]));
    let tests_at = out.lines().position(|l| l == "testes:").expect(&out);
    let writers_at = out.lines().position(|l| l == "escritores:").expect(&out);
    assert!(tests_at > writers_at, "{out}");
    for (i, line) in out.lines().enumerate() {
        let is_test_item = line.starts_with("  T ");
        assert_eq!(
            is_test_item,
            i > tests_at && line.starts_with("  T "),
            "{line}"
        );
        if i < tests_at {
            assert!(!is_test_item, "{line}");
        }
    }
    assert!(
        out.lines().skip(tests_at).any(|l| l.starts_with("  T ")),
        "{out}"
    );
}

/// GIVEN '--max-chars 1400' where the full brief does not fit
/// WHEN  'memo brief calculate limit' runs
/// THEN  the tests are cut first (a 'testes omitidos' marker, production symbols all kept)
///       and the last line is '+N omitidos' with N counting the cut tests
#[test]
fn test_given_small_max_chars_when_brief_then_tests_cut_first_and_omitidos_per_section() {
    let full = stdout(&memo(&open_repo(), &["brief", "calculate limit"]));
    let production: Vec<&str> = full
        .lines()
        .skip_while(|l| *l != "símbolos:")
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .collect();
    let limit = full.chars().count() - 40;
    let out = stdout(&memo(
        &open_repo(),
        &[
            "--max-chars",
            &limit.to_string(),
            "brief",
            "calculate limit",
        ],
    ));
    assert!(out.chars().count() <= limit, "{out}");
    for line in &production {
        assert!(out.contains(line), "production line lost: {line}\n{out}");
    }
    assert!(out.contains("testes omitidos"), "{out}");
    let last = out.lines().last().unwrap();
    assert!(
        last.starts_with('+') && last.ends_with("omitidos"),
        "{last}"
    );
}
