use memo_core::config::Config;

fn hash(toml: &str) -> String {
    Config::from_toml(toml).unwrap().index_hash()
}

/// GIVEN a memo.toml with min_specificity = 0.9 and min_coherence = 0.9
/// WHEN  the config is parsed
/// THEN  it succeeds and both values are kept as given
#[test]
fn test_given_min_coherence_and_min_specificity_in_toml_when_parsed_then_values_accepted() {
    let config = Config::from_toml("min_specificity = 0.9\nmin_coherence = 0.9\n").unwrap();
    assert_eq!(config.min_specificity, 0.9);
    assert_eq!(config.min_coherence, 0.9);
}

/// GIVEN one parsed config
/// WHEN  the index config hash is computed twice
/// THEN  both hashes are equal
#[test]
fn test_given_same_config_when_hashed_twice_then_hash_is_equal() {
    let config = Config::from_toml("skeleton = [\"get\"]\n").unwrap();
    assert_eq!(config.index_hash(), config.index_hash());
}

/// GIVEN two configs that differ only in the skeleton list
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_skeleton_when_hashed_then_hash_differs() {
    assert_ne!(hash("skeleton = []\n"), hash("skeleton = [\"get\"]\n"));
}

/// GIVEN two configs that differ only in context_root
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_context_root_when_hashed_then_hash_differs() {
    assert_ne!(
        hash("context_root = \"app\"\n"),
        hash("context_root = \"backend/app\"\n")
    );
}

/// GIVEN two configs that differ only in layers
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_layers_when_hashed_then_hash_differs() {
    assert_ne!(
        hash("layers = [\"domain\"]\n"),
        hash("layers = [\"domain\", \"infra\"]\n")
    );
}

/// GIVEN two configs that differ only in verbs
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_verbs_when_hashed_then_hash_differs() {
    assert_ne!(
        hash("verbs = [\"get\"]\n"),
        hash("verbs = [\"get\", \"place\"]\n")
    );
}

/// GIVEN two configs that differ only in roles
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_roles_when_hashed_then_hash_differs() {
    assert_ne!(
        hash("roles = [\"usecase\"]\n"),
        hash("roles = [\"usecase\", \"gateway\"]\n")
    );
}

/// GIVEN two configs that differ only in one [bridges] weight
/// WHEN  the index config hash is computed for each
/// THEN  the hashes differ
#[test]
fn test_given_changed_bridge_weight_when_hashed_then_hash_differs() {
    assert_ne!(
        hash("[bridges]\nimport = 0.6\n"),
        hash("[bridges]\nimport = 0.7\n")
    );
}

/// GIVEN two configs that differ only in min_coherence (and min_specificity)
/// WHEN  the index config hash is computed for each
/// THEN  the hashes are equal, because those keys no longer affect the index
#[test]
fn test_given_changed_min_coherence_when_hashed_then_hash_is_equal() {
    assert_eq!(
        hash("min_coherence = 0.30\nmin_specificity = 0.35\n"),
        hash("min_coherence = 0.80\nmin_specificity = 0.90\n")
    );
}
