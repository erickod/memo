mod common;

use common::Repo;
use memo_core::brief::brief_weighted;
use memo_core::config::Config;
use memo_core::index::index;
use memo_core::query::{ReadSpot, symbol_info};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn store_of(files: &[(&str, String)]) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    for (path, source) in files {
        repo.write(path, source);
    }
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

fn spot(path: &str, start: i64, end: i64, motivo: &str) -> ReadSpot {
    ReadSpot {
        path: path.to_string(),
        start,
        end,
        motivo: motivo.to_string(),
    }
}

/// `h.hub` e seis chamadores curtos em `a.py` (`f00` a `f05`, chamada na linha 4, 7, 10…).
fn six_callers() -> Vec<(&'static str, String)> {
    let body: String = (0..6)
        .map(|i| format!("def f{i:02}():\n    hub()\n\n"))
        .collect();
    vec![
        ("h.py", "def hub():\n    pass\n".to_string()),
        ("a.py", format!("from h import hub\n\n{body}")),
    ]
}

/// GIVEN hub with six short callers
/// WHEN  symbol_info runs on h.hub
/// THEN  ler has the target ('alvo') and the first 4 exact callers ('chama o alvo em :L<n>') (TASK-7, D4)
#[test]
fn test_given_symbol_with_callers_when_read_spots_then_target_and_four_callers() {
    let (_repo, store) = store_of(&six_callers());
    let info = symbol_info(&store, "h.hub").unwrap();
    assert_eq!(
        info.ler,
        vec![
            spot("h.py", 1, 2, "alvo"),
            spot("a.py", 3, 4, "chama o alvo em :L4"),
            spot("a.py", 6, 7, "chama o alvo em :L7"),
            spot("a.py", 9, 10, "chama o alvo em :L10"),
            spot("a.py", 12, 13, "chama o alvo em :L13"),
        ]
    );
}

/// GIVEN a target of 80 lines and a caller of 80 lines that calls it on its 50th line
/// WHEN  symbol_info runs on the target
/// THEN  the target spot is its first 20 lines and the caller spot is a 20-line window around the call (TASK-7, D4)
#[test]
fn test_given_large_symbols_when_read_spots_then_windows_of_twenty_lines() {
    let filler: String = (0..78).map(|i| format!("    x{i} = {i}\n")).collect();
    let target = format!("def big():\n{filler}    return 0\n");
    let before: String = (0..48).map(|i| format!("    y{i} = {i}\n")).collect();
    let after: String = (0..30).map(|i| format!("    z{i} = {i}\n")).collect();
    let caller = format!("from h import big\n\ndef caller():\n{before}    big()\n{after}");
    let (_repo, store) = store_of(&[("h.py", target), ("a.py", caller)]);
    let info = symbol_info(&store, "h.big").unwrap();
    assert_eq!(info.ler[0], spot("h.py", 1, 20, "alvo"));
    assert_eq!(info.ler[1], spot("a.py", 42, 61, "chama o alvo em :L52"));
}

/// GIVEN a function nobody calls
/// WHEN  symbol_info runs on it
/// THEN  ler has only the target (TASK-7, D4)
#[test]
fn test_given_symbol_without_callers_when_read_spots_then_only_target() {
    let (_repo, store) = store_of(&[("h.py", "def lonely():\n    pass\n".to_string())]);
    let info = symbol_info(&store, "h.lonely").unwrap();
    assert_eq!(info.ler, vec![spot("h.py", 1, 2, "alvo")]);
}

/// GIVEN seven production functions whose names carry 'order'
/// WHEN  the brief for 'order' runs
/// THEN  ler has the first 5 production symbols of the list, each with 'carrega order' (TASK-7, D4)
#[test]
fn test_given_brief_when_read_spots_then_first_five_production_symbols() {
    let body: String = (0..7)
        .map(|i| format!("def order_{i}():\n    pass\n\n"))
        .collect();
    let (_repo, store) = store_of(&[("o.py", body)]);
    let config = Config::default();
    let b = brief_weighted(&store, "order", &[], &config.bridges).unwrap();
    let production: Vec<_> = b.symbols.iter().filter(|s| !s.in_tests).take(5).collect();
    assert_eq!(b.ler.len(), 5, "{:?}", b.ler);
    for (spot, symbol) in b.ler.iter().zip(production) {
        assert_eq!(spot.path, symbol.addr.path);
        assert_eq!(spot.start, symbol.addr.line_start);
        assert_eq!(spot.motivo, "carrega order");
    }
}
