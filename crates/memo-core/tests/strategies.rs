mod common;

use common::Repo;
use memo_core::config::{Config, StrategyWeights};
use memo_core::error::CoreError;
use memo_core::index::{index, index_with};
use memo_core::query::tests_for;
use memo_core::relevance::{Relevant, name_stats, relevant, resolve_targets};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use rusqlite::Connection;

fn indexed_with(files: &[(&str, &str)], config: &Config) -> (Repo, SqliteStore, Connection) {
    let repo = Repo::empty();
    for (path, text) in files {
        repo.write(path, text);
    }
    let db = repo.path().join(".git/strategies.db");
    let mut store = SqliteStore::open(&db).unwrap();
    index_with(&WorkTree::new(repo.path()), &mut store, config).unwrap();
    let conn = Connection::open(&db).unwrap();
    (repo, store, conn)
}

fn indexed(files: &[(&str, &str)]) -> (Repo, SqliteStore, Connection) {
    indexed_with(files, &Config::default())
}

fn id_of(conn: &Connection, qname: &str) -> i64 {
    conn.query_row("SELECT id FROM symbol WHERE qname = ?1", [qname], |r| {
        r.get(0)
    })
    .unwrap()
}

fn route_of(conn: &Connection, qname: &str) -> (Option<String>, bool) {
    conn.query_row(
        "SELECT route, is_entrypoint FROM symbol WHERE qname = ?1",
        [qname],
        |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
    )
    .unwrap()
}

fn find<'a>(items: &'a [Relevant], qname: &str) -> Option<&'a Relevant> {
    items.iter().find(|r| r.addr.qname == qname)
}

fn weight(item: &Relevant, strategy: &str) -> f64 {
    item.strategies
        .iter()
        .filter(|s| s.strategy == strategy)
        .map(|s| s.weight)
        .sum()
}

const API: &str = "from fastapi import APIRouter\nfrom svc import handle\n\nrouter = APIRouter()\n\n\
                   @router.get('/items/{item_id}')\ndef read_item(item_id):\n    return handle(item_id)\n\n\
                   @router.get(dynamic_path)\ndef dynamic():\n    pass\n\n\
                   def plain():\n    pass\n";
const SVC: &str = "from repo import Repo\n\ndef handle(item_id):\n    return Repo().run(item_id)\n";
const REPO: &str = "class Repo:\n    def run(self, item_id):\n        return item_id\n";

/// GIVEN an HTTP decorator with a literal path, one with a dynamic path and an undecorated function
/// WHEN the repository is indexed
/// THEN only the literal route marks an entrypoint, with its parameters normalized to `{}`.
#[test]
fn test_given_http_decorators_when_indexed_then_literal_route_is_entrypoint() {
    let (_r, _s, conn) = indexed(&[("api.py", API), ("svc.py", SVC), ("repo.py", REPO)]);
    assert_eq!(
        route_of(&conn, "api.read_item"),
        (Some("GET /items/{}".to_string()), true)
    );
    assert_eq!(route_of(&conn, "api.dynamic"), (None, false));
    assert_eq!(route_of(&conn, "api.plain"), (None, false));
}

/// GIVEN `entrypoint_decorators = ["shared_task"]` and a function decorated with `@app.shared_task`
/// WHEN the repository is indexed
/// THEN the function is an entrypoint without a route; another decorator is not.
#[test]
fn test_given_configured_decorator_when_indexed_then_entrypoint_without_route() {
    let config = Config::from_toml("entrypoint_decorators = [\"shared_task\"]\n").unwrap();
    let (_r, _s, conn) = indexed_with(
        &[(
            "jobs.py",
            "@app.shared_task\ndef job():\n    pass\n\n@cache\ndef cached():\n    pass\n",
        )],
        &config,
    );
    assert_eq!(route_of(&conn, "jobs.job"), (None, true));
    assert_eq!(route_of(&conn, "jobs.cached"), (None, false));
}

/// GIVEN a test that requests the route with an f-string path, one with the wrong method, and the
///       endpoint reaching the repository method by exact calls
/// WHEN tests_for runs on the repository method
/// THEN only the matching request is listed under via_http, naming the path and the endpoint.
#[test]
fn test_given_http_test_when_tests_for_deep_symbol_then_listed_via_http() {
    let test = "def test_read(client, item):\n    client.get(f'/items/{item.id}?full=1')\n\n\
                def test_wrong_method(client):\n    client.post('/items/1')\n";
    let (_r, store, _c) = indexed(&[
        ("api.py", API),
        ("svc.py", SVC),
        ("repo.py", REPO),
        ("tests/test_api.py", test),
    ]);
    let info = tests_for(&store, "repo.Repo.run").unwrap();
    let names: Vec<&str> = info
        .via_http
        .iter()
        .map(|t| t.addr.short.as_str())
        .collect();
    assert_eq!(names, ["test_read"]);
    assert_eq!(
        info.via_http[0].attr.as_deref(),
        Some("/items/{} -> read_item")
    );
    assert!(!info.via_http[0].approx);
}

