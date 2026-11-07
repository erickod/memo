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

/// GIVEN the fixture repository where 'execute' exists in four use cases
/// WHEN  'memo query symbol execute' runs
/// THEN  exit code is 2 and stdout lists the four candidate qnames
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
