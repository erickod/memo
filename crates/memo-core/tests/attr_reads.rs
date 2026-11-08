mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::query::readers;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn store_of(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

/// (qname, aproximado) de quem lê o atributo.
fn reader_list(store: &SqliteStore, attr: &str) -> Vec<(String, bool)> {
    readers(store, attr)
        .unwrap()
        .items
        .into_iter()
        .map(|i| (i.addr.qname, i.approx))
        .collect()
}

const MODELS: &str = "class Order:\n    status: str = \"new\"\n    can_lock: bool = False\n";

/// GIVEN 'Order.status' read in a function that imports Order, and assigned in another
/// WHEN  readers of Order.status run
/// THEN  the reading function is listed without '~'; the assigning one is not (TASK-7, D5)
#[test]
fn test_given_class_attr_read_when_readers_then_exact_reader() {
    let (_repo, store) = store_of(&[
        ("models.py", MODELS),
        (
            "svc.py",
            "from models import Order\n\ndef check():\n    return Order.status\n\n\
             def setter():\n    Order.status = \"done\"\n",
        ),
    ]);
    let list = reader_list(&store, "models.Order.status");
    assert!(list.contains(&("svc.check".to_string(), false)), "{list:?}");
    assert!(!list.iter().any(|(q, _)| q == "svc.setter"), "{list:?}");
}

/// GIVEN 'o.can_lock' read where 'o' has no resolvable type, and can_lock exists only in Order
/// WHEN  readers of Order.can_lock run
/// THEN  the function is listed with '~' (TASK-7, D5)
#[test]
fn test_given_untyped_attr_read_when_readers_then_approx_reader() {
    let (_repo, store) = store_of(&[
        ("models.py", MODELS),
        ("svc.py", "def check(o):\n    return o.can_lock\n"),
    ]);
    let list = reader_list(&store, "models.Order.can_lock");
    assert!(list.contains(&("svc.check".to_string(), true)), "{list:?}");
}

/// GIVEN 'x.flag' read with 'x' untyped, and 'flag' declared in two classes
/// WHEN  readers of each class's flag run
/// THEN  the function is in neither: an approximate read needs a name unique in the repository (TASK-7, D5a)
#[test]
fn test_given_attr_in_two_classes_when_readers_then_no_relation() {
    let (_repo, store) = store_of(&[
        (
            "models.py",
            "class A:\n    flag: bool = True\n\nclass B:\n    flag: bool = False\n",
        ),
        ("svc.py", "def check(x):\n    return x.flag\n"),
    ]);
    for attr in ["models.A.flag", "models.B.flag"] {
        let list = reader_list(&store, attr);
        assert!(
            !list.iter().any(|(q, _)| q == "svc.check"),
            "{attr}: {list:?}"
        );
    }
}

/// GIVEN 'x.nothing_here' read, and no class declares 'nothing_here'
/// WHEN  the repository is indexed
/// THEN  no read relation leaves the function (TASK-7, D5)
#[test]
fn test_given_unknown_attr_read_when_indexed_then_no_read_edge() {
    let (_repo, store) = store_of(&[
        ("models.py", MODELS),
        ("svc.py", "def check(x):\n    return x.nothing_here\n"),
    ]);
    for attr in ["models.Order.status", "models.Order.can_lock"] {
        let list = reader_list(&store, attr);
        assert!(
            !list.iter().any(|(q, _)| q == "svc.check"),
            "{attr}: {list:?}"
        );
    }
}
