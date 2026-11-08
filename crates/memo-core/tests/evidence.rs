use memo_core::evidence::{Anchor, Evidence, classify};

/// Textos de uso, como gravados em `raw_ref` (texto da chamada e receptor) ou no tipo anotado.
fn texts<'a>(items: &[&'a str]) -> Vec<&'a str> {
    items.to_vec()
}

/// Família de `a.Alpha.run`: Alpha, SubAlpha e o Protocol Runner, pelo nome da classe.
fn family() -> Vec<String> {
    vec!["Alpha".into(), "SubAlpha".into(), "Runner".into()]
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
    let got = classify(&texts(&["Alpha(1).run"]), &[], &Anchor::Family(&fam));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target a.Alpha.run, family {Alpha, SubAlpha, Runner} and a use 'SubAlpha(1).run'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_use_with_subclass_word_when_classified_then_strong() {
    let fam = vec!["SubAlpha".to_string()];
    let got = classify(&texts(&["SubAlpha(1).run"]), &[], &Anchor::Family(&fam));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target a.Alpha.run, family with Runner and a use 'Runner.run'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_use_with_protocol_word_when_classified_then_strong() {
    let fam = family();
    let got = classify(&texts(&["Runner.run"]), &[], &Anchor::Family(&fam));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN a use 'it.run' and the symbol's other words from 'list[Alpha]'
/// WHEN  classified for a.Alpha.run
/// THEN  medium
#[test]
fn test_given_owner_class_only_in_other_use_when_classified_then_medium() {
    let fam = family();
    let got = classify(
        &texts(&["it.run"]),
        &texts(&["list[Alpha]"]),
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
        &texts(&["Beta(1).run"]),
        &texts(&["Beta"]),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::SoNome);
}

/// GIVEN target m.helper and a use 'helper' in module m
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_function_target_same_module_when_classified_then_strong() {
    let got = classify(&texts(&["helper"]), &[], &module_anchor("m", &[]));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'helper' in a file importing 'm.helper'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_function_target_imported_when_classified_then_strong() {
    let imports = vec!["m.helper".to_string()];
    let got = classify(&texts(&["helper"]), &[], &module_anchor("n", &imports));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'm.helper' in a file importing 'm'
/// WHEN  classified
/// THEN  strong
#[test]
fn test_given_module_of_target_imported_when_classified_then_strong() {
    let imports = vec!["m".to_string()];
    let got = classify(&texts(&["m.helper"]), &[], &module_anchor("k", &imports));
    assert_eq!(got, Evidence::Forte);
}

/// GIVEN target m.helper and a use 'helper' in module w, which imports neither m nor m.helper
/// WHEN  classified
/// THEN  name only
#[test]
fn test_given_function_target_not_imported_when_classified_then_name_only() {
    let imports = vec!["z.helper".to_string(), "mm".to_string()];
    let got = classify(&texts(&["helper"]), &[], &module_anchor("w", &imports));
    assert_eq!(got, Evidence::SoNome);
}

/// GIVEN family {Alpha} and a use 'AlphaBeta(1).run'
/// WHEN  classified
/// THEN  name only: 'AlphaBeta' is not the identifier 'Alpha' (TASK-6, D18)
#[test]
fn test_given_lookalike_class_when_classified_then_not_strong() {
    let fam = vec!["Alpha".to_string()];
    let got = classify(&texts(&["AlphaBeta(1).run"]), &[], &Anchor::Family(&fam));
    assert_eq!(got, Evidence::SoNome);
}

/// GIVEN family {OrderDebtCodeRepository} and a use 'ClearOrderDebtCodeRepository(s).run'
/// WHEN  classified
/// THEN  name only (TASK-6, D18)
#[test]
fn test_given_protocol_lookalike_when_classified_then_name_only() {
    let fam = vec!["OrderDebtCodeRepository".to_string()];
    let got = classify(
        &texts(&["ClearOrderDebtCodeRepository(s).run"]),
        &texts(&["ClearOrderDebtCodeRepository"]),
        &Anchor::Family(&fam),
    );
    assert_eq!(got, Evidence::SoNome);
}
