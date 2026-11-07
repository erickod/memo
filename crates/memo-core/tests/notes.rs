mod common;

use common::Repo;
use memo_core::brief::brief_with;
use memo_core::config::Config;
use memo_core::error::CoreError;
use memo_core::index::index_with;
use memo_core::notes::{NewNote, Note, NoteDir, format_utc, notes_for, weight};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;

const NOW: &str = "2026-11-07T14:05:00Z";
const EXECUTE: &str = "CalculateLimitUseCase.execute";
const EXECUTE_Q: &str =
    "app.billing.application.usecases.calculate_limit_usecase.CalculateLimitUseCase.execute";

struct Env {
    _repo: Repo,
    graph: SqliteStore,
    notes: NoteDir,
    dir: std::path::PathBuf,
}

fn env(config: &str) -> Env {
    let repo = Repo::fixture();
    let mut graph = SqliteStore::open_in_memory().unwrap();
    let config = Config::from_toml(config).unwrap();
    index_with(&WorkTree::new(repo.path()), &mut graph, &config).unwrap();
    let dir = repo.path().join(".git/memo/notes");
    Env {
        notes: NoteDir::new(&dir),
        _repo: repo,
        graph,
        dir,
    }
}

fn new_note(ancora: &str, text: &str) -> NewNote {
    NewNote {
        tipo: "regra".into(),
        ancora: ancora.into(),
        secundarias: Vec::new(),
        origem: "humano".into(),
        text: text.into(),
        task: None,
        confirm: false,
        substitui: None,
        aliases: Vec::new(),
    }
}

fn sym(qname_suffix: &str) -> String {
    format!("sym:{qname_suffix}")
}

/// GIVEN a note with a secondary anchor, a task, hashes and a replaced id
/// WHEN  it is rendered to Markdown and parsed back
/// THEN  the parsed note equals the original
#[test]
fn test_given_note_struct_when_rendered_and_parsed_then_roundtrip() {
    let note = Note {
        id: "n-4k2m9q1x".into(),
        tipo: "regra".into(),
        estado: "confirmada".into(),
        escopo: "task:lend-9452".into(),
        ancora: "sym:app.billing.domain.term.calculate_term_days".into(),
        secundarias: vec!["word:prazo".into()],
        origem: "humano".into(),
        task: Some("lend-9452".into()),
        ancora_sig: Some("9f3c1a".into()),
        ancora_body: Some("07be44".into()),
        criada: "2026-11-07T14:05:00Z".into(),
        substitui: Some("n-aaaaaaaa".into()),
        estado_anterior: None,
        aliases: vec!["prazo".into(), "term".into()],
        text: "Prazo é contado em dias corridos, por regra do contrato.".into(),
    };
    assert_eq!(Note::parse(&note.render()).unwrap(), note);
}

/// GIVEN the fixture graph and the anchor 'CalculateLimitUseCase.execute'
/// WHEN  a note is added without confirm
/// THEN  it is 'proposta', anchored on the full qname with sig and body hashes, and written to disk
#[test]
fn test_given_valid_sym_anchor_when_added_then_proposta_with_hashes_and_file_written() {
    let e = env("");
    let out = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), "Limite é inteiro."), NOW)
        .unwrap();
    assert_eq!(out.note.estado, "proposta");
    assert_eq!(out.note.ancora, sym(EXECUTE_Q));
    assert!(out.note.ancora_sig.is_some() && out.note.ancora_body.is_some());
    assert!(out.note.id.starts_with("n-") && out.note.id.len() == 10);
    assert!(e.dir.join(format!("{}.md", out.note.id)).exists());
}

/// GIVEN the flag confirm on a new note
/// WHEN  it is added
/// THEN  its state is 'confirmada'
#[test]
fn test_given_confirm_flag_when_added_then_state_confirmada() {
    let e = env("");
    let mut n = new_note(&sym(EXECUTE), "Limite é inteiro.");
    n.confirm = true;
    assert_eq!(
        e.notes.add(&e.graph, n, NOW).unwrap().note.estado,
        "confirmada"
    );
}

