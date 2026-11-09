use memo_core::extract::{SymbolKind, extract_python};

const MODULE: &str = "app.m";

fn symbol(source: &str, qname: &str) -> Option<(SymbolKind, usize)> {
    extract_python(MODULE, source)
        .expect("source parses")
        .symbols
        .into_iter()
        .find(|s| s.qname == qname)
        .map(|s| (s.kind, s.line_start))
}

/// GIVEN `X = 1` at module level on line 2
/// WHEN  the file is extracted
/// THEN  a symbol `app.m.X` exists with its line
#[test]
fn test_given_module_variable_when_indexed_then_it_is_a_queryable_symbol() {
    let found = symbol("import os\nX = 1\n", "app.m.X");
    assert_eq!(found, Some((SymbolKind::Attribute, 2)));
}

/// GIVEN `class A: x = 1` with no annotation
/// WHEN  the file is extracted
/// THEN  a symbol `app.m.A.x` exists with its line
#[test]
fn test_given_class_attribute_when_indexed_then_it_is_a_queryable_symbol() {
    let found = symbol("class A:\n    x = 1\n", "app.m.A.x");
    assert_eq!(found, Some((SymbolKind::Attribute, 2)));
}

/// GIVEN `def f(): y = 1`
/// WHEN  the file is extracted
/// THEN  no symbol is created for `y`
#[test]
fn test_given_function_local_variable_when_indexed_then_no_symbol_is_created() {
    let found = symbol("def f():\n    y = 1\n", "app.m.f.y");
    assert_eq!(found, None);
}
