mod common;

use common::Repo;
use memo_core::index::index;
use memo_core::query::{EnumMember, symbol_info};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

fn store_of(source: &str) -> (Repo, SqliteStore) {
    let repo = Repo::empty();
    repo.write("m.py", source);
    let mut store = SqliteStore::open_in_memory().unwrap();
    index(&WorkTree::new(repo.path()), &mut store).unwrap();
    (repo, store)
}

fn member(name: &str, value: Option<&str>) -> EnumMember {
    EnumMember {
        name: name.to_string(),
        value: value.map(str::to_string),
    }
}

/// GIVEN 'class Status(StrEnum)' with A = "a", B = auto() and C = 3
/// WHEN  symbol_info runs on m.Status
/// THEN  the members are A = "a", B (no literal) and C = 3, in source order (TASK-7, D6)
#[test]
fn test_given_enum_when_symbol_info_then_members_with_literal_values() {
    let (_repo, store) = store_of(
        "from enum import StrEnum, auto\n\nclass Status(StrEnum):\n    A = \"a\"\n    B = auto()\n    C = 3\n",
    );
    let info = symbol_info(&store, "m.Status").unwrap();
    assert_eq!(
        info.membros,
        vec![
            member("A", Some("\"a\"")),
            member("B", None),
            member("C", Some("3"))
        ]
    );
}

/// GIVEN 'class Color(enum.Enum)' with RED = 1
/// WHEN  symbol_info runs on m.Color
/// THEN  RED = 1 is a member (TASK-7, D6)
#[test]
fn test_given_module_qualified_enum_base_when_symbol_info_then_members_found() {
    let (_repo, store) = store_of("import enum\n\nclass Color(enum.Enum):\n    RED = 1\n");
    let info = symbol_info(&store, "m.Color").unwrap();
    assert_eq!(info.membros, vec![member("RED", Some("1"))]);
}

/// GIVEN a plain class with an unannotated class-level assignment
/// WHEN  symbol_info runs on it
/// THEN  it has no members (TASK-7, D6)
#[test]
fn test_given_plain_class_assignment_when_symbol_info_then_no_members() {
    let (_repo, store) = store_of("class Plain:\n    X = 1\n");
    let info = symbol_info(&store, "m.Plain").unwrap();
    assert!(info.membros.is_empty(), "{:?}", info.membros);
}
