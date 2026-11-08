mod common;

use common::Repo;
use memo_core::config::Config;
use memo_core::evidence::Evidence;
use memo_core::index::index;
use memo_core::query::Impact;
use memo_core::source::WorkTree;
use memo_core::sources::use_words;
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
    /// Palavras de uso do símbolo `qname`: as de `ref`, cujo dono é cada uso (`raw_ref`) que
    /// está no símbolo, e as de `type`, cujo dono é o próprio símbolo.
    fn use_words(&self, qname: &str) -> BTreeSet<String> {
        self.conn
            .prepare(
                "SELECT w.text FROM occurrence o
                 JOIN word w ON w.id = o.word_id
                 JOIN raw_ref r ON o.owner_kind = 'ref' AND o.owner_id = r.id
                 JOIN symbol s ON s.id = r.src
                 WHERE s.qname = ?1 AND o.source = 'ref'
                 UNION
                 SELECT w.text FROM occurrence o
                 JOIN word w ON w.id = o.word_id
                 JOIN symbol s ON o.owner_kind = 'symbol' AND o.owner_id = s.id
                 WHERE s.qname = ?1 AND o.source = 'type'",
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
/// THEN  a use inside m.go owns the word 'alpha' from its call text, with no edge needed
#[test]
fn test_given_chained_call_when_indexed_then_receiver_words_belong_to_caller() {
    let i = indexed(&[
        ("a.py", TWO_RUNNERS),
        (
            "m.py",
            "from a import Alpha, Beta\n\ndef go():\n    Alpha(1).run()\n",
        ),
    ]);
    assert!(i.use_words("m.go").contains("alpha"));
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
/// THEN  the uses inside both callers own the words of their call text (independent of `resolved`)
#[test]
fn test_given_resolved_and_unresolved_calls_when_indexed_then_both_emit_words() {
    let i = indexed(&[
        ("a.py", TWO_RUNNERS),
        (
            "m.py",
            "from a import Alpha, Beta\n\ndef helper():\n    pass\n\n\
             def one():\n    helper()\n\ndef two():\n    Alpha(1).run()\n",
        ),
    ]);
    assert!(i.count("SELECT COUNT(*) FROM raw_ref WHERE resolved = 0") > 0);
    assert!(i.use_words("m.one").contains("helper"));
    assert!(i.use_words("m.two").contains("alpha"));
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

/// Fixture U (TASK-6, §7): uma família de `run` e usos com e sem evidência da classe dona.
const FIXTURE_U_A: &str = "from typing import Protocol\n\n\
                           class Runner(Protocol):\n    def run(self):\n        ...\n\n\
                           class Alpha:\n    def run(self):\n        pass\n\n\
                           class SubAlpha(Alpha):\n    pass\n\n\
                           class Beta:\n    def run(self):\n        pass\n";
const FIXTURE_U_M: &str = "from a import Alpha, SubAlpha, Beta, Runner\n\n\
                           def strong_owner():\n    Alpha(1).run()\n\n\
                           def strong_sub():\n    SubAlpha(1).run()\n\n\
                           def medium_owner(items: list[Alpha]):\n    for it in items:\n        it.run()\n\n\
                           def medium_protocol(items: list[Runner]):\n    for it in items:\n        it.run()\n\n\
                           def only_beta():\n    Beta(1).run()\n\n\
                           def only_obj(obj):\n    obj.run()\n";
/// Fixture R (TASK-6, §7): chamadas resolvidas (1), ambíguas (0) e externas (2).
const FIXTURE_R: &str = "class Gamma:\n    def go(self):\n        pass\n\n\
                         class Delta:\n    def go(self):\n        pass\n\n\
                         def helper():\n    pass\n\n\
                         def one():\n    helper()\n\n\
                         def two():\n    Gamma(1).go()\n\n\
                         def three():\n    external_thing()\n";

/// GIVEN fixtures U and R, with resolved 0, 1 and 2 all present among the references
/// WHEN  the repository is indexed
/// THEN  every raw_ref owns, as 'ref' occurrences, exactly the words of its callee text and receiver
#[test]
fn test_given_varied_refs_when_indexed_then_each_ref_owns_exactly_its_text_words() {
    let i = indexed(&[
        ("a.py", FIXTURE_U_A),
        ("m.py", FIXTURE_U_M),
        ("r.py", FIXTURE_R),
    ]);
    let states: BTreeSet<i64> = i
        .conn
        .prepare("SELECT DISTINCT resolved FROM raw_ref")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(states, BTreeSet::from([0, 1, 2]));
    let refs: Vec<(i64, Option<String>, Option<String>)> = i
        .conn
        .prepare("SELECT id, callee_text, receiver FROM raw_ref ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!refs.is_empty());
    let config = Config::default();
    for (id, text, receiver) in refs {
        let expected: BTreeSet<String> = [text, receiver]
            .into_iter()
            .flatten()
            .flat_map(|t| use_words(&t, &config))
            .map(|w| w.text)
            .collect();
        let owned: BTreeSet<String> = i
            .conn
            .prepare(
                "SELECT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
                 WHERE o.owner_kind = 'ref' AND o.owner_id = ?1 AND o.source = 'ref'",
            )
            .unwrap()
            .query_map([id], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(owned, expected, "raw_ref {id}");
    }
}

fn by_word_names(files: &[(&str, &str)], target: &str) -> (Vec<String>, Vec<String>) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = memo_core::query::impact(&store, target).unwrap();
    assert!(info.by_word.iter().all(|i| i.item.approx));
    (
        info.by_word
            .iter()
            .map(|i| i.item.addr.qname.clone())
            .collect(),
        info.consumers
            .iter()
            .map(|i| i.addr.qname.clone())
            .collect(),
    )
}

/// GIVEN 'run' defined in two classes and called through a chained instantiation (no edge)
/// WHEN  impact runs on 'a.Alpha.run'
/// THEN  the caller appears in the by-word level as strong, approximate; code without the word does not
#[test]
fn test_given_unresolved_call_when_impact_then_by_word_lists_caller() {
    let repo = Repo::empty();
    repo.write("a.py", TWO_RUNNERS);
    repo.write(
        "m.py",
        "from a import Alpha, Beta\n\ndef go():\n    Alpha(1).run()\n",
    );
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = memo_core::query::impact(&store, "a.Alpha.run").unwrap();
    assert!(
        info.by_word
            .iter()
            .any(|i| i.item.addr.qname == "m.go" && i.nivel == Evidence::Forte),
        "{:?}",
        info.by_word
    );
    let (found, _) = by_word_names(
        &[
            ("a.py", TWO_RUNNERS),
            (
                "m.py",
                "from a import Alpha, Beta\n\ndef go():\n    Alpha(1).run()\n\n\
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

fn words_of(conn: &Connection, sql: &str, id: i64) -> BTreeSet<String> {
    conn.prepare(sql)
        .unwrap()
        .query_map([id], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// GIVEN fixture U plus a class with annotated attributes (type_text is only set on attributes)
/// WHEN  the repository is indexed
/// THEN  every symbol's 'type' occurrences are exactly the words of its type_text, and none without it
#[test]
fn test_given_typed_symbols_when_indexed_then_type_words_match_type_text() {
    let holder = "from a import Alpha\n\nclass Holder:\n    runner: Alpha\n\n\
                  def __init__(self):\n        self.backup: Optional[Alpha] = None\n";
    let i = indexed(&[
        ("a.py", FIXTURE_U_A),
        ("m.py", FIXTURE_U_M),
        ("h.py", holder),
    ]);
    let symbols: Vec<(i64, Option<String>)> = i
        .conn
        .prepare("SELECT id, type_text FROM symbol ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(symbols.iter().any(|(_, t)| t.is_some()));
    let config = Config::default();
    for (id, type_text) in symbols {
        let expected: BTreeSet<String> = type_text
            .iter()
            .flat_map(|t| use_words(t, &config))
            .map(|w| w.text)
            .collect();
        let owned = words_of(
            &i.conn,
            "SELECT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
             WHERE o.owner_kind = 'symbol' AND o.owner_id = ?1 AND o.source = 'type'",
            id,
        );
        assert_eq!(owned, expected, "symbol {id}");
    }
}

/// GIVEN 'one()' in m.py calling 'helper()', resolved to the single definition in h.py
/// WHEN  h.py drops 'helper' and is reindexed, with the caller file untouched
/// THEN  the call's resolved state changes and the words of that use are identical
#[test]
fn test_given_call_made_ambiguous_elsewhere_when_reindexed_then_use_words_unchanged() {
    let repo = Repo::empty();
    repo.write("h.py", "def helper():\n    pass\n");
    repo.write("m.py", "def one():\n    helper()\n");
    let db = repo.path().join(".git/use-words-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    let state = |conn: &Connection| -> (i64, i64) {
        conn.query_row(
            "SELECT r.id, r.resolved FROM raw_ref r JOIN symbol s ON s.id = r.src
             WHERE s.qname = 'm.one' AND r.callee_text = 'helper'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    let ref_words = "SELECT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
                     WHERE o.owner_kind = 'ref' AND o.owner_id = ?1 AND o.source = 'ref'";
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let conn = Connection::open(&db).unwrap();
    let (id, before) = state(&conn);
    let words_before = words_of(&conn, ref_words, id);
    repo.write("h.py", "def other():\n    pass\n");
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let (id_after, after) = state(&conn);
    assert_ne!(before, after, "resolved should change");
    assert_eq!(id, id_after);
    assert_eq!(words_of(&conn, ref_words, id), words_before);
}

/// Ids das referências de `path` e rowids das ocorrências delas, em ordem.
fn use_rows(conn: &Connection, path: &str) -> Vec<(i64, i64)> {
    conn.prepare(
        "SELECT r.id, o.rowid FROM raw_ref r JOIN file f ON f.id = r.file_id
         JOIN occurrence o ON o.owner_kind = 'ref' AND o.owner_id = r.id
         WHERE f.path = ?1 ORDER BY r.id, o.rowid",
    )
    .unwrap()
    .query_map([path], |r| Ok((r.get(0)?, r.get(1)?)))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// GIVEN fixture U indexed
/// WHEN  only a.py changes and the repository is reindexed
/// THEN  the raw_ref ids of m.py and the rows of their occurrences are the same as before
#[test]
fn test_given_one_file_changed_when_reindexed_then_other_files_use_rows_untouched() {
    let repo = Repo::empty();
    repo.write("a.py", FIXTURE_U_A);
    repo.write("m.py", FIXTURE_U_M);
    let db = repo.path().join(".git/use-words-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let conn = Connection::open(&db).unwrap();
    let before = use_rows(&conn, "m.py");
    assert!(!before.is_empty());
    repo.write("a.py", &format!("{FIXTURE_U_A}\ndef extra():\n    pass\n"));
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    assert_eq!(use_rows(&conn, "m.py"), before);
}

/// (palavra, fonte, qname do símbolo, linha do uso) de todas as palavras de uso, com repetição.
fn use_multiset(conn: &Connection) -> Vec<(String, String, String, Option<i64>)> {
    conn.prepare(
        "SELECT w.text, o.source, s.qname, r.line FROM occurrence o
         JOIN word w ON w.id = o.word_id
         JOIN raw_ref r ON o.owner_kind = 'ref' AND o.owner_id = r.id
         JOIN symbol s ON s.id = r.src
         WHERE o.source = 'ref'
         UNION ALL
         SELECT w.text, o.source, s.qname, NULL FROM occurrence o
         JOIN word w ON w.id = o.word_id
         JOIN symbol s ON o.owner_kind = 'symbol' AND o.owner_id = s.id
         WHERE o.source = 'type'
         ORDER BY 1, 2, 3, 4",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// GIVEN fixture U indexed, then a.py changed and reindexed
/// WHEN  compared with a fresh index of the final tree
/// THEN  the multiset of (word, source, owner symbol, line) of the use words is equal
#[test]
fn test_given_incremental_reindex_when_compared_then_equals_fresh_index() {
    let changed = format!("{FIXTURE_U_A}\ndef extra(x: Beta):\n    x.run()\n");
    let repo = Repo::empty();
    repo.write("a.py", FIXTURE_U_A);
    repo.write("m.py", FIXTURE_U_M);
    let db = repo.path().join(".git/use-words-test.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    repo.write("a.py", &changed);
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let incremental = use_multiset(&Connection::open(&db).unwrap());
    let fresh = indexed(&[("a.py", &changed), ("m.py", FIXTURE_U_M)]);
    assert_eq!(incremental, use_multiset(&fresh.conn));
}

fn impact_of(files: &[(&str, &str)], target: &str) -> Impact {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    memo_core::query::impact(&store, target).unwrap()
}

/// Símbolos do nível por palavra com o nível pedido (`so_nome` vem da lista à parte).
fn level(info: &Impact, nivel: Evidence) -> BTreeSet<String> {
    info.by_word
        .iter()
        .chain(&info.by_word_name_only)
        .filter(|i| i.nivel == nivel)
        .map(|i| i.item.addr.qname.clone())
        .collect()
}

/// Símbolos que as arestas do `impact` já cobrem.
fn covered(info: &Impact) -> BTreeSet<String> {
    info.callers
        .iter()
        .filter_map(|c| c.addr.as_ref().map(|a| a.qname.clone()))
        .chain(info.consumers.iter().map(|c| c.addr.qname.clone()))
        .chain(info.protocol.iter().map(|c| c.addr.qname.clone()))
        .collect()
}

fn minus(names: &[&str], drop: &BTreeSet<String>) -> BTreeSet<String> {
    names
        .iter()
        .map(|n| n.to_string())
        .filter(|n| !drop.contains(n))
        .collect()
}

/// GIVEN fixture U
/// WHEN  impact runs on a.Alpha.run
/// THEN  uses split into strong, medium and name only, minus whatever edges already cover
#[test]
fn test_given_family_uses_when_impact_then_strong_medium_and_name_only_split() {
    let info = impact_of(
        &[("a.py", FIXTURE_U_A), ("m.py", FIXTURE_U_M)],
        "a.Alpha.run",
    );
    let edges = covered(&info);
    assert_eq!(
        level(&info, Evidence::Forte),
        minus(&["m.strong_owner", "m.strong_sub"], &edges)
    );
    assert_eq!(
        level(&info, Evidence::Media),
        minus(&["m.medium_owner", "m.medium_protocol"], &edges)
    );
    assert_eq!(
        level(&info, Evidence::SoNome),
        minus(&["m.only_beta", "m.only_obj"], &edges)
    );
    assert!(info.by_word.iter().all(|i| i.nivel != Evidence::SoNome));
    assert!(
        info.by_word_name_only
            .iter()
            .all(|i| i.nivel == Evidence::SoNome)
    );
}

/// Fixture F (TASK-6, §7): uma função e usos no próprio módulo, com import e sem import.
const FIXTURE_F: &[(&str, &str)] = &[
    (
        "m.py",
        "def helper():\n    pass\n\ndef local_use():\n    helper()\n",
    ),
    (
        "n.py",
        "from m import helper\n\ndef imported_use():\n    helper()\n",
    ),
    ("k.py", "import m\n\ndef module_use():\n    m.helper()\n"),
    ("z.py", "def helper():\n    pass\n"),
    ("y.py", "def helper():\n    pass\n"),
    ("w.py", "def other_use():\n    helper()\n"),
];

/// GIVEN fixture F
/// WHEN  impact runs on m.helper
/// THEN  other_use is counted as name only and never listed as strong or medium
#[test]
fn test_given_homonym_call_when_impact_then_name_only_counted() {
    let info = impact_of(FIXTURE_F, "m.helper");
    assert!(
        level(&info, Evidence::SoNome).contains("w.other_use"),
        "{:?}",
        info.by_word_name_only
    );
    assert!(
        info.by_word
            .iter()
            .all(|i| i.item.addr.qname != "w.other_use")
    );
}

/// GIVEN fixture U
/// WHEN  impact runs on a.Alpha.run
/// THEN  each strong and medium item carries the line of a use of 'run' inside its symbol
#[test]
fn test_given_by_word_item_when_impact_then_line_is_use_line() {
    let info = impact_of(
        &[("a.py", FIXTURE_U_A), ("m.py", FIXTURE_U_M)],
        "a.Alpha.run",
    );
    assert!(!info.by_word.is_empty());
    let expected = |qname: &str| -> i64 {
        let body = FIXTURE_U_M
            .lines()
            .position(|l| l.starts_with(&format!("def {}(", &qname[2..])))
            .unwrap();
        let offset = FIXTURE_U_M
            .lines()
            .skip(body)
            .position(|l| l.contains(".run()"))
            .unwrap();
        (body + offset + 1) as i64
    };
    for i in &info.by_word {
        assert_eq!(i.item.line, Some(expected(&i.item.addr.qname)), "{i:?}");
    }
}

/// GIVEN fixture U indexed fresh twice into two stores
/// WHEN  impact on a.Alpha.run is serialized to JSON
/// THEN  both outputs are byte-identical
#[test]
fn test_given_same_tree_twice_when_impact_then_identical_bytes() {
    let files = [("a.py", FIXTURE_U_A), ("m.py", FIXTURE_U_M)];
    let first = serde_json::to_string(&impact_of(&files, "a.Alpha.run")).unwrap();
    let second = serde_json::to_string(&impact_of(&files, "a.Alpha.run")).unwrap();
    assert_eq!(first, second);
}
