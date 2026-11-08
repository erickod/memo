mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::query::word_stats;
use memo_core::source::WorkTree;
use memo_core::stats::coherence;
use memo_core::store::SqliteStore;

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

fn build(files: &[(String, String)], config: &str) -> (Repo, SqliteStore, Config) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let config = Config::from_toml(config).unwrap();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    (repo, store, config)
}

fn module(dir: &str, name: &str, defs: &[String]) -> (String, String) {
    let body: String = defs
        .iter()
        .map(|d| format!("def {d}():\n    return 1\n\n"))
        .collect();
    (format!("app/{dir}/{name}.py"), body)
}

/// Palavra frequente com poucos vizinhos fixos: 40 módulos, cada um com `order_placed`,
/// `order_canceled`, `order_confirmed` e `order_failed`.
pub fn order_like() -> Vec<(String, String)> {
    let defs: Vec<String> = ["placed", "canceled", "confirmed", "failed"]
        .iter()
        .map(|s| format!("order_{s}"))
        .collect();
    (0..40)
        .map(|i| module("o", &format!("m{}", token(i)), &defs))
        .collect()
}

/// Palavra frequente ligada a tudo: 50 módulos, cada um com `get_<palavra distinta>`.
pub fn get_like() -> Vec<(String, String)> {
    (0..50)
        .map(|i| {
            module(
                "g",
                &format!("g{}", token(i)),
                &[format!("get_{}", token(i + 200))],
            )
        })
        .collect()
}

const OPEN: &str = "skeleton = []\nmin_specificity = 0.35\nmin_coherence = 0.30\n";

fn stats(store: &SqliteStore, word: &str) -> memo_core::query::WordStats {
    word_stats(store, word).unwrap().unwrap()
}

/// GIVEN the neighbors n01..n10 with weight 2.0 and n11, n12 with weight 1.0
/// WHEN  the coherence is computed
/// THEN  it is the weight of the ten strongest over the total: 20 / 22
#[test]
fn test_given_more_than_ten_neighbors_when_coherence_then_top_ten_weight_over_total() {
    let mut neighbors: Vec<(String, f64)> = (1..=10).map(|i| (format!("n{i:02}"), 2.0)).collect();
    neighbors.extend([("n11".to_string(), 1.0), ("n12".to_string(), 1.0)]);
    assert!((coherence(&neighbors) - 20.0 / 22.0).abs() < 1e-12);
    assert_eq!(coherence(&neighbors[..3]), 1.0);
    assert_eq!(coherence(&[]), 0.0);
}

/// GIVEN twelve neighbors with the same awkward weight 0.1, listed in two different orders
/// WHEN  the coherence is computed for each order
/// THEN  both results are bit-identical (alphabetical tie-break fixes the summation order)
#[test]
fn test_given_equal_neighbor_weights_when_coherence_then_alphabetical_tie_break() {
    let forward: Vec<(String, f64)> = (0..12).map(|i| (format!("n{i:02}"), 0.1)).collect();
    let mut backward = forward.clone();
    backward.reverse();
    let mut shuffled = forward.clone();
    shuffled.swap(0, 7);
    shuffled.swap(3, 11);
    let reference = coherence(&forward).to_bits();
    assert_eq!(coherence(&backward).to_bits(), reference);
    assert_eq!(coherence(&shuffled).to_bits(), reference);
}

/// GIVEN 'order' in the skeleton list and in a repository where it has four fixed neighbors
/// WHEN  the statistics are read
/// THEN  its coherence is 1.0 and it is still esqueleto (the list overrides coherence)
#[test]
fn test_given_skeleton_list_word_when_decided_then_esqueleto_regardless_of_coherence() {
    let config = "skeleton = [\"order\"]\nmin_coherence = 0.30\n";
    let (_r, store, _c) = build(&order_like(), config);
    let order = stats(&store, "order");
    assert_eq!(order.coerencia, 1.0);
    assert!(order.esqueleto);
}

/// GIVEN 'order' used in 40 modules next to the same four words
/// WHEN  the statistics are read with min_coherence 0.30
/// THEN  its specificity is below 0.35 and its coherence is 1.0, so it is not esqueleto
#[test]
fn test_given_low_idf_and_high_coherence_when_decided_then_not_esqueleto() {
    let (_r, store, _c) = build(&order_like(), OPEN);
    let order = stats(&store, "order");
    assert!(order.especificidade < 0.35, "{order:?}");
    assert_eq!(order.coerencia, 1.0);
    assert!(!order.esqueleto);
}

/// GIVEN 'get' used in 50 modules, each next to a different word
/// WHEN  the statistics are read with min_coherence 0.30
/// THEN  its specificity is below 0.35 and its coherence is below 0.30, so it is esqueleto
#[test]
fn test_given_low_idf_and_low_coherence_when_decided_then_esqueleto() {
    let (_r, store, _c) = build(&get_like(), OPEN);
    let get = stats(&store, "get");
    assert!(get.especificidade < 0.35, "{get:?}");
    assert!(get.coerencia < 0.30, "{get:?}");
    assert!(get.esqueleto);
}

fn hub_repo(doc_words: usize) -> (Vec<(String, String)>, String) {
    let mut files: Vec<(String, String)> = (0..12)
        .map(|i| module("h", &format!("h{}", token(i)), &["hub_one".to_string()]))
        .collect();
    files.extend((0..110).map(|i| module("f", &format!("f{}", token(i)), &[token(300 + i)])));
    let words: Vec<String> = (0..doc_words).map(|i| token(300 + i)).collect();
    let test = format!(
        "from app.h.h{} import hub_one\n\ndef test_a():\n    \"\"\"GIVEN {} WHEN hub THEN one\"\"\"\n    hub_one()\n",
        token(0),
        words.join(" ")
    );
    files.push(("app/h/tests/test_hub.py".to_string(), test));
    (files, token(0))
}

/// GIVEN 'hub' with one fixed neighbor, and a test that calls 'hub_one' exactly with a short docstring
/// WHEN  the docstring is edited to list 100 identifier words and the repository is reindexed
/// THEN  'hub' is not esqueleto before and is esqueleto after (the status is recomputed)
#[test]
fn test_given_docstring_edit_when_reindexed_then_esqueleto_status_recomputed() {
    let (before, _) = hub_repo(0);
    let (repo, mut store, config) = build(&before, OPEN);
    assert!(
        !stats(&store, "hub").esqueleto,
        "{:?}",
        stats(&store, "hub")
    );
    let (after, _) = hub_repo(100);
    let (path, source) = after.last().unwrap();
    repo.write(path, source);
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    let hub = stats(&store, "hub");
    assert!(hub.coerencia < 0.30 && hub.esqueleto, "{hub:?}");
}

/// GIVEN the order-like repository indexed twice into separate stores
/// WHEN  the canonical dumps are compared
/// THEN  they are byte-identical and carry the coherence column
#[test]
fn test_given_same_tree_twice_when_index_then_coherence_and_esqueleto_bytes_identical() {
    let dumps: Vec<String> = (0..2)
        .map(|_| build(&order_like(), OPEN).1.dump().unwrap())
        .collect();
    assert!(dumps[0].contains("stats|order|"));
    assert_eq!(dumps[0], dumps[1]);
    let line = dumps[0]
        .lines()
        .find(|l| l.starts_with("stats|order|"))
        .unwrap();
    assert_eq!(line.split('|').count(), 10, "{line}");
}
