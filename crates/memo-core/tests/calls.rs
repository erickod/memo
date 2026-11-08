mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::{index, index_with};
use memo_core::query::{EdgeView, coverage, symbol_info};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn indexed(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, src) in files {
        repo.write(path, src);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

fn out_edges(store: &SqliteStore, symbol: &str, kind: &str) -> Vec<EdgeView> {
    symbol_info(store, symbol)
        .unwrap()
        .edges
        .into_iter()
        .filter(|e| e.dir == "out" && e.kind == kind)
        .collect()
}

fn has(edges: &[EdgeView], other: &str, approx: bool) -> bool {
    edges.iter().any(|e| e.other == other && e.approx == approx)
}

/// GIVEN module 'm' with 'def helper()' and 'def run(): helper()'
/// WHEN  the repository is indexed
/// THEN  m.run has an exact CHAMA edge to 'helper'
#[test]
fn test_given_same_module_function_when_called_then_chama_exact() {
    let (_r, s) = indexed(&[(
        "m.py",
        "def helper():\n    pass\n\ndef run():\n    helper()\n",
    )]);
    assert!(has(&out_edges(&s, "m.run", "CHAMA"), "helper", false));
}

/// GIVEN 'from a import f' in module 'b' and 'def run(): f()'
/// WHEN  the repository is indexed
/// THEN  b.run has an exact CHAMA edge to 'f'
#[test]
fn test_given_imported_function_when_called_then_chama_exact() {
    let (_r, s) = indexed(&[
        ("a.py", "def f():\n    pass\n"),
        ("b.py", "from a import f\n\ndef run():\n    f()\n"),
    ]);
    assert!(has(&out_edges(&s, "b.run", "CHAMA"), "f", false));
}

/// GIVEN 'class Child(Base)' whose method calls 'self.m()' defined only in Base
/// WHEN  the repository is indexed
/// THEN  Child.n has an exact CHAMA edge to 'Base.m'
#[test]
fn test_given_self_method_when_called_then_chama_exact_including_base_class() {
    let src = "class Base:\n    def m(self):\n        pass\n\nclass Child(Base):\n    def n(self):\n        self.m()\n";
    let (_r, s) = indexed(&[("c.py", src)]);
    assert!(has(&out_edges(&s, "c.Child.n", "CHAMA"), "Base.m", false));
}

/// GIVEN 'class T' and 'def make(): T()'
/// WHEN  the repository is indexed
/// THEN  d.make has an exact CRIA edge to 'T'
#[test]
fn test_given_class_call_when_called_then_cria_exact() {
    let (_r, s) = indexed(&[("d.py", "class T:\n    pass\n\ndef make():\n    T()\n")]);
    assert!(has(&out_edges(&s, "d.make", "CRIA"), "T", false));
}

/// GIVEN 'x = T()' followed by 'x.go()' inside a function
/// WHEN  the repository is indexed
/// THEN  e.run has an approximate CHAMA edge to 'T.go'
#[test]
fn test_given_constructed_local_when_method_called_then_chama_approx() {
    let src = "class T:\n    def go(self):\n        pass\n\ndef run():\n    x = T()\n    x.go()\n";
    let (_r, s) = indexed(&[("e.py", src)]);
    assert!(has(&out_edges(&s, "e.run", "CHAMA"), "T.go", true));
}

/// GIVEN 'thing.only_here()' on an unannotated param and one method named 'only_here'
/// WHEN  the repository is indexed
/// THEN  f.run has an approximate CHAMA edge to 'A.only_here'
#[test]
fn test_given_unique_method_name_when_receiver_unknown_then_chama_approx() {
    let src = "class A:\n    def only_here(self):\n        pass\n\ndef run(thing):\n    thing.only_here()\n";
    let (_r, s) = indexed(&[("f.py", src)]);
    assert!(has(&out_edges(&s, "f.run", "CHAMA"), "A.only_here", true));
}

/// GIVEN 'thing.dup()' on an unannotated param and two methods named 'dup'
/// WHEN  the repository is indexed
/// THEN  g.run has no CHAMA edge and coverage counts at least 1 unresolved
#[test]
fn test_given_ambiguous_method_name_when_receiver_unknown_then_no_edge_and_not_resolved() {
    let src = "class A:\n    def dup(self):\n        pass\n\nclass B:\n    def dup(self):\n        pass\n\ndef run(thing):\n    thing.dup()\n";
    let (_r, s) = indexed(&[("g.py", src)]);
    assert!(out_edges(&s, "g.run", "CHAMA").is_empty());
    assert!(coverage(&s).unwrap().nao_resolvidas >= 1);
}

/// GIVEN 'def run(): print(1)' where 'print' is not defined in the repository
/// WHEN  the repository is indexed
/// THEN  h.run has a CHAMA edge whose external target text is 'print'
#[test]
fn test_given_unknown_name_when_called_then_chama_external_dst_text() {
    let (_r, s) = indexed(&[("h.py", "def run():\n    print(1)\n")]);
    assert!(has(&out_edges(&s, "h.run", "CHAMA"), "print", false));
}

/// GIVEN a Protocol 'Repo' with 'save', and 'U.go' calling 'self._repo.save(1)' on a typed attr
/// WHEN  the repository is indexed
/// THEN  U.go has an exact CHAMA to 'Repo.save' and an approximate ESCREVE to 'Repo.save'
#[test]
fn test_given_write_method_on_typed_attr_when_called_then_escreve_external_approx() {
    let src = "from typing import Protocol\n\nclass Repo(Protocol):\n    def save(self, v: int) -> None: ...\n\nclass U:\n    def __init__(self, *, repo: Repo):\n        self._repo = repo\n\n    def go(self):\n        self._repo.save(1)\n";
    let (_r, s) = indexed(&[("i.py", src)]);
    assert!(has(&out_edges(&s, "i.U.go", "CHAMA"), "Repo.save", false));
    assert!(has(&out_edges(&s, "i.U.go", "ESCREVE"), "Repo.save", true));
}

/// GIVEN the fixture where SqlLimitRepository has 'save(self, limit)' like Protocol LimitRepository
/// WHEN  the repository is indexed
/// THEN  SqlLimitRepository has an approximate ESPECIALIZA edge to 'LimitRepository'
#[test]
fn test_given_structural_impl_when_indexed_then_especializa_approx_edge() {
    let repo = Repo::fixture();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let edges = out_edges(&store, "SqlLimitRepository", "ESPECIALIZA");
    assert!(has(&edges, "LimitRepository", true), "{edges:?}");
}

/// GIVEN the fixture where 'self._repo' is annotated with Protocol LimitRepository
/// WHEN  the repository is indexed
/// THEN  execute has an exact CHAMA to 'LimitRepository.save' and an approximate one to 'SqlLimitRepository.save'
#[test]
fn test_given_protocol_dep_when_save_called_then_protocol_edge_exact_impl_edge_approx() {
    let repo = Repo::fixture();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let edges = out_edges(&store, "CalculateLimitUseCase.execute", "CHAMA");
    assert!(has(&edges, "LimitRepository.save", false), "{edges:?}");
    assert!(has(&edges, "SqlLimitRepository.save", true), "{edges:?}");
}

const ORDER: &str = "class Order:\n    def cancel(self):\n        return 1\n";
const RESERVATION: &str = "class Reservation:\n    def cancel(self):\n        return 2\n";
const OTHER: &str = "class Other:\n    def run(self):\n        return 3\n";

fn cancel_files(imports: &str) -> Vec<(&'static str, String)> {
    vec![
        ("a.py", ORDER.to_string()),
        ("b.py", RESERVATION.to_string()),
        ("o.py", OTHER.to_string()),
        (
            "c.py",
            format!("{imports}\n\ndef run(thing):\n    thing.cancel()\n"),
        ),
    ]
}

fn index_cancel(imports: &str) -> (Repo, SqliteStore) {
    let files = cancel_files(imports);
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, s)| (*p, s.as_str())).collect();
    indexed(&refs)
}

