mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

const EXECUTE: &str =
    "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute";

/// GIVEN the fixture repository already indexed once
/// WHEN  it is indexed a second time with no file changes
/// THEN  the report says nothing was written and the row-change and event counts are unchanged
#[test]
fn test_given_indexed_repo_when_indexed_again_then_no_writes_no_events() {
    let repo = Repo::fixture();
    let tree = WorkTree::new(repo.path());
    let mut store = SqliteStore::open_in_memory().unwrap();
    let first = index(&tree, &mut store).unwrap();
    assert!(first.wrote);
    let (changes, events) = (store.total_changes(), store.event_count().unwrap());

    let second = index(&tree, &mut store).unwrap();

    assert!(!second.wrote);
    assert_eq!(store.total_changes(), changes);
    assert_eq!(store.event_count().unwrap(), events);
}

/// GIVEN the fixture plus 'app/billing/broken.py' containing 'def broken(:'
/// WHEN  the repository is indexed
/// THEN  the command succeeds, 'app/billing/broken.py' is listed in files_with_error
///       and 'CalculateLimitUseCase.execute' is still indexed
#[test]
fn test_given_file_with_syntax_error_when_indexed_then_listed_in_arquivos_com_erro() {
    let repo = Repo::fixture();
    repo.write("app/billing/broken.py", "def broken(:\n");
    let mut store = SqliteStore::open_in_memory().unwrap();

    let report = index(&WorkTree::new(repo.path()), &mut store).unwrap();

    assert_eq!(report.files_with_error, vec!["app/billing/broken.py"]);
    assert!(store.symbol_exists(EXECUTE).unwrap());
}

/// GIVEN two identical copies of the fixture repository
/// WHEN  each is indexed into its own empty store
/// THEN  the canonical graph dumps are byte-identical
#[test]
fn test_given_two_runs_same_tree_when_index_then_graph_bytes_identical() {
    let dumps: Vec<String> = (0..2)
        .map(|_| {
            let repo = Repo::fixture();
            let mut store = SqliteStore::open_in_memory().unwrap();
            index(&WorkTree::new(repo.path()), &mut store).unwrap();
            store.dump().unwrap()
        })
        .collect();

    assert!(!dumps[0].is_empty());
    assert_eq!(dumps[0], dumps[1]);
}

fn index_files(files: &[(&str, &str)]) -> (memo_core::index::IndexReport, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    let report = index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (report, store)
}

/// GIVEN a class with a property getter and a '@x.setter' that share the name 'x'
/// WHEN  the repository is indexed
/// THEN  indexing succeeds, 'prop.A.x' is a single symbol and the file has no parse error
#[test]
fn test_given_property_setter_when_indexed_then_one_symbol_and_no_error() {
    let source = "class A:\n    @property\n    def x(self):\n        return 1\n\n    @x.setter\n    def x(self, v):\n        self._x = v\n";
    let (report, store) = index_files(&[("prop.py", source)]);
    assert!(report.files_with_error.is_empty());
    assert!(store.symbol_exists("prop.A.x").unwrap());
}

/// GIVEN a class with an annotated attribute 'value' and a method 'value'
/// WHEN  the repository is indexed
/// THEN  indexing succeeds and the first definition ('value' as attribute) is the one kept
#[test]
fn test_given_attribute_and_method_with_same_name_when_indexed_then_first_definition_kept() {
    let source = "class B:\n    value: int = 0\n\n    def value(self):\n        return 1\n";
    let (_, store) = index_files(&[("attr.py", source)]);
    assert!(store.symbol_exists("attr.B.value").unwrap());
    assert!(
        store
            .dump()
            .unwrap()
            .contains("symbol|attr.B.value|attribute|")
    );
}

/// GIVEN a module that defines the function 'f' twice at top level
/// WHEN  the repository is indexed
/// THEN  indexing succeeds with one symbol 'twice.f'
#[test]
fn test_given_function_defined_twice_when_indexed_then_single_symbol_and_no_error() {
    let source = "def f():\n    return 1\n\ndef f():\n    return 2\n";
    let (report, store) = index_files(&[("twice.py", source)]);
    assert!(report.files_with_error.is_empty());
    assert_eq!(store.dump().unwrap().matches("symbol|twice.f|").count(), 1);
}

/// GIVEN 'pkg/__init__.py' defining 'foo', a module 'pkg/foo.py' and a module 'pkg.py' (qname 'pkg' twice)
/// WHEN  the repository is indexed
/// THEN  indexing succeeds, the three files are indexed and 'pkg.foo' and 'pkg.foo.bar' exist
#[test]
fn test_given_qname_collision_across_files_when_indexed_then_no_error_and_first_kept() {
    let (report, store) = index_files(&[
        ("pkg/__init__.py", "def foo():\n    return 1\n"),
        ("pkg/foo.py", "def bar():\n    return 2\n"),
        ("pkg.py", "x = 1\n"),
    ]);
    assert!(report.files_with_error.is_empty());
    assert_eq!(report.files_total, 3);
    assert!(store.symbol_exists("pkg.foo").unwrap());
    assert!(store.symbol_exists("pkg.foo.bar").unwrap());
}
