mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::index;
use memo_core::query::word_info;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn store_of(source: &str) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    repo.write("m.py", source);
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

/// Qualified names of the symbols a word search lists; empty when the word does not exist.
fn found(store: &SqliteStore, word: &str) -> Vec<String> {
    let weights = Config::from_toml("").unwrap().bridges;
    word_info(store, word, &weights)
        .map(|info| info.symbols.into_iter().map(|s| s.addr.qname).collect())
        .unwrap_or_default()
}

/// GIVEN a function whose body uses a local variable `quarantine_total`
/// WHEN  searching the word `quarantine`
/// THEN  the function is found
#[test]
fn test_given_local_identifier_in_function_when_searched_by_word_then_function_is_found() {
    let (_r, store) = store_of("def f():\n    quarantine_total = 1\n    return 1\n");
    assert!(found(&store, "quarantine").contains(&"m.f".to_string()));
}

/// GIVEN a method whose body calls `reconcile_ledger()`
/// WHEN  searching the word `ledger`
/// THEN  the method is found
#[test]
fn test_given_identifier_in_method_body_when_searched_by_word_then_method_is_found() {
    let (_r, store) = store_of("class A:\n    def run(self):\n        reconcile_ledger()\n");
    assert!(found(&store, "ledger").contains(&"m.A.run".to_string()));
}

/// GIVEN a class body with attribute `retention_days`
/// WHEN  searching the word `retention`
/// THEN  the class is found
#[test]
fn test_given_identifier_in_class_body_when_searched_by_word_then_class_is_found() {
    let (_r, store) = store_of("class B:\n    retention_days = 1\n");
    assert!(found(&store, "retention").contains(&"m.B".to_string()));
}

/// GIVEN module-level code that uses `bootstrap_registry`
/// WHEN  searching the word `registry`
/// THEN  the module is found
#[test]
fn test_given_module_level_code_when_searched_by_word_then_module_is_found() {
    let (_r, store) = store_of("bootstrap_registry()\n");
    assert!(found(&store, "registry").contains(&"m".to_string()));
}

/// GIVEN a non-test function with docstring `Settles invoices`
/// WHEN  searching the word `invoices`
/// THEN  the function is found
#[test]
fn test_given_docstring_of_non_test_function_when_searched_then_function_is_found() {
    let (_r, store) = store_of("def settle():\n    \"\"\"Settles invoices\"\"\"\n    return 1\n");
    assert!(found(&store, "invoices").contains(&"m.settle".to_string()));
}

/// GIVEN a `# refund policy` comment inside a function
/// WHEN  searching the word `refund`
/// THEN  the function is found and the module is not linked by that comment
#[test]
fn test_given_comment_inside_function_when_searched_then_function_not_module() {
    let (_r, store) = store_of("def f():\n    # refund policy\n    return 1\n");
    let list = found(&store, "refund");
    assert!(list.contains(&"m.f".to_string()), "{list:?}");
    assert!(!list.contains(&"m".to_string()), "{list:?}");
}

/// GIVEN `__init__` whose body uses `tenant_id`
/// WHEN  searching the word `tenant`
/// THEN  `__init__` is found
#[test]
fn test_given_dunder_method_when_body_has_words_then_they_are_linked() {
    let (_r, store) = store_of("class A:\n    def __init__(self):\n        self.tenant_id = 1\n");
    assert!(found(&store, "tenant").contains(&"m.A.__init__".to_string()));
}

/// GIVEN a body that mentions `settle_invoice` twice
/// WHEN  the occurrences of `settle` owned by that symbol are counted
/// THEN  the symbol owns exactly one body occurrence of the word
#[test]
fn test_given_call_also_in_raw_ref_when_counted_then_word_is_not_double_counted() {
    let repo = Repo::empty();
    repo.write(
        "m.py",
        "def run():\n    settle_invoice()\n    settle_invoice()\n",
    );
    let db = repo.path().join(".git/inner-words.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let n: i64 = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM occurrence o JOIN word w ON w.id = o.word_id
             JOIN symbol s ON s.id = o.owner_id AND o.owner_kind = 'symbol'
             WHERE s.qname = 'm.run' AND o.source = 'body' AND w.text = 'settle'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}

/// GIVEN `self` in every method body and one method using `value`
/// WHEN  searching the word `self`
/// THEN  no symbol is linked by it, while `value` finds the method
#[test]
fn test_given_self_in_every_body_when_searched_by_word_then_it_is_not_linked() {
    let (_r, store) = store_of("class A:\n    def run(self):\n        return self.value\n");
    assert!(found(&store, "self").is_empty());
    assert!(found(&store, "value").contains(&"m.A.run".to_string()));
}
