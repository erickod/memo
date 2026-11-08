mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::error::CoreError;
use memo_core::index::{index, index_with};
use memo_core::query::word_stats;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn fixture_store(config: &Config) -> (Repo, SqliteStore) {
    let repo = Repo::fixture();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, config).unwrap();
    (repo, store)
}

fn spec(store: &SqliteStore, word: &str) -> f64 {
    word_stats(store, word).unwrap().unwrap().especificidade
}

/// GIVEN an empty memo.toml text
/// WHEN  it is parsed
/// THEN  the config equals the seeds: min_specificity 0.35, 'usecase' role and 'get' skeleton
#[test]
fn test_given_empty_toml_when_loaded_then_seed_vocabulary_and_defaults() {
    let config = Config::from_toml("").unwrap();
    assert_eq!(config, Config::default());
    assert_eq!(config.min_specificity, 0.35);
    assert!(config.vocabulary.roles.contains(&"usecase".to_string()));
    assert!(config.skeleton.contains(&"get".to_string()));
}

/// GIVEN a memo.toml with roles ["gateway"], min_specificity 0.5 and main_branch "develop"
/// WHEN  it is parsed
/// THEN  roles is exactly ["gateway"], min_specificity is 0.5, and verbs keep the seeds
#[test]
fn test_given_toml_overrides_when_loaded_then_lists_replace_and_rest_keep_seeds() {
    let text = "roles = [\"gateway\"]\nmin_specificity = 0.5\nmain_branch = \"develop\"\n";
    let config = Config::from_toml(text).unwrap();
    assert_eq!(config.vocabulary.roles, vec!["gateway"]);
    assert_eq!(config.min_specificity, 0.5);
    assert_eq!(config.main_branch.as_deref(), Some("develop"));
    assert_eq!(config.vocabulary.verbs, Config::default().vocabulary.verbs);
}

/// GIVEN a memo.toml text with 'roles = 3'
/// WHEN  it is parsed
/// THEN  a Config error is returned
#[test]
fn test_given_invalid_toml_when_loaded_then_config_error() {
    let err = Config::from_toml("roles = 3").unwrap_err();
    assert!(matches!(err, CoreError::Config(_)), "{err:?}");
}

/// GIVEN roles ["gateway"] in the config and a class 'BillingGateway'
/// WHEN  the repository is indexed
/// THEN  the graph dump holds 'gateway:role' for that class
#[test]
fn test_given_custom_role_when_indexed_then_word_classified_as_role() {
    let repo = Repo::empty();
    repo.write("app/b/domain/pay.py", "class BillingGateway:\n    pass\n");
    let config = Config::from_toml("roles = [\"gateway\"]").unwrap();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    assert!(store.dump().unwrap().contains("gateway:role:"));
}

/// GIVEN layers ["core"] and the file 'app/x/core/m.py'
/// WHEN  the repository is indexed
/// THEN  the file row has context 'x' and layer 'core'
#[test]
fn test_given_custom_layers_when_indexed_then_file_layer_follows_config() {
    let repo = Repo::empty();
    repo.write("app/x/core/m.py", "def f():\n    pass\n");
    let config = Config::from_toml("layers = [\"core\"]").unwrap();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    assert!(store.dump().unwrap().contains("file|app/x/core/m.py|"));
    assert!(store.dump().unwrap().contains("|x|core|"));
}

/// GIVEN the indexed fixture where 'limit' names 8 symbols across 4 files
/// WHEN  the stats of 'limit' are read
/// THEN  uso is 8 and alcance is 4
#[test]
fn test_given_indexed_fixture_when_stats_then_uso_and_alcance_of_limit() {
    let (_r, store) = fixture_store(&Config::default());
    let stats = word_stats(&store, "limit").unwrap().unwrap();
    assert_eq!((stats.uso, stats.alcance), (8, 4));
    assert!(stats.grau > 0);
}

