mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::query::{WordInfo, word_info};
use memo_core::representativeness::Representativeness;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn token(i: usize) -> String {
    format!(
        "w{}{}",
        (b'a' + (i / 26) as u8) as char,
        (b'a' + (i % 26) as u8) as char
    )
}

/// Escreve um arquivo com uma função por palavra, sem sufixo comum.
fn file(repo: &Repo, path: &str, words: &[&str]) {
    let body: String = words
        .iter()
        .map(|w| format!("def {w}():\n    return 1\n\n"))
        .collect();
    repo.write(path, &body);
}

/// A fixture da TASK-3 §7: 11 arquivos; contexto = segmento depois de `app/`.
/// alpha: a1 a2 a3 b1 raiz · beta: a1 a2 a3 b2 b3 b4 · gamma: a1 a2 b5.
fn fixture() -> Repo {
    let repo = Repo::empty();
    file(&repo, "app/a/a1.py", &["alpha", "beta", "gamma"]);
    file(&repo, "app/a/a2.py", &["alpha", "beta", "gamma"]);
    file(&repo, "app/a/a3.py", &["alpha", "beta"]);
    file(&repo, "app/a/a4.py", &["delta"]);
    file(&repo, "app/b/b1.py", &["alpha"]);
    file(&repo, "app/b/b2.py", &["beta"]);
    file(&repo, "app/b/b3.py", &["beta"]);
    file(&repo, "app/b/b4.py", &["beta"]);
    file(&repo, "app/b/b5.py", &["gamma"]);
    file(&repo, "app/b/b6.py", &["delta"]);
    file(&repo, "app/raiz.py", &["alpha"]);
    repo
}

fn card(repo: &Repo, word: &str) -> Representativeness {
    let config = Config::default();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    let info: WordInfo = word_info(&store, word, &config.bridges).unwrap();
    info.representativeness.unwrap()
}

fn neighbor_words(card: &Representativeness) -> Vec<&str> {
    card.neighbors.iter().map(|n| n.word.as_str()).collect()
}

/// GIVEN the representativeness fixture (see TASK-3 section 7)
/// WHEN  the word card for alpha is computed
/// THEN  the repository share is 5 files over 11 files
#[test]
fn test_given_fixture_when_word_card_then_repository_share_is_files_with_word_over_files() {
    let c = card(&fixture(), "alpha");
    assert_eq!((c.repository.files, c.repository.total), (5, 11));
}

/// GIVEN the representativeness fixture
/// WHEN  the word card for alpha is computed
/// THEN  context a shows 3 of 4 files and context b shows 1 of 6 files
#[test]
fn test_given_files_in_two_contexts_when_word_card_then_share_per_context_with_size() {
    let c = card(&fixture(), "alpha");
    let rows: Vec<(&str, usize, usize)> = c
        .contexts
        .iter()
        .map(|x| (x.context.as_str(), x.files, x.total))
        .collect();
    assert_eq!(rows, vec![("a", 3, 4), ("b", 1, 6)]);
}

/// GIVEN the representativeness fixture, with alpha also in one file directly under app/
/// WHEN  the word card for alpha is computed
/// THEN  that file counts in the repository share and in no context
#[test]
fn test_given_file_without_context_when_word_card_then_counted_in_repository_only() {
    let c = card(&fixture(), "alpha");
    let in_contexts: usize = c.contexts.iter().map(|x| x.files).sum();
    assert_eq!(c.repository.files, 5);
    assert_eq!(in_contexts, 4);
}

/// GIVEN the fixture where beta shares 3 files with alpha, alpha is in 5 files and beta in 6
/// WHEN  the word card for alpha is computed
/// THEN  beta is listed with P(alpha|beta) = 3/6 = 0.5, P(beta|alpha) = 3/5 = 0.6 and 3 shared files
#[test]
fn test_given_neighbor_with_three_shared_files_when_word_card_then_both_directions_listed() {
    let c = card(&fixture(), "alpha");
    let beta = c.neighbors.iter().find(|n| n.word == "beta").unwrap();
    assert_eq!(beta.shared, 3);
    assert!((beta.word_given_neighbor - 0.5).abs() < 1e-12, "{beta:?}");
    assert!((beta.neighbor_given_word - 0.6).abs() < 1e-12, "{beta:?}");
}

/// GIVEN the fixture where gamma shares only 2 files with alpha
/// WHEN  the word card for alpha is computed
/// THEN  gamma is not listed
#[test]
fn test_given_neighbor_with_two_shared_files_when_word_card_then_neighbor_not_listed() {
    let c = card(&fixture(), "alpha");
    assert_eq!(neighbor_words(&c), vec!["beta"]);
}

/// GIVEN a word that shares 3 or more files with 12 neighbors, with distinct shared counts
/// WHEN  the word card is computed
/// THEN  10 neighbors are listed, by shared files descending, and the 2 weakest are absent
#[test]
fn test_given_more_than_ten_neighbors_when_word_card_then_ten_strongest_listed_in_order() {
    let repo = Repo::empty();
    for f in 0..14 {
        let mut words = vec!["hub".to_string()];
        for k in 1..=12usize {
            if f < k + 2 {
                words.push(token(100 + k));
            }
        }
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        file(&repo, &format!("app/h/h{}.py", token(f)), &refs);
    }
    let c = card(&repo, "hub");
    let expected: Vec<String> = (3..=12).rev().map(|k| token(100 + k)).collect();
    let got: Vec<String> = c.neighbors.iter().map(|n| n.word.clone()).collect();
    assert_eq!(got, expected);
    let shared: Vec<usize> = c.neighbors.iter().map(|n| n.shared).collect();
    assert_eq!(shared, (5..=14).rev().collect::<Vec<usize>>());
}

/// GIVEN a word with three neighbors that each share 3 files with it
/// WHEN  the word card is computed
/// THEN  the neighbors are listed in alphabetical order
#[test]
fn test_given_equal_shared_files_when_word_card_then_ties_ordered_alphabetically() {
    let repo = Repo::empty();
    for f in 0..3 {
        file(
            &repo,
            &format!("app/t/t{}.py", token(f)),
            &["hub", "cat", "ant", "bee"],
        );
    }
    let c = card(&repo, "hub");
    assert_eq!(neighbor_words(&c), vec!["ant", "bee", "cat"]);
}

/// GIVEN a word that appears in one file and shares 3 or more files with no other word
/// WHEN  the word card is computed
/// THEN  the neighbor section is empty and the repository share is still shown
#[test]
fn test_given_word_without_neighbors_when_word_card_then_neighbor_section_is_empty() {
    let repo = Repo::empty();
    file(&repo, "app/s/solo.py", &["solo"]);
    file(&repo, "app/s/other.py", &["other"]);
    let c = card(&repo, "solo");
    assert!(c.neighbors.is_empty());
    assert_eq!((c.repository.files, c.repository.total), (1, 2));
}
