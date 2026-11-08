mod common;

use common::Repo;
use memo_core::brief::brief;
use memo_core::index::index;
use memo_core::query::{callers, readers, symbol_info, tests_for, writers};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn indexed(files: &[(&str, &str)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, text) in files {
        repo.write(path, text);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

const MODEL: &str = "class Record:\n    state: str = 'new'\n\nclass Base:\n    def run(self):\n        pass\n\nclass Repository(Base):\n    pass\n";

/// GIVEN an aliased imported class with an inherited method and a competing method
/// WHEN a test constructs it and immediately calls run
/// THEN creation and the inherited call are exact and tests_for finds the test exactly.
#[test]
fn test_given_chained_constructor_when_indexed_then_exact_call_and_test() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        (
            "other.py",
            "class Other:\n    def run(self):\n        pass\n",
        ),
        (
            "test_flow.py",
            "from models import Repository as R\n\ndef test_flow():\n    R(None).run()\n",
        ),
    ]);
    let edges = symbol_info(&store, "test_flow.test_flow").unwrap().edges;
    assert!(
        edges
            .iter()
            .any(|e| e.kind == "CHAMA" && e.other == "Base.run" && !e.approx)
    );
    assert!(
        edges
            .iter()
            .any(|e| e.kind == "CRIA" && e.other == "Repository" && !e.approx)
    );
    let tests = tests_for(&store, "models.Base.run").unwrap();
    assert_eq!(tests.exact.len(), 1);
    assert!(tests.approx.is_empty());
}

/// GIVEN a module-qualified constructor and a factory sharing its method name
/// WHEN both calls are indexed
/// THEN only construction of the known class yields an exact method call.
#[test]
fn test_given_factory_and_module_constructor_when_called_then_only_class_is_exact() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        (
            "svc.py",
            "import models as m\n\ndef factory():\n    return None\n\ndef direct():\n    m.Repository(None).run()\n\ndef indirect():\n    factory().run()\n",
        ),
    ]);
    let calls = callers(&store, "models.Base.run", 1).unwrap();
    assert!(
        calls
            .items
            .iter()
            .any(|i| i.addr.as_ref().unwrap().short == "direct" && !i.approx)
    );
    assert!(
        !calls
            .items
            .iter()
            .any(|i| i.addr.as_ref().unwrap().short == "indirect" && !i.approx)
    );
}

/// GIVEN SQLAlchemy aliases, a homonymous API and direct attribute initialization
/// WHEN writers of the model state are requested
/// THEN only supported writes are listed with their distinct origins.
#[test]
fn test_given_sql_and_attribute_writes_when_writers_then_origins_without_homonyms() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        (
            "svc.py",
            "from sqlalchemy import update as change, insert\nimport sqlalchemy as sa\nfrom models import Record\n\ndef update_record():\n    return change(Record).where(Record.state == 'new').values(state='done')\n\ndef insert_record():\n    return insert(Record).values(state='new')\n\ndef qualified():\n    return sa.update(Record).values(state='done')\n\ndef make():\n    return Record(state='new')\n\ndef assign():\n    Record.state = 'done'\n\ndef unrelated(other):\n    return other.update(Record).values(state='done')\n",
        ),
    ]);
    let list = writers(&store, "models.Record.state").unwrap().items;
    for (name, origin) in [
        ("update_record", "sql:update"),
        ("insert_record", "sql:insert"),
        ("qualified", "sql:update"),
        ("make", "inicializacao"),
        ("assign", "atribuicao"),
    ] {
        assert!(
            list.iter()
                .any(|i| i.addr.short == name && i.attr.as_deref() == Some(origin) && !i.approx),
            "{name}: {list:?}"
        );
    }
    assert!(!list.iter().any(|i| i.addr.short == "unrelated"));
}

/// GIVEN a qualified symbol and several lexical distractors
/// WHEN the brief resolves the request
/// THEN the explicit symbol is first and identified by a structural tag.
#[test]
fn test_given_qualified_symbol_when_brief_then_explicit_target_first() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        ("noise.py", "def run_records():\n    pass\n"),
    ]);
    let b = brief(&store, "explain models.Base.run").unwrap();
    assert_eq!(b.symbols[0].addr.qname, "models.Base.run");
    assert!(b.symbols[0].tags.iter().any(|t| t == "simbolo"));
    assert_eq!(b.ler[0].motivo, "simbolo explicito");
}

/// GIVEN an indexed path and an unrelated similarly named module
/// WHEN a brief uses that path
/// THEN its code symbols are prioritized and py is not interpreted as a word anchor.
#[test]
fn test_given_indexed_path_when_brief_then_file_symbols_first() {
    let (_repo, store) = indexed(&[
        ("pkg/entities.py", MODEL),
        ("other/entities.py", "def noise():\n    pass\n"),
    ]);
    let b = brief(&store, "pkg/entities.py").unwrap();
    assert!(!b.symbols.is_empty());
    assert!(b.symbols.iter().all(|s| s.addr.path == "pkg/entities.py"));
    assert!(b.symbols[0].tags.iter().any(|t| t == "arquivo"));
    assert!(b.anchors.is_empty());
}