/// GIVEN two endpoints whose routes both match a requested path
/// WHEN tests_for runs on each endpoint
/// THEN the request is listed under both, marked approximate.
#[test]
fn test_given_request_matching_two_routes_when_tests_for_then_approximate() {
    let api = "@router.get('/items/{a}')\ndef one(a):\n    pass\n\n\
               @router.get('/items/latest')\ndef two():\n    pass\n";
    let (_r, store, _c) = indexed(&[
        ("api.py", api),
        (
            "tests/test_api.py",
            "def test_latest(client):\n    client.get('/items/latest')\n",
        ),
    ]);
    for target in ["api.one", "api.two"] {
        let info = tests_for(&store, target).unwrap();
        assert_eq!(info.via_http.len(), 1, "{target}");
        assert!(info.via_http[0].approx, "{target}");
    }
}

/// GIVEN a test requesting a route that does not exist yet
/// WHEN the route is added in another file and only that file is reindexed
/// THEN the request now reaches the endpoint (requests are re-resolved globally).
#[test]
fn test_given_route_added_later_when_reindexed_then_request_links() {
    let (repo, mut store, _c) = indexed(&[
        ("api.py", "def placeholder():\n    pass\n"),
        (
            "tests/test_api.py",
            "def test_ping(client):\n    client.get('/ping')\n",
        ),
    ]);
    assert!(resolve_targets(&store, "api.ping").is_err());
    repo.write("api.py", "@router.get('/ping')\ndef ping():\n    pass\n");
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    let info = tests_for(&store, "api.ping").unwrap();
    assert_eq!(info.via_http.len(), 1);
}

/// GIVEN `run` defined in four classes, `apply` defined once but called 12 times on unknown
///       receivers, and `compute_total` defined once and called once
/// WHEN name_stats runs
/// THEN `run` is generic by definitions, `apply` by unresolved uses, `compute_total` is not.
#[test]
fn test_given_name_distributions_when_name_stats_then_generic_flags() {
    let classes: String = (0..4)
        .map(|i| format!("class C{i}:\n    def run(self):\n        pass\n\n"))
        .collect();
    let uses: String = (0..12)
        .map(|i| format!("def u{i}(x):\n    x.apply()\n\n"))
        .collect();
    let (_r, store, _c) = indexed(&[
        ("c.py", classes.as_str()),
        ("a.py", "def apply():\n    pass\n"),
        ("u.py", uses.as_str()),
        (
            "t.py",
            "def compute_total():\n    pass\n\ndef caller():\n    compute_total()\n",
        ),
    ]);
    let run = name_stats(&store, "run").unwrap();
    assert_eq!(run.definitions, 4);
    assert!(run.generic);
    let apply = name_stats(&store, "apply").unwrap();
    assert_eq!((apply.uses, apply.resolved), (12, 0));
    assert!(apply.generic);
    let total = name_stats(&store, "compute_total").unwrap();
    assert_eq!((total.definitions, total.uses, total.resolved), (1, 1, 1));
    assert!(!total.generic);
}

/// GIVEN endpoint → service → repository by exact calls, the service also calling a logging hub
///       that 12 other functions call
/// WHEN relevant runs on the repository method
/// THEN the service (d1) and endpoint (d2) carry neighborhood and path weights with the route;
///      the hub and its other callers (siblings) are not listed.
#[test]
fn test_given_entrypoint_chain_when_relevant_then_neighbors_and_path_without_siblings() {
    let svc = "from repo import Repo\nfrom log import emit\n\n\
               def handle(item_id):\n    emit()\n    return Repo().run(item_id)\n";
    let others: String = (0..12)
        .map(|i| format!("def other{i}():\n    emit()\n\n"))
        .collect();
    let others = format!("from log import emit\n\n{others}");
    let (_r, store, conn) = indexed(&[
        ("api.py", API),
        ("svc.py", svc),
        ("repo.py", REPO),
        ("log.py", "def emit():\n    pass\n"),
        ("others.py", others.as_str()),
    ]);
    let items = relevant(
        &store,
        id_of(&conn, "repo.Repo.run"),
        &StrategyWeights::default(),
    )
    .unwrap();
    let service = find(&items, "svc.handle").expect("service");
    assert_eq!((service.depth, service.direction), (Some(1), "chamador"));
    assert_eq!(weight(service, "vizinhanca"), 1.0);
    assert_eq!(weight(service, "caminho"), 1.0);
    let endpoint = find(&items, "api.read_item").expect("endpoint");
    assert_eq!(endpoint.depth, Some(2));
    assert_eq!(weight(endpoint, "vizinhanca"), 0.5);
    assert_eq!(weight(endpoint, "caminho"), 1.0);
    assert_eq!(endpoint.route.as_deref(), Some("GET /items/{}"));
    assert!(find(&items, "log.emit").is_none());
    assert!(!items.iter().any(|r| r.addr.qname.starts_with("others.")));
    assert_eq!(items[0].addr.qname, "svc.handle");
}