/// GIVEN two classes with a 'cancel' method and a file that imports only Order, then calls
///       'thing.cancel()' on an unannotated receiver
/// WHEN  the repository is indexed
/// THEN  c.run has an approximate CHAMA edge to 'Order.cancel' and the call counts as resolved
#[test]
fn test_given_ambiguous_call_and_one_imported_class_when_resolved_then_chama_approx() {
    let (_r, s) = index_cancel("from a import Order");
    let edges = out_edges(&s, "c.run", "CHAMA");
    assert!(has(&edges, "Order.cancel", true), "{edges:?}");
    assert_eq!(coverage(&s).unwrap().nao_resolvidas, 0);
}

/// GIVEN the same two classes and a file that imports both of them
/// WHEN  the repository is indexed
/// THEN  c.run has no CHAMA edge and the call stays unresolved
#[test]
fn test_given_two_imported_classes_with_method_when_resolved_then_no_edge() {
    let (_r, s) = index_cancel("from a import Order\nfrom b import Reservation");
    assert!(out_edges(&s, "c.run", "CHAMA").is_empty());
    assert!(coverage(&s).unwrap().nao_resolvidas >= 1);
}

/// GIVEN a file that imports only a class without a 'cancel' method
/// WHEN  the repository is indexed
/// THEN  c.run has no CHAMA edge (the import is no evidence for an unrelated method)
#[test]
fn test_given_import_of_class_without_the_method_when_resolved_then_no_edge() {
    let (_r, s) = index_cancel("from o import Other");
    assert!(out_edges(&s, "c.run", "CHAMA").is_empty());
}