/// GIVEN an anchor 'sym:NoSuchSymbol' that is not in the graph
/// WHEN  a note is added
/// THEN  the error is NotFound (exit 2)
#[test]
fn test_given_missing_symbol_anchor_when_added_then_not_found() {
    let e = env("");
    let err = e
        .notes
        .add(&e.graph, new_note("sym:NoSuchSymbol", "x"), NOW)
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)), "{err:?}");
}

/// GIVEN skeleton ["usecase"] and the anchor 'word:usecase'
/// WHEN  a note is added
/// THEN  the error is Validation with code ancora_generica (exit 4)
#[test]
fn test_given_skeleton_word_anchor_when_added_then_validation_ancora_generica() {
    let e = env("skeleton = [\"usecase\"]\n");
    let err = e
        .notes
        .add(&e.graph, new_note("word:usecase", "x"), NOW)
        .unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::Validation {
                code: "ancora_generica",
                ..
            }
        ),
        "{err:?}"
    );
}

/// GIVEN the anchor 'word:zzzzzz' that no symbol uses
/// WHEN  a note is added
/// THEN  the error is NotFound
#[test]
fn test_given_unknown_word_anchor_when_added_then_not_found() {
    let e = env("");
    let err = e
        .notes
        .add(&e.graph, new_note("word:zzzzzz", "x"), NOW)
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)), "{err:?}");
}

/// GIVEN a note text of 1201 characters
/// WHEN  it is added
/// THEN  the error is Validation with code texto_longo and no file is written
#[test]
fn test_given_text_over_1200_chars_when_added_then_validation_error() {
    let e = env("");
    let err = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), &"a".repeat(1201)), NOW)
        .unwrap_err();
    assert!(
        matches!(
            err,
            CoreError::Validation {
                code: "texto_longo",
                ..
            }
        ),
        "{err:?}"
    );
    assert!(e.notes.list().unwrap().is_empty());
}

/// GIVEN a note text of exactly 1200 characters
/// WHEN  it is added
/// THEN  it is accepted
#[test]
fn test_given_text_of_1200_chars_when_added_then_accepted() {
    let e = env("");
    assert!(
        e.notes
            .add(&e.graph, new_note(&sym(EXECUTE), &"a".repeat(1200)), NOW)
            .is_ok()
    );
}

/// GIVEN six notes on the same anchor
/// WHEN  a seventh is added
/// THEN  the outcome lists exactly 5 conflicts
#[test]
fn test_given_same_anchor_notes_when_added_then_conflicts_listed_up_to_five() {
    let e = env("");
    for i in 0..6 {
        let n = new_note(&sym(EXECUTE), &format!("regra {i}"));
        e.notes.add(&e.graph, n, NOW).unwrap();
    }
    let out = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), "regra 7"), NOW)
        .unwrap();
    assert_eq!(out.conflicts.len(), 5);
}

/// GIVEN an existing note
/// WHEN  a new note is added with substitui pointing to it
/// THEN  the old note becomes 'substituida' and the new one records substitui
#[test]
fn test_given_substitui_when_added_then_old_note_substituida() {
    let e = env("");
    let old = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), "antiga"), NOW)
        .unwrap()
        .note;
    let mut n = new_note(&sym(EXECUTE), "nova");
    n.substitui = Some(old.id.clone());
    let out = e.notes.add(&e.graph, n, NOW).unwrap();
    assert_eq!(out.note.substitui.as_deref(), Some(old.id.as_str()));
    assert_eq!(e.notes.get(&old.id).unwrap().estado, "substituida");
}

