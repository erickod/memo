mod common;

use common::Repo;
use memo_core::extract::{RefKind, extract_python_file};
use memo_core::index::index;
use memo_core::query::{symbol_info, writers};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn refs(source: &str, kind: RefKind) -> Vec<(String, String)> {
    extract_python_file("pkg.mod", false, source)
        .unwrap()
        .refs
        .into_iter()
        .filter(|r| r.kind == kind)
        .map(|r| (r.src_qname, r.text))
        .collect()
}

/// GIVEN 'class Child(Base, Protocol): ...'
/// WHEN  the file is extracted
/// THEN  there are ESPECIALIZA refs 'Base' and 'Protocol' and the class is_protocol
#[test]
fn test_given_base_class_when_extracted_then_especializa_refs_and_protocol_flag() {
    let src = "class Child(Base, Protocol):\n    pass\n";
    let got = refs(src, RefKind::Especializa);
    assert_eq!(
        got,
        vec![
            ("pkg.mod.Child".into(), "Base".into()),
            ("pkg.mod.Child".into(), "Protocol".into())
        ]
    );
    let facts = extract_python_file("pkg.mod", false, src).unwrap();
    let class = facts.symbols.iter().find(|s| s.qname == "pkg.mod.Child");
    assert!(class.unwrap().is_protocol);
}

/// GIVEN 'def f() -> list[Item]' and 'def g() -> None'
/// WHEN  the file is extracted
/// THEN  RETORNA is 'Item' for f and absent for g
#[test]
fn test_given_return_annotation_when_extracted_then_retorna_unwrapped_and_none_skipped() {
    let src = "def f() -> list[Item]:\n    pass\n\ndef g() -> None:\n    pass\n";
    assert_eq!(
        refs(src, RefKind::Retorna),
        vec![("pkg.mod.f".into(), "Item".into())]
    );
}

/// GIVEN a function with 'raise Boom("x")', 'raise Other' and a bare 'raise'
/// WHEN  the file is extracted
/// THEN  LEVANTA refs are 'Boom' and 'Other' and the bare raise adds none
#[test]
fn test_given_raise_statements_when_extracted_then_levanta_without_bare_raise() {
    let src = "def f():\n    try:\n        raise Boom('x')\n    except Boom:\n        raise\n    raise Other\n";
    assert_eq!(
        refs(src, RefKind::Levanta),
        vec![
            ("pkg.mod.f".into(), "Boom".into()),
            ("pkg.mod.f".into(), "Other".into())
        ]
    );
}

/// GIVEN a method with 'self.a = 1', 'self.b += 1', 'self.c[0] = 1' and 'self.d.append(1)'
/// WHEN  the file is extracted
/// THEN  ESCREVE refs are 'a', 'b', 'c' and 'd'
#[test]
fn test_given_self_writes_when_extracted_then_escreve_refs_for_every_write_form() {
    let src = "class C:\n    def m(self):\n        self.a = 1\n        self.b += 1\n        self.c[0] = 1\n        self.d.append(1)\n";
    let texts: Vec<String> = refs(src, RefKind::Escreve)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(texts, vec!["a", "b", "c", "d"]);
}

/// GIVEN a method reading 'self.repo.save(x)' and calling 'self.helper()'
/// WHEN  the file is extracted
/// THEN  LE has 'repo' and no ref for the method call 'helper'
#[test]
fn test_given_self_reads_when_extracted_then_le_for_attributes_not_method_calls() {
    let src = "class C:\n    def m(self, x):\n        self.repo.save(x)\n        self.helper()\n";
    assert_eq!(
        refs(src, RefKind::Le),
        vec![("pkg.mod.C.m".into(), "repo".into())]
    );
}

/// GIVEN 'from typing import TYPE_CHECKING' and an import under 'if TYPE_CHECKING:'
/// WHEN  the file is extracted
/// THEN  the guarded import is flagged type_checking and the plain import is not
#[test]
fn test_given_type_checking_import_when_extracted_then_import_flagged() {
    let src = "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from app.x import Y\n";
    let imports = extract_python_file("pkg.mod", false, src).unwrap().imports;
    let flags: Vec<(&str, bool)> = imports
        .iter()
        .map(|i| (i.target.as_str(), i.type_checking))
        .collect();
    assert_eq!(
        flags,
        vec![("typing.TYPE_CHECKING", false), ("app.x.Y", true)]
    );
}

/// GIVEN 'from . import sibling' in module 'pkg.mod' (not a package)
/// WHEN  the file is extracted
/// THEN  the import target is 'pkg.sibling'
#[test]
fn test_given_relative_import_when_extracted_then_target_is_absolute() {
    let imports = extract_python_file("pkg.mod", false, "from . import sibling\n")
        .unwrap()
        .imports;
    assert_eq!(imports[0].target, "pkg.sibling");
    assert_eq!(imports[0].local_name, "sibling");
}

/// GIVEN the fixture where collections' ChargeUseCase imports billing's SqlLimitRepository
/// WHEN  the repository is indexed and the charge module symbol is queried
/// THEN  it has an IMPORTA out-edge to 'SqlLimitRepository'
#[test]
fn test_given_forbidden_import_when_indexed_then_importa_edge_resolved() {
    let repo = Repo::fixture();
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = symbol_info(
        &store,
        "app.collections.application.usecases.charge_usecase",
    )
    .unwrap();
    assert!(
        info.edges.iter().any(|e| e.dir == "out"
            && e.kind == "IMPORTA"
            && e.other.ends_with("SqlLimitRepository")),
        "{:?}",
        info.edges
    );
}

/// GIVEN the fixture indexed with two use cases writing Invoice.status
/// WHEN  CancelInvoiceUseCase is edited to stop writing it and the repo is reindexed
/// THEN  writers of Invoice.status lists only AnticipateInvoiceUseCase.execute
#[test]
fn test_given_edited_writer_when_reindexed_then_writers_updated() {
    let repo = Repo::fixture();
    let tree = WorkTree::new(repo.path());
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&tree, &mut store).unwrap();
    assert_eq!(writers(&store, "Invoice.status").unwrap().items.len(), 2);

    repo.write(
        "app/billing/application/usecases/cancel_invoice_usecase.py",
        "from app.billing.domain.entities.invoice import Invoice\n\n\nclass CancelInvoiceUseCase:\n    def __init__(self, *, invoice: Invoice) -> None:\n        self._invoice = invoice\n\n    def execute(self) -> None:\n        return None\n",
    );
    index(&tree, &mut store).unwrap();

    let items = writers(&store, "Invoice.status").unwrap().items;
    let names: Vec<&str> = items.iter().map(|i| i.addr.short.as_str()).collect();
    assert_eq!(names, vec!["AnticipateInvoiceUseCase.execute"]);
}