/// GIVEN c.run resolved to Order.cancel through its import
/// WHEN  the import is removed and the repository is reindexed
/// THEN  the edge is gone
#[test]
fn test_given_import_removed_when_reindexed_then_import_evidence_edge_disappears() {
    let (repo, mut s) = index_cancel("from a import Order");
    assert!(has(&out_edges(&s, "c.run", "CHAMA"), "Order.cancel", true));
    repo.write("c.py", "def run(thing):\n    thing.cancel()\n");
    index_with(&WorkTree::new(repo.path()), &mut s, &Config::default()).unwrap();
    assert!(out_edges(&s, "c.run", "CHAMA").is_empty());
}

/// GIVEN 'def go(): Alpha(1).run()' where 'run' exists in both Alpha and Beta
/// WHEN  the repository is indexed
/// THEN  go has an approximate CHAMA edge to Alpha.run only (the creation names the class)
#[test]
fn test_given_call_on_creation_when_indexed_then_chama_approx_to_created_class() {
    let (_r, s) = indexed(&[
        (
            "a.py",
            "class Alpha:\n    def run(self):\n        pass\n\nclass Beta:\n    def run(self):\n        pass\n",
        ),
        (
            "m.py",
            "from a import Alpha, Beta\n\ndef go():\n    Alpha(1).run()\n",
        ),
    ]);
    let edges = out_edges(&s, "m.go", "CHAMA");
    assert!(has(&edges, "Alpha.run", true), "{edges:?}");
    assert!(!edges.iter().any(|e| e.other == "Beta.run"), "{edges:?}");
}

/// GIVEN 'Child(Base)' where only Base defines 'run', and 'def go(): Child().run()'
/// WHEN  the repository is indexed
/// THEN  go has an approximate CHAMA edge to Base.run
#[test]
fn test_given_call_on_creation_of_subclass_when_indexed_then_inherited_method_found() {
    let (_r, s) = indexed(&[(
        "m.py",
        "class Base:\n    def run(self):\n        pass\n\nclass Other:\n    def run(self):\n        pass\n\nclass Child(Base):\n    pass\n\ndef go():\n    Child().run()\n",
    )]);
    assert!(has(&out_edges(&s, "m.go", "CHAMA"), "Base.run", true));
}

/// GIVEN 'Alpha(Beta()).run()' where both classes define 'run'
/// WHEN  the repository is indexed
/// THEN  no edge is created for run (two creations on the line have the method) and it stays unresolved
#[test]
fn test_given_two_creations_with_method_when_indexed_then_call_stays_unresolved() {
    let (_r, s) = indexed(&[(
        "m.py",
        "class Alpha:\n    def __init__(self, b):\n        pass\n    def run(self):\n        pass\n\nclass Beta:\n    def run(self):\n        pass\n\nclass Gamma:\n    def run(self):\n        pass\n\ndef go():\n    Alpha(Beta()).run()\n",
    )]);
    let edges = out_edges(&s, "m.go", "CHAMA");
    assert!(
        !edges.iter().any(|e| e.other.ends_with(".run")),
        "{edges:?}"
    );
    assert!(coverage(&s).unwrap().nao_resolvidas >= 1);
}