/// GIVEN a substitui id that does not exist
/// WHEN  a note is added
/// THEN  the error is NotFound and no note is written
#[test]
fn test_given_substitui_unknown_id_when_added_then_not_found() {
    let e = env("");
    let mut n = new_note(&sym(EXECUTE), "nova");
    n.substitui = Some("n-zzzzzzzz".into());
    let err = e.notes.add(&e.graph, n, NOW).unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)), "{err:?}");
    assert!(e.notes.list().unwrap().is_empty());
}

/// GIVEN edge anchors: one with a real CHAMA edge and one between unrelated symbols
/// WHEN  notes are added
/// THEN  the first is accepted and the second is NotFound
#[test]
fn test_given_edge_anchor_when_added_then_requires_existing_edge() {
    let e = env("");
    let ok = format!("edge:{EXECUTE}->LimitRepository.save");
    assert!(e.notes.add(&e.graph, new_note(&ok, "x"), NOW).is_ok());
    let bad = format!("edge:{EXECUTE}->ChargeUseCase.execute");
    let err = e.notes.add(&e.graph, new_note(&bad, "x"), NOW).unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)), "{err:?}");
}

/// GIVEN file anchors: an indexed path and a missing path
/// WHEN  notes are added
/// THEN  the indexed path is accepted and the missing one is NotFound
#[test]
fn test_given_file_anchor_when_added_then_path_must_be_indexed() {
    let e = env("");
    let ok = "file:app/billing/domain/entities/invoice.py";
    assert!(e.notes.add(&e.graph, new_note(ok, "x"), NOW).is_ok());
    let err = e
        .notes
        .add(&e.graph, new_note("file:nope.py", "x"), NOW)
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound(_)), "{err:?}");
}

/// GIVEN a note scoped to task 'lend-1' and a repo-scoped note
/// WHEN  visibility is computed for no task, for 'lend-1' and for 'lend-2'
/// THEN  no task and 'lend-2' see only the repo note; 'lend-1' sees both
#[test]
fn test_given_task_scoped_note_when_viewed_by_other_task_then_hidden() {
    let e = env("");
    e.notes
        .add(&e.graph, new_note(&sym(EXECUTE), "repo"), NOW)
        .unwrap();
    let mut scoped = new_note(&sym(EXECUTE), "task");
    scoped.task = Some("lend-1".into());
    e.notes.add(&e.graph, scoped, NOW).unwrap();
    assert_eq!(e.notes.visible(None).unwrap().len(), 1);
    assert_eq!(e.notes.visible(Some("lend-2")).unwrap().len(), 1);
    assert_eq!(e.notes.visible(Some("lend-1")).unwrap().len(), 2);
}

/// GIVEN notes in states confirmada, proposta, stale, orfa and substituida on one symbol
/// WHEN  notes_for runs for that symbol
/// THEN  it returns confirmada, proposta, stale in that order, with weights 1.0, 0.5 and 0.3
#[test]
fn test_given_notes_in_all_states_when_notes_for_then_weighted_and_inactive_excluded() {
    let e = env("");
    let base = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), "base"), NOW)
        .unwrap()
        .note;
    let make = |estado: &str, id: &str| Note {
        id: id.into(),
        estado: estado.into(),
        ..base.clone()
    };
    let notes = vec![
        make("stale", "n-00000003"),
        make("orfa", "n-00000004"),
        make("proposta", "n-00000002"),
        make("substituida", "n-00000005"),
        make("confirmada", "n-00000001"),
    ];
    let got = notes_for(&notes, &[EXECUTE_Q.to_string()], &[], &[]);
    let states: Vec<&str> = got.iter().map(|n| n.estado.as_str()).collect();
    assert_eq!(states, vec!["confirmada", "proposta", "stale"]);
    let weights: Vec<f64> = got.iter().map(|n| weight(&n.estado)).collect();
    assert_eq!(weights, vec![1.0, 0.5, 0.3]);
}