/// GIVEN two methods sharing a suffix
/// WHEN a structural brief uses that ambiguous suffix
/// THEN it reports ambiguity rather than silently selecting a lexical interpretation.
#[test]
fn test_given_ambiguous_structural_target_when_brief_then_error() {
    let (_repo, store) = indexed(&[("a.py", MODEL), ("b.py", MODEL)]);
    assert!(matches!(
        brief(&store, "Base.run"),
        Err(memo_core::error::CoreError::Ambiguous { .. })
    ));
}

/// GIVEN an approximate reader alphabetically before an exact reader
/// WHEN readers are queried
/// THEN the exact relation is first, before presentation can truncate the list.
#[test]
fn test_given_mixed_readers_when_ranked_then_exact_before_approximate() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        ("a.py", "def approximate(x):\n    return x.state\n"),
        (
            "z.py",
            "from models import Record\n\ndef exact():\n    return Record.state\n",
        ),
    ]);
    let list = readers(&store, "models.Record.state").unwrap().items;
    assert_eq!(list[0].addr.qname, "z.exact");
    assert!(!list[0].approx);
    assert!(list.last().unwrap().approx);
}

/// GIVEN two exact readers, one in the target package and an alphabetically earlier outsider
/// WHEN readers are ranked
/// THEN the structurally closer reader appears first and repeated results are stable.
#[test]
fn test_given_equal_confidence_when_readers_ranked_then_package_proximity_first() {
    let (_repo, store) = indexed(&[
        ("pkg/models.py", MODEL),
        (
            "a.py",
            "from pkg.models import Record\n\ndef read():\n    return Record.state\n",
        ),
        (
            "pkg/z.py",
            "from pkg.models import Record\n\ndef read():\n    return Record.state\n",
        ),
    ]);
    let first = readers(&store, "pkg.models.Record.state").unwrap();
    assert_eq!(first.items[0].addr.qname, "pkg.z.read");
    assert_eq!(first, readers(&store, "pkg.models.Record.state").unwrap());
}

/// GIVEN one approximate and one exact path to the same caller at depth two
/// WHEN traversal continues to depth three
/// THEN the exact path is used for downstream confidence propagation.
#[test]
fn test_given_competing_paths_when_callers_expand_then_exact_path_survives() {
    let (_repo, store) = indexed(&[(
        "m.py",
        "class Target:\n    def run(self):\n        pass\n\ndef a_weak(x):\n    x.run()\n\ndef z_exact():\n    Target().run()\n\ndef middle(x):\n    a_weak(x)\n    z_exact()\n\ndef outer(x):\n    middle(x)\n",
    )]);
    let chain = callers(&store, "m.Target.run", 3).unwrap();
    let outer = chain
        .items
        .iter()
        .find(|i| i.addr.as_ref().unwrap().short == "outer")
        .unwrap();
    assert_eq!(outer.depth, 3);
    assert!(!outer.approx);
}

/// GIVEN a base graph, a changed function, a new function and an untouched function
/// WHEN the feature brief is computed with a code comparison
/// THEN changed symbols are included after an explicit target and unrelated symbols stay out.
#[test]
fn test_given_feature_changes_when_brief_then_explicit_then_changed_symbols() {
    use memo_core::brief::brief_with_changes;
    use memo_core::config::Config;
    use memo_core::structural::changed_symbols;
    let (repo, base) = indexed(&[(
        "m.py",
        "def changed():\n    return 1\n\ndef untouched():\n    return 0\n",
    )]);
    repo.write("m.py", "def changed():\n    return 2\n\ndef untouched():\n    return 0\n\ndef added():\n    return 3\n");
    let mut feature = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut feature).unwrap();
    let changes = changed_symbols(&base, &feature).unwrap();
    assert_eq!(changes.len(), 2);
    assert!(!changes.contains(&"m.untouched".to_string()));
    let b = brief_with_changes(
        &feature,
        "m.untouched",
        &[],
        &Config::default().bridges,
        &changes,
    )
    .unwrap();
    assert_eq!(b.symbols[0].addr.qname, "m.untouched");
    assert_eq!(
        b.symbols
            .iter()
            .filter(|s| s.tags.iter().any(|t| t == "alterado"))
            .count(),
        2
    );
    let b = brief_with_changes(
        &feature,
        "review",
        &[],
        &Config::default().bridges,
        &changes,
    )
    .unwrap();
    assert_eq!(b.symbols.len(), 2);
}

