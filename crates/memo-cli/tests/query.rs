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
fn test_given_ambiguous_suffix_when_query_symbol_then_candidates_and_exit_2() {
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
fn test_given_callee_when_query_callers_then_lists_callers_with_depth() {
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
fn test_given_same_brief_twice_when_run_then_stdout_bytes_identical() {
    let repo = open_repo();
    let a = memo(&repo, &["brief", "calculate limit"]);
    let b = memo(&repo, &["brief", "calculate limit"]);
    assert_eq!(a.stdout, b.stdout);
}

/// GIVEN the fixture and '--max-chars 700'
/// WHEN  'memo brief calculate limit' runs
/// THEN  stdout has at most 700 chars and its last line is '+N omitidos'
#[test]
fn test_given_small_max_chars_when_brief_then_last_line_omitidos() {
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
