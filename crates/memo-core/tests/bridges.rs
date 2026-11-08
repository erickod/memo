mod common;

use common::Repo;
use memo_core::bridges::{Bridge, BridgeKind, word_bridges};
use memo_core::config::{BridgeWeights, Config};
use memo_core::error::CoreError;
use memo_core::index::index_with;
use memo_core::query::tests_for;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn build(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &Config::default()).unwrap();
    (repo, store)
}

fn weights() -> BridgeWeights {
    Config::from_toml("").unwrap().bridges
}

fn bridges(store: &SqliteStore, word: &str) -> Vec<Bridge> {
    word_bridges(store, word, &weights()).unwrap()
}

fn weight_of(list: &[Bridge], other: &str, kind: BridgeKind) -> Option<f64> {
    list.iter()
        .find(|b| b.other == other && b.kind == kind)
        .map(|b| b.weight)
}

fn close(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-9)
}

/// GIVEN an empty memo.toml and a memo.toml with only 'symbol = 2.0' under [bridges]
/// WHEN  both are parsed
/// THEN  the defaults are 1.0, 0.8, 0.6 (external 0.3), 0.5, 0.3, 0.1 and approximate 0.5;
///       the override changes only 'symbol'; a negative weight is a Config error
#[test]
fn test_given_default_bridges_when_loaded_then_weights_match_decision_d18() {
    let w = weights();
    assert_eq!(
        (
            w.symbol,
            w.test,
            w.import,
            w.import_external,
            w.call,
            w.file,
            w.context,
            w.approximate
        ),
        (1.0, 0.8, 0.6, 0.3, 0.5, 0.3, 0.1, 0.5)
    );
    let tuned = Config::from_toml("[bridges]\nsymbol = 2.0\n")
        .unwrap()
        .bridges;
    assert_eq!((tuned.symbol, tuned.test), (2.0, 0.8));
    let err = Config::from_toml("[bridges]\ncall = -1.0\n").unwrap_err();
    assert!(matches!(err, CoreError::Config(_)), "{err:?}");
}

/// GIVEN 'alpha_task' calling 'beta_job' (exact) and, in another symbol, calling an
///       unannotated receiver's unique method 'only_here' (approximate)
/// WHEN  the call bridges of 'alpha' are read
/// THEN  'beta' weighs 0.5 and 'only' weighs 0.25 (half of the call weight)
#[test]
fn test_given_approximate_bridge_when_weighted_then_half_of_type_weight() {
    let source = "def beta_job():\n    return 1\n\nclass Box:\n    def only_here(self):\n        return 2\n\ndef alpha_task(thing):\n    beta_job()\n    thing.only_here()\n";
    let (_r, store) = build(&[("m.py", source)]);
    let list = bridges(&store, "alpha");
    assert!(
        close(weight_of(&list, "beta", BridgeKind::Call), 0.5),
        "{list:?}"
    );
    assert!(
        close(weight_of(&list, "only", BridgeKind::Call), 0.25),
        "{list:?}"
    );
}

/// GIVEN 'from stripe.api import Charge' and 'from app.orders.domain.order import f' in 'pay.py'
/// WHEN  the import bridges of 'pay' are read
/// THEN  the external words 'stripe', 'api' and 'charge' weigh 0.3 and the internal 'order' weighs 0.6
#[test]
fn test_given_external_import_when_split_then_all_segments_kept_with_weight_0_3() {
    let pay = "from stripe.api import Charge\nfrom app.orders.domain.order import f\n\ndef run():\n    pass\n";
    let order = "def f():\n    return 1\n";
    let (_r, store) = build(&[
        ("app/orders/pay.py", pay),
        ("app/orders/domain/order.py", order),
    ]);
    let list = bridges(&store, "pay");
    for word in ["stripe", "api", "charge"] {
        assert!(
            close(weight_of(&list, word, BridgeKind::Import), 0.3),
            "{word}: {list:?}"
        );
    }
    assert!(
        close(weight_of(&list, "order", BridgeKind::Import), 0.6),
        "{list:?}"
    );
}

/// GIVEN 'alpha_task' calling 'beta_job'
/// WHEN  the call is removed and the repository is reindexed
/// THEN  the call bridge alpha-beta exists before and is gone after
#[test]
fn test_given_edge_change_when_reindexed_then_bridge_weights_follow() {
    let (repo, mut store) = build(&[(
        "m.py",
        "def beta_job():\n    return 1\n\ndef alpha_task():\n    beta_job()\n",
    )]);
    assert!(close(
        weight_of(&bridges(&store, "alpha"), "beta", BridgeKind::Call),
        0.5
    ));
    repo.write(
        "m.py",
        "def beta_job():\n    return 1\n\ndef alpha_task():\n    return 3\n",
    );
    index_with(&WorkTree::new(repo.path()), &mut store, &Config::default()).unwrap();
    assert_eq!(
        weight_of(&bridges(&store, "alpha"), "beta", BridgeKind::Call),
        None
    );
}

const TESTED: &str = "def calc_total():\n    return 1\n";

/// GIVEN a test 'test_given_invoice_when_total_then_ok' that calls 'calc_total' by import (exact)
/// WHEN  the test bridges of 'invoice' are read
/// THEN  'calc' and 'total' of the tested symbol weigh 0.8 (the test words propagate)
#[test]
fn test_given_test_calls_symbol_exact_when_bridged_then_test_words_propagate() {
    let test = "from m import calc_total\n\ndef test_given_invoice_when_total_then_ok():\n    calc_total()\n";
    let (_r, store) = build(&[("m.py", TESTED), ("tests/test_m.py", test)]);
    let list = bridges(&store, "invoice");
    assert!(
        close(weight_of(&list, "calc", BridgeKind::Test), 0.8),
        "{list:?}"
    );
    assert!(
        close(weight_of(&list, "total", BridgeKind::Test), 0.8),
        "{list:?}"
    );
}

/// GIVEN a test that reaches 'only_here' through an unannotated receiver (approximate edge)
/// WHEN  the bridges of the test word 'invoice' are read
/// THEN  there is no test bridge and no call bridge to 'only' or 'here'
#[test]
fn test_given_test_calls_symbol_approx_when_bridged_then_test_words_do_not_propagate() {
    let tested = "class Box:\n    def only_here(self):\n        return 2\n";
    let test = "def test_given_invoice_when_x_then_ok(thing):\n    thing.only_here()\n";
    let (_r, store) = build(&[("m.py", tested), ("tests/test_m.py", test)]);
    let list = bridges(&store, "invoice");
    for word in ["only", "here"] {
        assert!(list.iter().all(|b| b.other != word), "{word}: {list:?}");
    }
}

/// GIVEN a test whose name overlaps 'total' with 'calc_total' but which never calls it
/// WHEN  the bridges are read and 'query tests' runs for the symbol
/// THEN  the graph has no test bridge to 'calc', and 'query tests' still lists the test as approximate
#[test]
fn test_given_word_overlap_only_when_bridged_then_graph_unchanged_but_query_tests_lists_it() {
    let test = "def test_given_invoice_when_total_then_ok():\n    assert 1 == 1\n";
    let (_r, store) = build(&[("m.py", TESTED), ("tests/test_m.py", test)]);
    let list = bridges(&store, "invoice");
    assert_eq!(weight_of(&list, "calc", BridgeKind::Test), None, "{list:?}");
    let found = tests_for(&store, "calc_total").unwrap();
    let names: Vec<&str> = found.approx.iter().map(|i| i.addr.short.as_str()).collect();
    assert!(
        names.contains(&"test_given_invoice_when_total_then_ok"),
        "{names:?}"
    );
}
