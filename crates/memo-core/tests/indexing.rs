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
