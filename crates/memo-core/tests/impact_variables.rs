mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::query::impact;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

/// GIVEN a module variable written by one function (`global`) and read by another
/// WHEN  `impact` runs on the variable
/// THEN  the writer and the reader are listed with their lines, each in its own list
#[test]
fn test_given_written_and_read_variable_when_impact_runs_then_lists_writers_and_readers() {
    let repo = Repo::empty();
    repo.write(
        "m.py",
        "X = 0\n\ndef put():\n    global X\n    X = 1\n\ndef get():\n    return X\n",
    );
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = impact(&store, "m.X").unwrap();
    let writers: Vec<(&str, Option<i64>)> = info
        .escritores
        .iter()
        .map(|i| (i.addr.qname.as_str(), i.line))
        .collect();
    let readers: Vec<(&str, Option<i64>)> = info
        .leitores
        .iter()
        .map(|i| (i.addr.qname.as_str(), i.line))
        .collect();
    assert!(writers.contains(&("m.put", Some(5))), "{writers:?}");
    assert!(readers.contains(&("m.get", Some(8))), "{readers:?}");
    assert!(readers.iter().all(|(q, _)| *q != "m.put"), "{readers:?}");
}