/// GIVEN a stray temporary file 'n-dead0000.md.tmp' left by an interrupted write
/// WHEN  notes are listed and a new note is added
/// THEN  the stray file is ignored and is the only '.tmp' left (the add leaves none of its own)
#[test]
fn test_given_interrupted_write_when_listed_then_tmp_ignored_and_no_tmp_left() {
    let e = env("");
    std::fs::create_dir_all(&e.dir).unwrap();
    std::fs::write(e.dir.join("n-dead0000.md.tmp"), "---\nid: broken").unwrap();
    assert!(e.notes.list().unwrap().is_empty());
    e.notes
        .add(&e.graph, new_note(&sym(EXECUTE), "ok"), NOW)
        .unwrap();
    let tmp_left = std::fs::read_dir(&e.dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.file_name().to_string_lossy() == "n-dead0000.md.tmp")
        .count();
    assert_eq!(tmp_left, 1);
    assert_eq!(e.notes.list().unwrap().len(), 1);
}

/// GIVEN a 'proposta' note and an 'orfa' note
/// WHEN  confirm runs on each
/// THEN  the first becomes 'confirmada' and the second fails with Validation
#[test]
fn test_given_confirm_when_note_inactive_then_validation_error() {
    let e = env("");
    let n = e
        .notes
        .add(&e.graph, new_note(&sym(EXECUTE), "a"), NOW)
        .unwrap()
        .note;
    assert_eq!(e.notes.confirm(&n.id).unwrap().estado, "confirmada");
    let orphan = Note {
        id: "n-0rfa0000".into(),
        estado: "orfa".into(),
        ..n
    };
    std::fs::write(e.dir.join("n-0rfa0000.md"), orphan.render()).unwrap();
    let err = e.notes.confirm("n-0rfa0000").unwrap_err();
    assert!(matches!(err, CoreError::Validation { .. }), "{err:?}");
}

/// GIVEN a glossario note on word 'limit' with alias 'limite'
/// WHEN  brief runs for the request 'limite'
/// THEN  'limit' is an exact anchor
#[test]
fn test_given_glossario_alias_when_brief_then_alias_token_matches_word() {
    let e = env("min_specificity = 0.0\nskeleton = []\n");
    let mut n = new_note("word:limit", "Limite e limit são o mesmo conceito.");
    n.tipo = "glossario".into();
    n.aliases = vec!["limite".into()];
    e.notes.add(&e.graph, n, NOW).unwrap();
    let visible = e.notes.visible(None).unwrap();
    let b = brief_with(&e.graph, "limite", &visible).unwrap();
    assert_eq!(b.anchors[0].word, "limit");
    assert!(!b.anchors[0].approx);
}

/// GIVEN a confirmada note on CalculateLimitUseCase.execute
/// WHEN  brief runs for 'calculate limit'
/// THEN  the notes section has one line with the note id, its type and state
#[test]
fn test_given_notes_when_brief_then_notes_section_lists_matching_notes() {
    let e = env("min_specificity = 0.0\nskeleton = []\n");
    let mut n = new_note(&sym(EXECUTE), "Limite é inteiro.");
    n.confirm = true;
    let id = e.notes.add(&e.graph, n, NOW).unwrap().note.id;
    let visible = e.notes.visible(None).unwrap();
    let b = brief_with(&e.graph, "calculate limit", &visible).unwrap();
    assert_eq!(b.notes.len(), 1);
    assert!(
        b.notes[0].contains(&id) && b.notes[0].contains("regra confirmada"),
        "{:?}",
        b.notes
    );
}

/// GIVEN the Unix time 1794060300 (2026-11-07T14:05:00Z)
/// WHEN  it is formatted
/// THEN  the stamp is '2026-11-07T14:05:00Z'
#[test]
fn test_given_epoch_seconds_when_formatted_then_iso_utc() {
    assert_eq!(format_utc(1_794_060_300), "2026-11-07T14:05:00Z");
}
