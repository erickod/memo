use memo_core::config::Config;
use memo_core::evidence::{Anchor, Evidence, classify};
use memo_core::sources::use_words;
use std::collections::BTreeSet;

/// Palavras de um ou mais textos, como a camada relacional as grava.
fn words(texts: &[&str]) -> BTreeSet<String> {
    let config = Config::default();
    texts
        .iter()
        .flat_map(|t| use_words(t, &config))
        .map(|w| w.text)
        .collect()
}

/// Família de `a.Alpha.run`: Alpha, SubAlpha e o Protocol Runner.
fn family() -> Vec<BTreeSet<String>> {
    vec![words(&["Alpha"]), words(&["SubAlpha"]), words(&["Runner"])]
}

fn module_anchor<'a>(use_module: &'a str, imports: &'a [String]) -> Anchor<'a> {
    Anchor::Module {
        target: "m.helper",
        target_module: "m",
        use_module,
        imports,
    }
}

/// GIVEN target a.Alpha.run and a use 'Alpha(1).run'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_use_with_owner_class_word_when_classified_then_strong() {
    let fam = family();
    let got = classify(
        &words(&["Alpha(1).run"]),
        &BTreeSet::new(),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target a.Alpha.run, family {Alpha, SubAlpha, Runner} and a use 'SubAlpha(1).run'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_use_with_subclass_word_when_classified_then_strong() {
    let fam = vec![words(&["SubAlpha"])];
    let got = classify(
        &words(&["SubAlpha(1).run"]),
        &BTreeSet::new(),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target a.Alpha.run, family with Runner and a use 'Runner.run'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_use_with_protocol_word_when_classified_then_strong() {
    let fam = family();
    let got = classify(
        &words(&["Runner.run"]),
        &BTreeSet::new(),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN a use 'it.run' and the symbol's other words from 'list[Alpha]'
/// WHEN  classified for a.Alpha.run
/// THEN  medium
#[test]
fn test_given_owner_class_only_in_other_use_when_classified_then_medium() {
    let fam = family();
    let got = classify(
        &words(&["it.run"]),
        &words(&["list[Alpha]"]),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::Media);
}

/// GIVEN a use 'Beta(1).run' and no family word in the symbol
/// WHEN  classified for a.Alpha.run
/// THEN  name only
#[test]
fn test_given_no_family_word_when_classified_then_name_only() {
    let fam = family();
    let got = classify(
        &words(&["Beta(1).run"]),
        &words(&["Beta"]),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::SoNome);
}

/// GIVEN target m.helper and a use 'helper' in module m
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_function_target_same_module_when_classified_then_strong() {
    let got = classify(
        &words(&["helper"]),
        &BTreeSet::new(),
        &module_anchor("m", &[]),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'helper' in a file importing 'm.helper'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_function_target_imported_when_classified_then_strong() {
    let imports = vec!["m.helper".to_string()];
    let got = classify(
        &words(&["helper"]),
        &BTreeSet::new(),
        &module_anchor("n", &imports),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'm.helper' in a file importing 'm'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_module_of_target_imported_when_classified_then_strong() {
    let imports = vec!["m".to_string()];
    let got = classify(
        &words(&["m.helper"]),
        &BTreeSet::new(),
        &module_anchor("k", &imports),
    );
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'helper' in module w, which imports neither m nor m.helper
/// WHEN  classified
/// THEN  name only
#[test]
fn test_given_function_target_not_imported_when_classified_then_name_only() {
    let imports = vec!["z.helper".to_string(), "mm".to_string()];
    let got = classify(
        &words(&["helper"]),
        &BTreeSet::new(),
        &module_anchor("w", &imports),
    );
    assert_eq!(got, Evidence::SoNome);
}
