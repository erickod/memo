mod common;

use common::Repo;
use memo_core::aliases::{AliasPair, suggest};
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

const OPEN: &str = "skeleton = []\nmin_specificity = 0.35\nmin_coherence = 0.30\n";

fn build(source: &str) -> (Repo, SqliteStore, Config) {
    let repo = Repo::empty();
    repo.write("app/m.py", source);
    let db = repo.path().join(".git/aliases-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    let config = Config::from_toml(OPEN).unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    (repo, store, config)
}

fn report(store: &SqliteStore, config: &Config) -> Vec<AliasPair> {
    suggest(store, &config.bridges, &[]).unwrap()
}

fn defs(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("def {n}():\n    return 1\n\n"))
        .collect()
}

/// GIVEN the words cancel, canceled, cancelled (prefix 6, rest up to 3) and order (unrelated)
/// WHEN  the alias report runs
/// THEN  the three cancel pairs are suggested with reason 'prefixo', and nothing pairs with 'order'
#[test]
fn test_given_pair_with_common_prefix_5_and_rest_3_when_reported_then_reason_prefix() {
    let (_r, store, config) = build(&defs(&["cancel_a", "canceled_b", "cancelled_c", "order_d"]));
    let pairs = report(&store, &config);
    let prefix: Vec<(&str, &str)> = pairs
        .iter()
        .filter(|p| p.reason == "prefixo")
        .map(|p| (p.first.as_str(), p.second.as_str()))
        .collect();
    assert_eq!(
        prefix,
        vec![
            ("cancel", "canceled"),
            ("cancel", "cancelled"),
            ("canceled", "cancelled")
        ]
    );
    assert!(
        pairs
            .iter()
            .all(|p| p.first != "order" && p.second != "order"),
        "{pairs:?}"
    );
}

/// GIVEN 'invoice' and 'fatura', each used with the same neighbors alpha, beta and gamma
/// WHEN  the alias report runs
/// THEN  the pair (fatura, invoice) is suggested with reason 'vizinhança' and similarity 1.0
#[test]
fn test_given_pair_with_jaccard_over_0_6_when_reported_then_reason_neighbors() {
    let names: Vec<String> = ["invoice", "fatura"]
        .iter()
        .flat_map(|w| {
            ["alpha", "beta", "gamma"]
                .iter()
                .map(move |n| format!("{w}_{n}"))
        })
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (_r, store, config) = build(&defs(&refs));
    let pairs = report(&store, &config);
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    let pair = &pairs[0];
    assert_eq!(
        (pair.first.as_str(), pair.second.as_str()),
        ("fatura", "invoice")
    );
    assert_eq!(pair.reason, "vizinhança");
    assert!((pair.similarity - 1.0).abs() < 1e-9);
}

/// GIVEN an indexed repository with a pair to suggest
/// WHEN  the alias report runs twice
/// THEN  the outputs are identical, the row-change and event counts do not move and no note is written
#[test]
fn test_given_alias_report_when_run_then_nothing_is_written() {
    let (repo, store, config) = build(&defs(&["cancel_a", "canceled_b"]));
    let (changes, events) = (store.total_changes(), store.events().unwrap().len());
    let first = report(&store, &config);
    let second = report(&store, &config);
    assert!(!first.is_empty());
    assert_eq!(first, second);
    assert_eq!(store.total_changes(), changes);
    assert_eq!(store.events().unwrap().len(), events);
    assert!(!repo.path().join(".git/memo/notes").exists());
}