/// GIVEN a large function whose first twenty lines end inside a short decision block
/// WHEN a read spot is selected
/// THEN the complete decision is included without exceeding the forty-line budget.
#[test]
fn test_given_window_inside_decision_when_read_spot_then_complete_short_block() {
    let source = format!(
        "def large():\n{}    if ready:\n        first()\n        second()\n        third()\n{}",
        "    value = 1\n".repeat(17),
        "    value = 2\n".repeat(60)
    );
    let (_repo, store) = indexed(&[("m.py", &source)]);
    let spot = &symbol_info(&store, "m.large").unwrap().ler[0];
    assert_eq!(spot.start, 1);
    assert_eq!(spot.end, 22);
}

/// GIVEN an exact constructor call in a source file
/// WHEN its body is removed and the graph is incrementally indexed
/// THEN its call and write relations disappear rather than surviving as stale facts.
#[test]
fn test_given_writes_removed_when_reindexed_then_no_stale_relations() {
    let (repo, mut store) = indexed(&[
        ("models.py", MODEL),
        (
            "svc.py",
            "from models import Record, Repository\n\ndef flow():\n    Record(state='new')\n    Repository().run()\n",
        ),
    ]);
    assert!(
        !writers(&store, "models.Record.state")
            .unwrap()
            .items
            .is_empty()
    );
    repo.write("svc.py", "def flow():\n    pass\n");
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    assert!(
        writers(&store, "models.Record.state")
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        callers(&store, "models.Base.run", 1)
            .unwrap()
            .items
            .is_empty()
    );
}

/// GIVEN a code symbol carrying the requested word and a module importing it repeatedly
/// WHEN the brief ranks direct matches
/// THEN code carrying the word precedes the import-only module regardless of accumulated score.
#[test]
fn test_given_import_hub_when_brief_then_named_code_before_import_only_module() {
    let (_repo, store) = indexed(&[
        ("models.py", "class Ledger:\n    pass\n"),
        ("z.py", "def ledger_action():\n    pass\n"),
        (
            "a.py",
            "from models import Ledger as A\nfrom models import Ledger as B\nfrom models import Ledger as C\nfrom models import Ledger as D\nfrom models import Ledger as E\nfrom models import Ledger as F\n",
        ),
    ]);
    let b = brief(&store, "ledger").unwrap();
    let named = b
        .symbols
        .iter()
        .position(|s| s.addr.qname == "z.ledger_action")
        .unwrap();
    let imported = b.symbols.iter().position(|s| s.addr.qname == "a").unwrap();
    assert!(named < imported, "{:?}", b.symbols);
}

/// GIVEN a file anchor containing a class, its method and the enclosing module
/// WHEN the brief suggests code to read
/// THEN it does not spend multiple slots on overlapping lines.
#[test]
fn test_given_enclosing_symbols_when_brief_then_read_spots_do_not_overlap() {
    let (_repo, store) = indexed(&[("models.py", MODEL)]);
    let b = brief(&store, "models.py").unwrap();
    for (n, spot) in b.ler.iter().enumerate() {
        assert!(!b.ler[..n].iter().any(|other| other.path == spot.path
            && other.start <= spot.end
            && spot.start <= other.end));
    }
}

/// GIVEN parameters shadowing imported constructor and SQLAlchemy names
/// WHEN those parameters are called using the same syntax as the imports
/// THEN no exact constructor-method relation or model write is invented.
#[test]
fn test_given_shadowed_imports_when_called_then_no_exact_class_or_sql_write() {
    let (_repo, store) = indexed(&[
        ("models.py", MODEL),
        (
            "svc.py",
            "from sqlalchemy import update\nfrom models import Record, Repository\n\ndef flow(Repository, update):\n    Repository().run()\n    update(Record).values(state='done')\n",
        ),
    ]);
    assert!(
        callers(&store, "models.Base.run", 1)
            .unwrap()
            .items
            .iter()
            .all(|i| i.approx)
    );
    assert!(
        writers(&store, "models.Record.state")
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        !symbol_info(&store, "svc.flow")
            .unwrap()
            .edges
            .iter()
            .any(|e| e.kind == "CRIA" && e.other == "Repository")
    );
}

/// GIVEN a task that has already seen a symbol and a body-only edit with unchanged lines
/// WHEN the same structural brief is requested after incremental indexing
/// THEN the edited symbol and its read spot are delivered again.
#[test]
fn test_given_body_edit_same_lines_when_task_brief_then_changed_symbol_is_returned() {
    use memo_core::brief::apply_delta;
    let (repo, mut store) = indexed(&[("m.py", "def flow():\n    return 1\n")]);
    let mut first = brief(&store, "m.flow").unwrap();
    apply_delta(&store, "task8", &mut first, false).unwrap();
    let mut repeated = brief(&store, "m.flow").unwrap();
    apply_delta(&store, "task8", &mut repeated, false).unwrap();
    assert!(repeated.symbols.is_empty());
    repo.write("m.py", "def flow():\n    return 2\n");
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let mut edited = brief(&store, "m.flow").unwrap();
    apply_delta(&store, "task8", &mut edited, false).unwrap();
    assert_eq!(edited.symbols.len(), 1);
    assert_eq!(edited.ler.len(), 1);
}
