mod common;

use common::Repo;
use memo_core::brief::{apply_delta, brief, tokenize};
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn open(config: &str) -> (Repo, SqliteStore) {
    let repo = Repo::fixture();
    let config = Config::from_toml(config).unwrap();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    (repo, store)
}

const OPEN: &str = "min_specificity = 0.0\nskeleton = []\n";

/// GIVEN the text 'O limite da Fatura, não é Cálculo!'
/// WHEN  it is tokenized
/// THEN  the tokens are 'limite', 'fatura', 'calculo' (lowercase, no accents, no stopwords)
#[test]
fn test_given_accented_stopworded_text_when_tokenized_then_ascii_tokens_without_stopwords() {
    assert_eq!(
        tokenize("O limite da Fatura, não é Cálculo!"),
        vec!["limite", "fatura", "calculo"]
    );
}

/// GIVEN the indexed fixture where 'limit' is a word
/// WHEN  brief runs for 'limit'
/// THEN  the first anchor is 'limit' and it is not approximate
#[test]
fn test_given_exact_word_when_brief_then_anchor_not_approx() {
    let (_r, store) = open(OPEN);
    let b = brief(&store, "limit").unwrap();
    assert_eq!(b.anchors[0].word, "limit");
    assert!(!b.anchors[0].approx);
}

/// GIVEN the token 'calculat' (8 chars), a prefix of the word 'calculate'
/// WHEN  brief runs
/// THEN  'calculate' is an anchor marked approximate
#[test]
fn test_given_long_prefix_when_brief_then_anchor_marked_approx() {
    let (_r, store) = open(OPEN);
    let b = brief(&store, "calculat").unwrap();
    let anchor = b.anchors.iter().find(|a| a.word == "calculate").unwrap();
    assert!(anchor.approx);
}

/// GIVEN the token 'calc' (4 chars), shorter than the 5-char prefix minimum
/// WHEN  brief runs
/// THEN  there are no anchors and no symbols
#[test]
fn test_given_short_prefix_when_brief_then_no_anchor() {
    let (_r, store) = open(OPEN);
    let b = brief(&store, "calc").unwrap();
    assert!(b.anchors.is_empty() && b.symbols.is_empty());
}

/// GIVEN skeleton ["usecase"] in the config
/// WHEN  brief runs for 'usecase limit'
/// THEN  'usecase' is listed as ignored and is not an anchor
#[test]
fn test_given_skeleton_word_when_brief_then_ignored_and_never_anchor() {
    let (_r, store) = open("min_specificity = 0.0\nskeleton = [\"usecase\"]\n");
    let b = brief(&store, "usecase limit").unwrap();
    assert_eq!(b.ignored, vec!["usecase"]);
    assert!(b.anchors.iter().all(|a| a.word != "usecase"));
}

/// GIVEN eight request words that exist in the fixture
/// WHEN  brief runs
/// THEN  there are exactly 5 anchors, ordered by descending score, at most 15 production symbols and 5 tests,
///       each group ordered by descending score within each tier (one rise at most, where the tiers meet)
#[test]
fn test_given_many_words_when_brief_then_five_anchors_and_fifteen_symbols_at_most() {
    let (_r, store) = open(OPEN);
    let b = brief(
        &store,
        "calculate limit usecase invoice status amount checkout anticipate",
    )
    .unwrap();
    assert_eq!(b.anchors.len(), 5);
    assert!(b.anchors.windows(2).all(|w| w[0].score >= w[1].score));
    let production = b.symbols.iter().filter(|s| !s.in_tests).count();
    let tests = b.symbols.len() - production;
    assert!(production > 0 && production <= 15 && tests <= 5);
    for group in [false, true] {
        let scores: Vec<f64> = b
            .symbols
            .iter()
            .filter(|s| s.in_tests == group)
            .map(|s| s.score)
            .collect();
        let rises = scores.windows(2).filter(|w| w[0] < w[1]).count();
        assert!(rises <= 1, "{scores:?}");
    }
}

/// GIVEN the fixture where two use cases write Invoice.status
/// WHEN  brief runs for 'invoice status'
/// THEN  the writers section lists both use cases for Invoice.status
#[test]
fn test_given_status_request_when_brief_then_writers_section_lists_both_usecases() {
    let (_r, store) = open(OPEN);
    let b = brief(&store, "invoice status").unwrap();
    let line = b
        .writers
        .iter()
        .find(|w| w.attr == "Invoice.status")
        .unwrap();
    assert_eq!(
        line.writers,
        vec![
            "AnticipateInvoiceUseCase.execute",
            "CancelInvoiceUseCase.execute"
        ]
    );
}

/// GIVEN the same indexed fixture
/// WHEN  brief runs twice for the same request
/// THEN  the JSON serializations are byte-identical
#[test]
fn test_given_same_request_twice_when_brief_then_json_bytes_identical() {
    let (_r, store) = open(OPEN);
    let run = || serde_json::to_string(&brief(&store, "calculate limit").unwrap()).unwrap();
    assert_eq!(run(), run());
}

/// GIVEN a brief delivered once to task 'lend-1'
/// WHEN  the same brief is delivered again, then with full
/// THEN  the second has no symbols and counts them as unchanged; the full one has them all
#[test]
fn test_given_task_when_brief_twice_then_second_has_only_unchanged_count() {
    let (_r, store) = open(OPEN);
    let mut first = brief(&store, "calculate limit").unwrap();
    apply_delta(&store, "lend-1", &mut first, false).unwrap();
    let total = first.symbols.len();
    assert!(total > 0 && first.unchanged == Some(0));

    let mut second = brief(&store, "calculate limit").unwrap();
    apply_delta(&store, "lend-1", &mut second, false).unwrap();
    assert!(second.symbols.is_empty());
    assert!(second.unchanged.unwrap() >= total);

    let mut full = brief(&store, "calculate limit").unwrap();
    apply_delta(&store, "lend-1", &mut full, true).unwrap();
    assert_eq!(full.symbols.len(), total);
}

/// GIVEN a brief already delivered and a new first line added to calculate_limit_usecase.py
/// WHEN  the repository is reindexed and the brief is delivered again
/// THEN  only symbols of the edited file come back and others stay counted as unchanged
#[test]
fn test_given_edited_file_when_brief_again_then_delta_has_only_changed_symbols() {
    let (repo, mut store) = open(OPEN);
    let mut first = brief(&store, "calculate limit").unwrap();
    apply_delta(&store, "lend-1", &mut first, false).unwrap();

    let path = "app/billing/application/usecases/calculate_limit_usecase.py";
    let source = std::fs::read_to_string(repo.path().join(path)).unwrap();
    repo.write(path, &format!("# shifted\n{source}"));
    let config = Config::from_toml(OPEN).unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();

    let mut again = brief(&store, "calculate limit").unwrap();
    apply_delta(&store, "lend-1", &mut again, false).unwrap();
    assert!(!again.symbols.is_empty());
    assert!(again.symbols.iter().all(|s| s.addr.path == path));
    assert!(again.unchanged.unwrap() > 0);
}