/// GIVEN a target calling a helper used only by it and a hub called by 20 other functions
/// WHEN relevant runs on the target
/// THEN both are d1 neighbors, but the helper's connectivity weight is higher and it ranks first.
#[test]
fn test_given_helper_and_hub_when_relevant_then_connectivity_favors_helper() {
    let callers: String = (0..20)
        .map(|i| format!("def c{i}():\n    hub()\n\n"))
        .collect();
    let source = format!(
        "def hub():\n    pass\n\ndef helper():\n    pass\n\ndef target():\n    hub()\n    helper()\n\n{callers}"
    );
    let (_r, store, conn) = indexed(&[("m.py", source.as_str())]);
    let items = relevant(
        &store,
        id_of(&conn, "m.target"),
        &StrategyWeights::default(),
    )
    .unwrap();
    let helper = find(&items, "m.helper").unwrap();
    let hub = find(&items, "m.hub").unwrap();
    assert!(
        weight(helper, "conexao") > 4.0 * weight(hub, "conexao"),
        "{items:?}"
    );
    assert_eq!(items[0].addr.qname, "m.helper");
}

/// GIVEN a specific name called on an unknown receiver and a generic name called the same way
/// WHEN relevant runs on each definition
/// THEN only the specific name brings the caller through the `nome` strategy.
#[test]
fn test_given_unresolved_use_when_relevant_then_name_strategy_only_for_specific_names() {
    let classes: String = (0..4)
        .map(|i| format!("class C{i}:\n    def run(self):\n        pass\n\n"))
        .collect();
    let (_r, store, conn) = indexed(&[
        ("c.py", classes.as_str()),
        (
            "k.py",
            "class K:\n    def compute_total(self):\n        pass\n",
        ),
        (
            "u.py",
            "def use_total(obj):\n    obj.compute_total()\n\ndef use_run(obj):\n    obj.run()\n",
        ),
    ]);
    let weights = StrategyWeights::default();
    let total = relevant(&store, id_of(&conn, "k.K.compute_total"), &weights).unwrap();
    let user = find(&total, "u.use_total").expect("name strategy");
    assert_eq!(weight(user, "nome"), 0.25);
    assert_eq!(user.direction, "nome");
    let run = relevant(&store, id_of(&conn, "c.C0.run"), &weights).unwrap();
    assert!(find(&run, "u.use_run").is_none(), "{run:?}");
}

/// GIVEN `[strategies]` with path weight 0
/// WHEN relevant runs on the repository method
/// THEN no item carries a `caminho` weight and the endpoint, two hops up, scores 0.5 plus connectivity.
#[test]
fn test_given_zero_path_weight_when_relevant_then_no_path_strategy() {
    let (_r, store, conn) = indexed(&[("api.py", API), ("svc.py", SVC), ("repo.py", REPO)]);
    let weights = StrategyWeights {
        path: 0.0,
        ..StrategyWeights::default()
    };
    let items = relevant(&store, id_of(&conn, "repo.Repo.run"), &weights).unwrap();
    assert!(
        items.iter().all(|r| weight(r, "caminho") == 0.0),
        "{items:?}"
    );
}

/// GIVEN a resource and a service with the same name, the resource calling the service exactly,
///       and two unrelated homonyms elsewhere
/// WHEN resolve_targets receives each bare name
/// THEN the linked pair is a group ordered caller first; the unrelated pair stays ambiguous.
#[test]
fn test_given_homonyms_when_resolve_targets_then_linked_group_or_ambiguous() {
    let (_r, store, _c) = indexed(&[
        (
            "res2.py",
            "import svc\n\ndef list_things():\n    return svc.list_things()\n",
        ),
        ("svc.py", "def list_things():\n    pass\n"),
        ("a.py", "def lonely_name():\n    pass\n"),
        ("b.py", "def lonely_name():\n    pass\n"),
    ]);
    assert_eq!(
        resolve_targets(&store, "list_things").unwrap(),
        ["res2.list_things", "svc.list_things"]
    );
    assert!(matches!(
        resolve_targets(&store, "lonely_name"),
        Err(CoreError::Ambiguous { .. })
    ));
}

/// GIVEN a negative weight in `[strategies]`
/// WHEN the configuration is parsed
/// THEN it is rejected; valid values replace only the given keys.
#[test]
fn test_given_strategies_config_when_parsed_then_validated() {
    assert!(Config::from_toml("[strategies]\nname = -1.0\n").is_err());
    let config = Config::from_toml("[strategies]\nconnectivity = 0.0\n").unwrap();
    assert_eq!(config.strategies.connectivity, 0.0);
    assert_eq!(config.strategies.neighborhood, 1.0);
}
