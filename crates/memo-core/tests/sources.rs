mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::index::index_with;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use rusqlite::Connection;
use std::collections::BTreeSet;

struct Indexed {
    _repo: Repo,
    conn: Connection,
}

fn indexed(files: &[(&str, &str)], config: &str) -> Indexed {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let db = repo.path().join(".git/sources-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    let config = Config::from_toml(config).unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, &config).unwrap();
    let conn = Connection::open(&db).unwrap();
    Indexed { _repo: repo, conn }
}

impl Indexed {
    /// `(palavra, slot, segmento, dono)` das ocorrências de uma origem.
    fn words(&self, source: &str) -> BTreeSet<(String, String, String, String)> {
        self.conn
            .prepare(
                "SELECT w.text, o.slot, COALESCE(o.segment, ''), o.owner_kind
                 FROM occurrence o JOIN word w ON w.id = o.word_id WHERE o.source = ?1",
            )
            .unwrap()
            .query_map([source], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn texts(&self, source: &str) -> BTreeSet<String> {
        self.words(source).into_iter().map(|w| w.0).collect()
    }
}

fn row(word: &str, slot: &str, segment: &str, owner: &str) -> (String, String, String, String) {
    (word.into(), slot.into(), segment.into(), owner.into())
}

const IMPORT: &str =
    "from app.orders.domain.value_objects.order_status import OrderStatus\n\ndef f():\n    pass\n";

/// GIVEN the file 'app/billing/application/usecases/calc.py' with the default context_root 'app'
/// WHEN  the repository is indexed
/// THEN  path words are 'billing' (slot context) and 'usecases' (slot object), owned by the file
#[test]
fn test_given_directory_segments_when_indexed_then_words_have_path_source_and_file_owner() {
    let ix = indexed(
        &[(
            "app/billing/application/usecases/calc.py",
            "def f():\n    pass\n",
        )],
        "",
    );
    let expected: BTreeSet<_> = [
        row("billing", "context", "", "file"),
        row("usecases", "object", "", "file"),
    ]
    .into();
    assert_eq!(ix.words("path"), expected);
}

/// GIVEN 'from app.orders.domain.value_objects.order_status import OrderStatus'
/// WHEN  the repository is indexed
/// THEN  import words are orders, value, objects, order and status, owned by the import
#[test]
fn test_given_import_statement_when_indexed_then_words_from_name_and_path_with_import_source() {
    let ix = indexed(&[("app/x/m.py", IMPORT)], "");
    let expected: BTreeSet<_> = [
        row("orders", "context", "", "import"),
        row("value", "object", "", "import"),
        row("objects", "object", "", "import"),
        row("order", "object", "", "import"),
        row("status", "object", "", "import"),
    ]
    .into();
    assert_eq!(ix.words("import"), expected);
}

/// GIVEN the same import and layers 'domain', 'application', 'infra'
/// WHEN  the repository is indexed
/// THEN  the package root 'app' and the layer 'domain' are not import words
#[test]
fn test_given_import_path_when_split_then_root_and_layer_segments_dropped() {
    let ix = indexed(&[("app/x/m.py", IMPORT)], "");
    let words = ix.texts("import");
    assert!(
        !words.contains("app") && !words.contains("domain"),
        "{words:?}"
    );
}

/// GIVEN the paths 'app/billing/...' and the import 'app.orders...'
/// WHEN  the repository is indexed
/// THEN  the context segment has slot 'context' in both the path and the import words
#[test]
fn test_given_context_segment_when_indexed_then_word_has_slot_context() {
    let ix = indexed(&[("app/billing/domain/m.py", IMPORT)], "");
    assert!(
        ix.words("path")
            .contains(&row("billing", "context", "", "file"))
    );
    assert!(
        ix.words("import")
            .contains(&row("orders", "context", "", "import"))
    );
}

/// GIVEN context_root 'backend/easy_lease', the file 'backend/easy_lease/orders/domain/m.py'
///       and the import 'easy_lease.orders.domain.order import Order'
/// WHEN  the repository is indexed
/// THEN  the file context is 'orders', and path and import words both give 'orders' as context
#[test]
fn test_given_context_root_path_prefix_when_indexed_then_context_is_next_segment() {
    let source = "from easy_lease.orders.domain.order import Order\n\ndef f():\n    pass\n";
    let ix = indexed(
        &[("backend/easy_lease/orders/domain/m.py", source)],
        "context_root = \"backend/easy_lease\"\n",
    );
    let file_context: String = ix
        .conn
        .query_row("SELECT context FROM file", [], |r| r.get(0))
        .unwrap();
    assert_eq!(file_context, "orders");
    let expected_path: BTreeSet<_> = [row("orders", "context", "", "file")].into();
    assert_eq!(ix.words("path"), expected_path);
    assert!(
        ix.words("import")
            .contains(&row("orders", "context", "", "import"))
    );
    assert!(!ix.texts("import").contains("easy_lease"));
}

const DOC_NAMES: &str = "def order():\n    pass\n\ndef placed():\n    pass\n\ndef cancel():\n    pass\n\ndef canceled():\n    pass\n";

/// GIVEN a test with the docstring 'GIVEN order placed WHEN cancel THEN canceled'
///       and identifiers order, placed, cancel and canceled elsewhere
/// WHEN  the repository is indexed
/// THEN  test_doc words carry the given, when and then segments and are owned by the test symbol
#[test]
fn test_given_test_docstring_when_indexed_then_words_have_test_doc_source_and_segments() {
    let test = "def test_a():\n    \"\"\"GIVEN order placed WHEN cancel THEN canceled\"\"\"\n";
    let ix = indexed(
        &[
            ("app/o/names.py", DOC_NAMES),
            ("app/o/tests/test_a.py", test),
        ],
        "",
    );
    let expected: BTreeSet<_> = [
        row("order", "object", "given", "symbol"),
        row("placed", "object", "given", "symbol"),
        row("cancel", "object", "when", "symbol"),
        row("canceled", "object", "then", "symbol"),
    ]
    .into();
    assert_eq!(ix.words("test_doc"), expected);
}

/// GIVEN a test docstring 'GIVEN an order WHEN the user cancels THEN nothing'
///       and the only matching identifier word is 'order'
/// WHEN  the repository is indexed
/// THEN  only 'order' is stored; 'an', 'the', 'user', 'cancels' and 'nothing' are not
#[test]
fn test_given_prose_word_absent_from_identifiers_when_indexed_then_not_stored() {
    let test = "def test_a():\n    \"\"\"GIVEN an order WHEN the user cancels THEN nothing\"\"\"\n";
    let ix = indexed(
        &[
            ("app/o/names.py", DOC_NAMES),
            ("app/o/tests/test_a.py", test),
        ],
        "",
    );
    let words: BTreeSet<String> = ["order".to_string()].into();
    assert_eq!(ix.texts("test_doc"), words);
}

/// GIVEN an identifier 'then_value' (so 'then' and 'value' are identifier words)
///       and a test docstring 'GIVEN value WHEN value THEN value'
/// WHEN  the repository is indexed
/// THEN  test_doc has 'value' in the given, when and then segments and no row for given, when or then
#[test]
fn test_given_given_when_then_markers_when_docstring_split_then_markers_not_words() {
    let names = "def then_value():\n    pass\n";
    let test = "def test_a():\n    \"\"\"GIVEN value WHEN value THEN value\"\"\"\n";
    let ix = indexed(
        &[("app/o/names.py", names), ("app/o/tests/test_a.py", test)],
        "",
    );
    let expected: BTreeSet<_> = [
        row("value", "object", "given", "symbol"),
        row("value", "object", "when", "symbol"),
        row("value", "object", "then", "symbol"),
    ]
    .into();
    assert_eq!(ix.words("test_doc"), expected);
}