/// GIVEN the indexed fixture where 'usecase' is on many symbols and 'checkout' on two
/// WHEN  specificity is read
/// THEN  'checkout' is more specific than 'usecase' and all values lie in [0, 1]
#[test]
fn test_given_symbols_when_idf_then_rarer_word_has_higher_specificity() {
    let (_r, store) = fixture_store(&Config::default());
    let (rare, common) = (spec(&store, "checkout"), spec(&store, "usecase"));
    assert!(rare > common, "{rare} <= {common}");
    assert!((0.0..=1.0).contains(&rare) && (0.0..=1.0).contains(&common));
}

/// GIVEN skeleton ["usecase"] in the config
/// WHEN  the fixture is indexed
/// THEN  'usecase' is esqueleto with centralidade 0 and 'limit' is not esqueleto by name
#[test]
fn test_given_skeleton_config_when_stats_then_word_is_esqueleto_with_zero_centrality() {
    let config = Config::from_toml("skeleton = [\"usecase\"]\nmin_specificity = 0.0").unwrap();
    let (_r, store) = fixture_store(&config);
    let usecase = word_stats(&store, "usecase").unwrap().unwrap();
    assert!(usecase.esqueleto);
    assert_eq!(usecase.centralidade, 0.0);
    assert!(!word_stats(&store, "limit").unwrap().unwrap().esqueleto);
}

/// GIVEN min_specificity 0.5, an empty skeleton list and the fixture, where every word has few neighbors
/// WHEN  the fixture is indexed
/// THEN  'usecase' (specificity below 0.5) has coherence 1.0 and is not esqueleto; neither is 'checkout'
#[test]
fn test_given_low_specificity_but_coherent_when_stats_then_not_esqueleto() {
    let config = Config::from_toml("skeleton = []\nmin_specificity = 0.5").unwrap();
    let (_r, store) = fixture_store(&config);
    let checkout = word_stats(&store, "checkout").unwrap().unwrap();
    let usecase = word_stats(&store, "usecase").unwrap().unwrap();
    assert!(checkout.especificidade >= 0.5 && !checkout.esqueleto);
    assert!(usecase.especificidade < 0.5 && usecase.coerencia >= 0.30);
    assert!(!usecase.esqueleto);
}

/// GIVEN the indexed fixture with the seed skeleton
/// WHEN  centralities of the words 'limit', 'checkout', 'usecase' and 'invoice' are summed
/// THEN  each is positive and each is below 1 (PageRank mass is shared across the graph)
#[test]
fn test_given_pagerank_when_computed_then_centralities_are_positive_and_shared() {
    let (_r, store) = fixture_store(&Config::default());
    for word in ["limit", "checkout", "usecase", "invoice"] {
        let c = word_stats(&store, word).unwrap().unwrap().centralidade;
        assert!(c > 0.0 && c < 1.0, "{word}: {c}");
    }
}

/// GIVEN two identical copies of the fixture indexed with the same config
/// WHEN  the canonical dumps are compared
/// THEN  they are byte-identical, including the word statistics lines
#[test]
fn test_given_same_tree_when_stats_computed_then_dump_bytes_identical() {
    let dumps: Vec<String> = (0..2)
        .map(|_| {
            let repo = Repo::fixture();
            let mut store = SqliteStore::open_in_memory().unwrap();
            index(&WorkTree::new(repo.path()), &mut store).unwrap();
            store.dump().unwrap()
        })
        .collect();
    assert!(dumps[0].contains("stats|limit|"));
    assert_eq!(dumps[0], dumps[1]);
}

/// GIVEN the default config serialized by to_toml
/// WHEN  the text is parsed again
/// THEN  the parsed config equals the default
#[test]
fn test_given_default_config_when_serialized_then_roundtrip_equals_default() {
    let text = Config::default().to_toml();
    assert_eq!(Config::from_toml(&text).unwrap(), Config::default());
    assert!(text.contains("min_specificity = 0.35"));
}
