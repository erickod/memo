use memo_core::extract::extract_python;
use memo_core::words::{NameKind, Segment, Slot, classify_name, split_name};

const MODULE: &str = "app.billing.application.usecases.calculate_limit_usecase";
const EXECUTE: &str =
    "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute";
const USECASE_SRC: &str = include_str!(
    "../../../tests/fixtures/app/billing/application/usecases/calculate_limit_usecase.py"
);

fn body_hash(source: &str) -> String {
    let facts = extract_python(MODULE, source).expect("fixture parses");
    facts
        .symbols
        .into_iter()
        .find(|s| s.qname == EXECUTE)
        .expect("execute symbol present")
        .body_hash
}

#[test]
/// GIVEN the class name 'CalculateLimitUseCase'
/// WHEN  its words are classified with the seed vocabulary
/// THEN  'calculate' is action, 'limit' is object and 'usecase' is role
fn test_given_calculate_limit_usecase_when_split_then_action_object_role() {
    let words = classify_name("CalculateLimitUseCase", NameKind::Callable);
    let got: Vec<(&str, Slot)> = words.iter().map(|w| (w.text.as_str(), w.slot)).collect();
    assert_eq!(
        got,
        vec![
            ("calculate", Slot::Action),
            ("limit", Slot::Object),
            ("usecase", Slot::Role)
        ]
    );
}

#[test]
/// GIVEN 'execute' of CalculateLimitUseCase and a copy with a different comment
/// WHEN  both are hashed
/// THEN  the body_hash values are equal
fn test_given_comment_only_change_when_hashed_then_body_hash_unchanged() {
    let changed = USECASE_SRC.replace("comment kept stable for hash tests", "another comment");
    assert_eq!(body_hash(USECASE_SRC), body_hash(&changed));
}

#[test]
/// GIVEN 'execute' of CalculateLimitUseCase and a copy with docstring 'Other text.'
/// WHEN  both are hashed
/// THEN  the body_hash values are equal
fn test_given_docstring_change_when_hashed_then_body_hash_unchanged() {
    let changed = USECASE_SRC.replace("Calculate a limit.", "Other text.");
    assert_eq!(body_hash(USECASE_SRC), body_hash(&changed));
}

#[test]
/// GIVEN the class name 'HTTPServer'
/// WHEN  the name is split
/// THEN  the words are 'http' and 'server'
fn test_given_http_server_name_when_split_then_http_and_server_words() {
    assert_eq!(split_name("HTTPServer"), vec!["http", "server"]);
}

#[test]
/// GIVEN the method name '__init__'
/// WHEN  the name is classified
/// THEN  no words are produced
fn test_given_dunder_init_when_split_then_no_words() {
    assert!(classify_name("__init__", NameKind::Callable).is_empty());
}

#[test]
/// GIVEN the test name 'test_given_amount_when_calculate_then_returns_limit'
/// WHEN  the name is classified as a test
/// THEN  'amount' is in segment given, 'calculate' in when and 'returns', 'limit' in then
fn test_given_test_prefixed_name_when_split_then_segments_given_when_then() {
    let words = classify_name(
        "test_given_amount_when_calculate_then_returns_limit",
        NameKind::Test,
    );
    let got: Vec<(&str, Option<Segment>)> =
        words.iter().map(|w| (w.text.as_str(), w.segment)).collect();
    assert_eq!(
        got,
        vec![
            ("amount", Some(Segment::Given)),
            ("calculate", Some(Segment::When)),
            ("returns", Some(Segment::Then)),
            ("limit", Some(Segment::Then)),
        ]
    );
}

#[test]
/// GIVEN 'execute(self, amount, rate: int)' where 'amount' has no annotation
/// WHEN  the file is extracted
/// THEN  RECEBE for 'amount' has dst_text None and RECEBE for 'rate' has 'int'
fn test_given_unannotated_param_when_extracted_then_recebe_with_null_dst() {
    let facts = extract_python(MODULE, USECASE_SRC).expect("fixture parses");
    let get = |p: &str| {
        facts
            .receives
            .iter()
            .find(|r| r.src_qname == EXECUTE && r.attr == p)
            .map(|r| r.dst_text.clone())
    };
    assert_eq!(get("amount"), Some(None));
    assert_eq!(get("rate"), Some(Some("int".to_string())));
}
