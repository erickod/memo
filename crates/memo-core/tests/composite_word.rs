mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::error::CoreError;
use memo_core::index::index;
use memo_core::query::{composite_word, word_parts};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn store_of(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

const SOURCE: &str = "def order_payments_total():\n    pass\n\ndef order_items():\n    pass\n\n\
                      def list_payments():\n    pass\n";

/// GIVEN functions order_payments_total, order_items and list_payments
/// WHEN  'order_payments' and 'order.payments' are split and crossed
/// THEN  the parts are order and payments, both cards exist and only order_payments_total has both (TASK-7, D7)
#[test]
fn test_given_composite_word_when_crossed_then_parts_cards_and_shared_symbols() {
    let (_repo, store) = store_of(&[("m.py", SOURCE)]);
    let config = Config::default();
    for input in ["order_payments", "order.payments"] {
        let parts = word_parts(input, &config);
        assert_eq!(parts, ["order", "payments"], "{input}");
        let c = composite_word(&store, input, &parts).unwrap();
        let card_words: Vec<&str> = c.cards.iter().map(|w| w.word.as_str()).collect();
        assert_eq!(card_words, ["order", "payments"]);
        let shared: Vec<&str> = c.symbols.iter().map(|s| s.qname.as_str()).collect();
        assert_eq!(shared, ["m.order_payments_total"]);
    }
}

/// GIVEN the inputs '__init__' and '_order_'
/// WHEN  they are split
/// THEN  the leading and trailing underscores go away: 'init' and 'order' (TASK-7, D7)
#[test]
fn test_given_underscore_prefix_suffix_when_split_then_stripped() {
    let config = Config::default();
    assert_eq!(word_parts("__init__", &config), ["init"]);
    assert_eq!(word_parts("_order_", &config), ["order"]);
}

/// GIVEN a composite input whose parts appear nowhere
/// WHEN  it is crossed
/// THEN  NotFound (TASK-7, D7)
#[test]
fn test_given_unknown_composite_when_crossed_then_not_found() {
    let (_repo, store) = store_of(&[("m.py", SOURCE)]);
    let parts = word_parts("zebra_quokka", &Config::default());
    let got = composite_word(&store, "zebra_quokka", &parts);
    assert!(matches!(got, Err(CoreError::NotFound(_))), "{got:?}");
}
