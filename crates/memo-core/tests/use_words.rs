mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use rusqlite::Connection;
use std::collections::BTreeSet;

struct Indexed {
    _repo: Repo,
    conn: Connection,
}

fn indexed(files: &[(&str, &str)]) -> Indexed {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let db = repo.path().join(".git/use-words-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let conn = Connection::open(&db).unwrap();
    Indexed { _repo: repo, conn }
}

impl Indexed {
    /// Palavras de uso (`ref` e `type`) cujo dono é o símbolo `qname`.
    fn use_words(&self, qname: &str) -> BTreeSet<String> {
        self.conn
            .prepare(
                "SELECT w.text FROM occurrence o
                 JOIN word w ON w.id = o.word_id
                 JOIN symbol s ON o.owner_kind = 'symbol' AND o.owner_id = s.id
                 WHERE s.qname = ?1 AND o.source IN ('ref', 'type')",
            )
            .unwrap()
            .query_map([qname], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn count(&self, sql: &str) -> i64 {
        self.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }
}

const TWO_RUNNERS: &str = "class Alpha:\n    def run(self):\n        pass\n\n\
                           class Beta:\n    def run(self):\n        pass\n";

/// GIVEN 'def go(): Alpha(1).run()' where 'run' exists in two classes (call stays unresolved)
/// WHEN  the repository is indexed
/// THEN  m.go owns the word 'alpha' from its call text, with no edge needed
#[test]
fn test_given_chained_call_when_indexed_then_receiver_words_belong_to_caller() {
    let i = indexed(&[
        ("a.py", TWO_RUNNERS),
        (
            "m.py",
            "from a import Alpha, Beta\n\ndef go():\n    Alpha(1).run()\n",
        ),
    ]);
    let words = i.use_words("m.go");
    assert!(
        words.contains("alpha") && words.contains("run"),
        "{words:?}"
    );
}

/// GIVEN a function whose parameter and return are annotated with 'Optional[LockResult]'
/// WHEN  the repository is indexed
/// THEN  the function owns the words 'lock' and 'result' even though nothing calls it
#[test]
fn test_given_type_annotation_when_indexed_then_type_words_belong_to_symbol() {
    let i = indexed(&[(
        "m.py",
        "from typing import Optional\n\nclass LockResult:\n    pass\n\n\
         def read(value: Optional[LockResult]) -> Optional[LockResult]:\n    return value\n",
    )]);
    let words = i.use_words("m.read");
    assert!(
        words.contains("lock") && words.contains("result"),
        "{words:?}"
    );
}

/// GIVEN a call that the resolver leaves unresolved and one it resolves
/// WHEN  the repository is indexed
/// THEN  both callers own the words of their call text (independent of `resolved`)
#[test]
fn test_given_resolved_and_unresolved_calls_when_indexed_then_both_emit_words() {
    let i = indexed(&[
        ("a.py", TWO_RUNNERS),
        (
            "m.py",
            "from a import Alpha, Beta\n\ndef helper():\n    pass\n\n\
             def one():\n    helper()\n\ndef two(obj):\n    obj.run()\n",
        ),
    ]);
    assert!(i.count("SELECT COUNT(*) FROM raw_ref WHERE resolved = 0") > 0);
    assert!(i.use_words("m.one").contains("helper"));
    assert!(i.use_words("m.two").contains("run"));
}

/// GIVEN any indexed repository
/// WHEN  the use words are stored
/// THEN  they carry no `symbol_id`, so statistics and `brief` do not see them
#[test]
fn test_given_use_words_when_stored_then_symbol_id_is_null() {
    let i = indexed(&[(
        "m.py",
        "def helper():\n    pass\n\ndef one():\n    helper()\n",
    )]);
    assert!(i.count("SELECT COUNT(*) FROM occurrence WHERE source IN ('ref','type')") > 0);
    assert_eq!(
        i.count(
            "SELECT COUNT(*) FROM occurrence WHERE source IN ('ref','type') AND symbol_id IS NOT NULL"
        ),
        0
    );
}

/// GIVEN a file with a call that is then rewritten to call something else
/// WHEN  the repository is reindexed
/// THEN  the old use words are gone
#[test]
fn test_given_changed_file_when_reindexed_then_stale_use_words_removed() {
    let repo = Repo::empty();
    repo.write("m.py", "def one():\n    helper()\n");
    let db = repo.path().join(".git/use-words-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    repo.write("m.py", "def one():\n    other()\n");
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let conn = Connection::open(&db).unwrap();
    let words: BTreeSet<String> = conn
        .prepare(
            "SELECT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
             WHERE o.source = 'ref'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        words.contains("other") && !words.contains("helper"),
        "{words:?}"
    );
}

fn by_word_names(files: &[(&str, &str)], target: &str) -> (Vec<String>, Vec<String>) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = memo_core::query::impact(&store, target).unwrap();
    assert!(info.by_word.iter().all(|i| i.approx));
    (
        info.by_word.iter().map(|i| i.addr.qname.clone()).collect(),
        info.consumers
            .iter()
            .map(|i| i.addr.qname.clone())
            .collect(),
    )
}

/// GIVEN 'run' defined in two classes and called on an untyped parameter (no edge)
/// WHEN  impact runs on 'a.Alpha.run'
/// THEN  the caller appears in the by-word level, approximate; code without the word does not
#[test]
fn test_given_unresolved_call_when_impact_then_by_word_lists_caller() {
    let (found, _) = by_word_names(
        &[
            ("a.py", TWO_RUNNERS),
            (
                "m.py",
                "from a import Alpha, Beta\n\ndef go(obj):\n    obj.run()\n\n\
                 def typed(x: Alpha):\n    return x\n",
            ),
        ],
        "a.Alpha.run",
    );
    assert!(found.contains(&"m.go".to_string()), "{found:?}");
    assert!(!found.contains(&"m.typed".to_string()), "{found:?}");
}

/// GIVEN a symbol already listed among the consumers by an edge
/// WHEN  impact runs
/// THEN  it does not repeat in the by-word level
#[test]
fn test_given_edge_consumer_when_impact_then_by_word_does_not_repeat_it() {
    let (found, consumers) = by_word_names(
        &[(
            "m.py",
            "class Alpha:\n    pass\n\ndef typed(x: Alpha):\n    return x\n",
        )],
        "m.Alpha",
    );
    assert!(consumers.contains(&"m.typed".to_string()), "{consumers:?}");
    assert!(!found.contains(&"m.typed".to_string()), "{found:?}");
}
