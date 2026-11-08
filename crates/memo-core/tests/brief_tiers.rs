mod common;

use common::Repo;
use memo_core::brief::{Brief, brief};
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

/// Uma função por item, chamando as funções listadas (chamadas exatas no mesmo módulo).
fn module(defs: &[(String, Vec<String>)]) -> String {
    defs.iter()
        .map(|(name, calls)| {
            let body: String = if calls.is_empty() {
                "    return 1\n".to_string()
            } else {
                calls.iter().map(|c| format!("    {c}()\n")).collect()
            };
            format!("def {name}():\n{body}\n")
        })
        .collect()
}

fn def(name: &str, calls: &[&str]) -> (String, Vec<String>) {
    (
        name.to_string(),
        calls.iter().map(|c| (*c).to_string()).collect(),
    )
}

/// Fixture A: `emit` é chamado por 4 símbolos da âncora e por 6 de ruído; `audit` por 2 da âncora.
fn fixture_a() -> Repo {
    let repo = Repo::empty();
    let mut defs = vec![
        def("emit", &[]),
        def("audit", &[]),
        def("order_a", &["emit", "audit"]),
        def("order_b", &["emit", "audit"]),
        def("order_c", &["emit"]),
        def("order_d", &["emit"]),
    ];
    for i in 0..6 {
        defs.push((format!("noise_{}", token(i)), vec!["emit".to_string()]));
    }
    repo.write("app/o/core.py", &module(&defs));
    repo
}

/// Fixture B: 12 símbolos da âncora, 6 folhas próprias e `emit` com grau 20.
fn fixture_b() -> Repo {
    let repo = Repo::empty();
    let mut defs = vec![def("emit", &[])];
    for i in 0..12 {
        let mut calls = vec!["emit".to_string()];
        if i < 6 {
            calls.push(format!("leaf_{}", token(i)));
        }
        defs.push((format!("order_{}", token(i)), calls));
    }
    for i in 0..6 {
        defs.push((format!("leaf_{}", token(i)), Vec::new()));
    }
    for i in 0..8 {
        defs.push((
            format!("noise_{}", token(100 + i)),
            vec!["emit".to_string()],
        ));
    }
    repo.write("app/o/core.py", &module(&defs));
    repo
}

fn brief_of(repo: &Repo) -> Brief {
    let config = Config::default();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    brief(&store, "order").unwrap()
}

fn production(b: &Brief) -> Vec<String> {
    b.symbols
        .iter()
        .filter(|s| !s.in_tests)
        .map(|s| s.addr.short.clone())
        .collect()
}

fn position(list: &[String], name: &str) -> usize {
    list.iter()
        .position(|s| s == name)
        .unwrap_or_else(|| panic!("{name} not listed in {list:?}"))
}

/// GIVEN fixture A (below) and the request "order"
/// WHEN  brief is computed
/// THEN  the production symbols start with order_a, order_b, order_c, order_d, before emit and audit
#[test]
fn test_given_hub_called_by_anchor_symbols_when_brief_then_anchor_symbols_listed_first() {
    let list = production(&brief_of(&fixture_a()));
    assert_eq!(&list[..4], ["order_a", "order_b", "order_c", "order_d"]);
}

/// GIVEN fixture A and the request "order"
/// WHEN  brief is computed
/// THEN  emit and audit are still listed, after the four order_* symbols
#[test]
fn test_given_neighbor_below_anchor_symbols_when_brief_then_neighbors_still_listed() {
    let list = production(&brief_of(&fixture_a()));
    assert!(position(&list, "emit") >= 4, "{list:?}");
    assert!(position(&list, "audit") >= 4, "{list:?}");
}

/// GIVEN fixture A, where emit has 10 incident call edges and 4 anchor callers and audit has 2 incident edges and 2 anchor callers
/// WHEN  brief is computed
/// THEN  audit is listed before emit (0.5 against 0.2 times the anchor score; before the change emit came first)
#[test]
fn test_given_hub_with_many_links_when_brief_then_it_ranks_below_specific_neighbor() {
    let list = production(&brief_of(&fixture_a()));
    assert!(
        position(&list, "audit") < position(&list, "emit"),
        "{list:?}"
    );
}

/// GIVEN fixture B and the request "order"
/// WHEN  brief is computed
/// THEN  15 production symbols: the first 10 are order_waa to order_wak, then leaf_waa to leaf_wae
#[test]
fn test_given_twelve_anchor_symbols_and_neighbors_when_brief_then_ten_then_five_slots() {
    let list = production(&brief_of(&fixture_b()));
    let expected: Vec<String> = (0..10)
        .map(|i| format!("order_{}", token(i)))
        .chain((0..5).map(|i| format!("leaf_{}", token(i))))
        .collect();
    assert_eq!(list, expected);
}

