mod common;

use common::Repo;
use memo_core::extract::{RefKind, SymbolKind, extract_python};
use memo_core::index::index;
use memo_core::query::{readers, writers};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

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

fn store_of(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

/// (qname, aproximado) de quem escreve (`true`) ou lê (`false`) o alvo.
fn who(store: &SqliteStore, target: &str, write: bool) -> Vec<(String, bool)> {
    let relation = if write {
        writers(store, target)
    } else {
        readers(store, target)
    };
    relation
        .unwrap()
        .items
        .into_iter()
        .map(|i| (i.addr.qname, i.approx))
        .collect()
}

const MODELS: &str = "class Foo:\n    attr = 1\n";
const IMPORT_FOO: &str = "from models import Foo\n\n";

/// GIVEN `X = 1` at module level
/// WHEN  the file is indexed and resolved
/// THEN  the module has an ESCREVE edge to `m.X`
#[test]
fn test_given_plain_assignment_in_module_when_resolved_then_writes_edge_from_module() {
    let (_r, store) = store_of(&[("m.py", "X = 1\n")]);
    assert!(who(&store, "m.X", true).contains(&("m".to_string(), false)));
}

/// GIVEN `def f(): global X; X += 1`
/// WHEN  the file is indexed and resolved
/// THEN  `m.f` has an ESCREVE edge to `m.X`
#[test]
fn test_given_global_and_augmented_assign_when_resolved_then_function_writes_variable() {
    let (_r, store) = store_of(&[("m.py", "X = 0\n\ndef f():\n    global X\n    X += 1\n")]);
    assert!(who(&store, "m.X", true).contains(&("m.f".to_string(), false)));
}

/// GIVEN `def g(): return X` reading the module variable
/// WHEN  the file is indexed and resolved
/// THEN  `m.g` has an exact LE edge to `m.X`
#[test]
fn test_given_read_of_module_variable_in_function_when_resolved_then_reads_edge() {
    let (_r, store) = store_of(&[("m.py", "X = 0\n\ndef g():\n    return X\n")]);
    assert!(who(&store, "m.X", false).contains(&("m.g".to_string(), false)));
}

/// GIVEN `Foo.attr = 2` inside a function that imports Foo
/// WHEN  the files are indexed and resolved
/// THEN  the function has an ESCREVE edge to `models.Foo.attr`
#[test]
fn test_given_class_attribute_assignment_when_resolved_then_writes_class_attribute() {
    let src = format!("{IMPORT_FOO}def f():\n    Foo.attr = 2\n");
    let (_r, store) = store_of(&[("models.py", MODELS), ("svc.py", &src)]);
    assert!(who(&store, "models.Foo.attr", true).contains(&("svc.f".to_string(), false)));
}

/// GIVEN `x = Foo(); x.attr`
/// WHEN  the files are indexed and resolved
/// THEN  the function has an exact LE edge to `models.Foo.attr`
#[test]
fn test_given_instance_of_known_class_when_attr_read_then_resolves_exactly() {
    let src = format!("{IMPORT_FOO}def f():\n    x = Foo()\n    return x.attr\n");
    let (_r, store) = store_of(&[("models.py", MODELS), ("svc.py", &src)]);
    assert!(who(&store, "models.Foo.attr", false).contains(&("svc.f".to_string(), false)));
}

/// GIVEN `x = Foo(); x.attr = 1`
/// WHEN  the files are indexed and resolved
/// THEN  the function has an exact ESCREVE edge to `models.Foo.attr`
#[test]
fn test_given_instance_of_known_class_when_attr_written_then_resolves_exactly() {
    let src = format!("{IMPORT_FOO}def f():\n    x = Foo()\n    x.attr = 1\n");
    let (_r, store) = store_of(&[("models.py", MODELS), ("svc.py", &src)]);
    assert!(who(&store, "models.Foo.attr", true).contains(&("svc.f".to_string(), false)));
}

/// GIVEN `def f(x): return x.attr` with an untyped receiver
/// WHEN  the files are indexed and resolved
/// THEN  there is no exact edge to `models.Foo.attr`
#[test]
fn test_given_receiver_without_known_type_when_attr_read_then_not_invented() {
    let src = "def f(x):\n    return x.attr\n";
    let (_r, store) = store_of(&[("models.py", MODELS), ("svc.py", src)]);
    assert!(!who(&store, "models.Foo.attr", false).contains(&("svc.f".to_string(), false)));
}

/// GIVEN `X = 1` and `def f(X): return X`
/// WHEN  the file is indexed and resolved
/// THEN  `m.f` has no LE edge to the module variable
#[test]
fn test_given_parameter_named_like_module_variable_when_resolved_then_no_edge() {
    let (_r, store) = store_of(&[("m.py", "X = 1\n\ndef f(X):\n    return X\n")]);
    assert!(who(&store, "m.X", false).iter().all(|(q, _)| q != "m.f"));
}

/// GIVEN `X = 1` and `def f(): X = 2` without `global`
/// WHEN  the file is indexed and resolved
/// THEN  `m.f` has no ESCREVE edge to the module variable
#[test]
fn test_given_local_assignment_without_global_when_resolved_then_no_module_write() {
    let (_r, store) = store_of(&[("m.py", "X = 1\n\ndef f():\n    X = 2\n")]);
    assert!(who(&store, "m.X", true).iter().all(|(q, _)| q != "m.f"));
}

/// GIVEN `self.a = 1` in `__init__`
/// WHEN  the file is extracted
/// THEN  `app.m.A.a` is an Attribute and `__init__` has an ESCREVE ref with text `a`
#[test]
fn test_given_self_attribute_write_when_indexed_then_existing_edges_are_unchanged() {
    let src = "class A:\n    def __init__(self):\n        self.a = 1\n";
    let facts = extract_python(MODULE, src).unwrap();
    assert!(
        facts
            .symbols
            .iter()
            .any(|s| s.qname == "app.m.A.a" && s.kind == SymbolKind::Attribute)
    );
    assert!(
        facts.refs.iter().any(|r| r.src_qname == "app.m.A.__init__"
            && r.kind == RefKind::Escreve
            && r.text == "a")
    );
}

/// GIVEN `from m import X` and `def f(): global X; X = 2` in another module
/// WHEN  the files are indexed and resolved
/// THEN  `svc.f` has no ESCREVE edge to `m.X`, since `global` rebinds the importing module's name
#[test]
fn test_given_global_of_imported_name_when_resolved_then_no_write_to_origin() {
    let src = "from m import X\n\ndef f():\n    global X\n    X = 2\n";
    let (_r, store) = store_of(&[("m.py", "X = 1\n"), ("svc.py", src)]);
    assert!(who(&store, "m.X", true).iter().all(|(q, _)| q != "svc.f"));
}

/// GIVEN `X = 1` and a function whose inner function declares `nonlocal X`
/// WHEN  the file is indexed and resolved
/// THEN  the outer function has no ESCREVE edge to the module variable
#[test]
fn test_given_nonlocal_name_when_resolved_then_no_module_write() {
    let src = "X = 1\n\ndef outer():\n    X = 0\n\n    def inner():\n        nonlocal X\n        X = 2\n    return inner\n";
    let (_r, store) = store_of(&[("m.py", src)]);
    assert!(who(&store, "m.X", true).iter().all(|(q, _)| q != "m.outer"));
}

/// GIVEN `class A: y = X` reading the module variable in the class body
/// WHEN  the file is indexed and resolved
/// THEN  `m.A` has a LE edge to `m.X`
#[test]
fn test_given_variable_read_in_class_body_when_resolved_then_class_reads_it() {
    let (_r, store) = store_of(&[("m.py", "X = 1\n\nclass A:\n    y = X\n")]);
    assert!(who(&store, "m.X", false).contains(&("m.A".to_string(), false)));
}

/// GIVEN `def f(a=X)` using the module variable as a default value
/// WHEN  the file is indexed and resolved
/// THEN  the module has a LE edge to `m.X`, since defaults are evaluated at definition time
#[test]
fn test_given_variable_as_default_argument_when_resolved_then_module_reads_it() {
    let (_r, store) = store_of(&[("m.py", "X = 1\n\ndef f(a=X):\n    return a\n")]);
    assert!(who(&store, "m.X", false).contains(&("m".to_string(), false)));
}

/// GIVEN `@deco(X)` decorating a function
/// WHEN  the file is indexed and resolved
/// THEN  the module has a LE edge to `m.X`
#[test]
fn test_given_variable_in_decorator_when_resolved_then_module_reads_it() {
    let src = "def deco(v):\n    return lambda f: f\n\nX = 1\n\n@deco(X)\ndef f():\n    return 1\n";
    let (_r, store) = store_of(&[("m.py", src)]);
    assert!(who(&store, "m.X", false).contains(&("m".to_string(), false)));
}

/// GIVEN `A, X = 1, 2`, `for X in ...` and `with ... as X` at module level
/// WHEN  the file is indexed and resolved
/// THEN  the module has no LE edge to `m.X`, since those names are being bound, not read
#[test]
fn test_given_unpacking_and_for_target_when_resolved_then_no_read_edge() {
    let src = "X = 1\nA, X = 1, 2\nfor X in range(3):\n    pass\nwith open('f') as X:\n    pass\n";
    let (_r, store) = store_of(&[("m.py", src)]);
    assert!(who(&store, "m.X", false).iter().all(|(q, _)| q != "m"));
}