/// GIVEN fixture C (14 anchor symbols, 2 neighbors) and the request "order"
/// WHEN  brief is computed
/// THEN  15 production symbols: 13 order_* symbols followed by the 2 neighbors
#[test]
fn test_given_two_neighbors_only_when_brief_then_spare_slots_go_to_anchor_symbols() {
    let repo = Repo::empty();
    let mut defs = vec![def("emit", &[]), def("audit", &[])];
    for i in 0..14 {
        defs.push((
            format!("order_{}", token(i)),
            vec!["emit".to_string(), "audit".to_string()],
        ));
    }
    repo.write("app/o/core.py", &module(&defs));
    let list = production(&brief_of(&repo));
    let expected: Vec<String> = (0..13)
        .map(|i| format!("order_{}", token(i)))
        .chain(["audit".to_string(), "emit".to_string()])
        .collect();
    assert_eq!(list, expected);
}

/// GIVEN fixture D (3 anchor symbols, each calling 3 distinct leaves) and the request "order"
/// WHEN  brief is computed
/// THEN  12 production symbols: the 3 order_* symbols followed by the 9 leaves
#[test]
fn test_given_three_anchor_symbols_when_brief_then_spare_slots_go_to_neighbors() {
    let repo = Repo::empty();
    let mut defs = Vec::new();
    for owner in ["a", "b", "c"] {
        let leaves: Vec<String> = ["aa", "ab", "ac"]
            .iter()
            .map(|l| format!("leaf_{owner}{l}"))
            .collect();
        defs.push((format!("order_{owner}"), leaves.clone()));
        defs.extend(leaves.into_iter().map(|l| (l, Vec::new())));
    }
    repo.write("app/o/core.py", &module(&defs));
    let list = production(&brief_of(&repo));
    assert_eq!(list.len(), 12, "{list:?}");
    assert_eq!(&list[..3], ["order_a", "order_b", "order_c"]);
    assert!(list[3..].iter().all(|s| s.starts_with("leaf_")), "{list:?}");
}

/// GIVEN fixture E (order_a calls order_b and emit, order_b calls emit) and the request "order"
/// WHEN  brief is computed
/// THEN  order_a and order_b are listed before emit, and emit is still listed
#[test]
fn test_given_anchor_symbol_that_is_also_neighbor_when_brief_then_it_stays_in_first_tier() {
    let repo = Repo::empty();
    let defs = [
        def("emit", &[]),
        def("order_a", &["order_b", "emit"]),
        def("order_b", &["emit"]),
    ];
    repo.write("app/o/core.py", &module(&defs));
    let list = production(&brief_of(&repo));
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[2], "emit");
}

/// GIVEN fixture F (4 tests named test_order_*, each calling helper, and 8 other tests calling helper) and the request "order"
/// WHEN  brief is computed
/// THEN  in the tests section the 4 test_order_* come before helper
#[test]
fn test_given_test_hub_when_brief_then_direct_tests_precede_neighbor_in_tests_section() {
    let repo = Repo::empty();
    let mut defs = vec![def("helper", &[])];
    for owner in ["a", "b", "c", "d"] {
        defs.push((format!("test_order_{owner}"), vec!["helper".to_string()]));
    }
    for i in 0..8 {
        defs.push((format!("test_zz_{}", token(i)), vec!["helper".to_string()]));
    }
    repo.write("app/o/tests/test_core.py", &module(&defs));
    let b = brief_of(&repo);
    let tests: Vec<String> = b
        .symbols
        .iter()
        .filter(|s| s.in_tests)
        .map(|s| s.addr.short.clone())
        .collect();
    assert_eq!(
        &tests[..4],
        [
            "test_order_a",
            "test_order_b",
            "test_order_c",
            "test_order_d"
        ]
    );
    assert!(position(&tests, "helper") >= 4, "{tests:?}");
}

/// GIVEN fixture G (a module that imports order_a from the core module, plus emit called by order_a) and the request "order"
/// WHEN  brief is computed
/// THEN  the importing module is listed before emit
#[test]
fn test_given_module_importing_anchor_word_when_brief_then_module_is_in_first_tier() {
    let repo = fixture_a();
    repo.write("app/o/user.py", "from app.o.core import order_a\n");
    let list = production(&brief_of(&repo));
    assert!(
        position(&list, "user") < position(&list, "emit"),
        "{list:?}"
    );
}

/// GIVEN fixture B indexed twice into separate stores
/// WHEN  brief is computed on each
/// THEN  both results are equal, symbol by symbol and score by score
#[test]
fn test_given_same_tree_twice_when_brief_then_symbols_are_identical() {
    let first = brief_of(&fixture_b());
    let second = brief_of(&fixture_b());
    assert!(!first.symbols.is_empty());
    assert_eq!(first.symbols, second.symbols);
}
