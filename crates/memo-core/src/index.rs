//! Indexação incremental (spec §6.3).

use crate::error::CoreError;
use crate::extract::{FileFacts, SymbolKind, extract_python};
use crate::source::{SourceFile, SourceTree};
use crate::store::SqliteStore;
use crate::words::{NameKind, Segment, Slot, classify_name};
use rayon::prelude::*;
use rusqlite::{OptionalExtension, Transaction, params};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub files_total: usize,
    pub files_changed: usize,
    pub files_removed: usize,
    pub files_with_error: Vec<String>,
    pub wrote: bool,
}

struct StoredFile {
    id: i64,
    mtime_ns: i64,
    size: i64,
    hash: String,
}

struct Pending {
    file: SourceFile,
    hash: String,
    content: String,
}

/// Caminho do arquivo para módulo pontuado (`app/a/b.py` → `app.a.b`; `__init__` vira o pacote).
fn module_of(path: &str) -> String {
    let stem = path.strip_suffix(".py").unwrap_or(path);
    let stem = stem.strip_suffix("/__init__").unwrap_or(stem);
    stem.replace('/', ".")
}

/// Contexto (segmento após `app/`) e camada (`domain`, `application` ou `infra`), spec §4.
fn context_and_layer(path: &str) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = path.split('/').collect();
    let context = (parts.first() == Some(&"app") && parts.len() > 2).then(|| parts[1].to_string());
    let layer = parts
        .iter()
        .find(|p| matches!(**p, "domain" | "application" | "infra"))
        .map(|p| (*p).to_string());
    (context, layer)
}

fn is_test_file(path: &str) -> bool {
    let mut parts = path.split('/');
    let file = parts.next_back().unwrap_or("");
    file.starts_with("test_") || parts.any(|p| p == "tests")
}

fn int(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn kind_label(kind: SymbolKind) -> (&'static str, i64) {
    match kind {
        SymbolKind::Attribute => ("attribute", 1),
        SymbolKind::Class => ("class", 2),
        SymbolKind::Function => ("function", 3),
        SymbolKind::Method => ("method", 3),
        SymbolKind::Module => ("module", 4),
    }
}

fn slot_label(slot: Slot) -> &'static str {
    match slot {
        Slot::Action => "action",
        Slot::Object => "object",
        Slot::Role => "role",
    }
}

fn segment_label(segment: Segment) -> &'static str {
    match segment {
        Segment::Given => "given",
        Segment::When => "when",
        Segment::Then => "then",
    }
}

fn load_files(store: &SqliteStore) -> Result<HashMap<String, StoredFile>, CoreError> {
    let mut stmt = store
        .conn
        .prepare("SELECT path, id, mtime_ns, size, hash FROM file")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            StoredFile {
                id: r.get(1)?,
                mtime_ns: r.get(2)?,
                size: r.get(3)?,
                hash: r.get(4)?,
            },
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn delete_file_symbols(tx: &Transaction, file_id: i64) -> Result<(), CoreError> {
    tx.execute(
        "DELETE FROM occurrence WHERE symbol_id IN (SELECT id FROM symbol WHERE file_id = ?1)",
        [file_id],
    )?;
    tx.execute("DELETE FROM symbol WHERE file_id = ?1", [file_id])?;
    Ok(())
}

fn word_id(tx: &Transaction, text: &str) -> Result<i64, CoreError> {
    tx.execute("INSERT OR IGNORE INTO word(text) VALUES (?1)", [text])?;
    Ok(tx.query_row("SELECT id FROM word WHERE text = ?1", [text], |r| r.get(0))?)
}

fn insert_symbols(
    tx: &Transaction,
    file_id: i64,
    path: &str,
    facts: &FileFacts,
) -> Result<(), CoreError> {
    let test_file = is_test_file(path);
    let mut ids: HashMap<&str, i64> = HashMap::new();
    for sym in &facts.symbols {
        let (kind, nivel) = kind_label(sym.kind);
        let name = sym.qname.rsplit('.').next().unwrap_or(&sym.qname);
        let callable = matches!(sym.kind, SymbolKind::Function | SymbolKind::Method);
        let is_test = callable && (test_file || name.starts_with("test_"));
        let parent = sym
            .qname
            .rsplit_once('.')
            .and_then(|(p, _)| ids.get(p).copied());
        tx.execute(
            "INSERT INTO symbol(qname, name, kind, nivel, parent_id, file_id, line_start, line_end,
                                sig_hash, body_hash, is_test)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                sym.qname,
                name,
                kind,
                nivel,
                parent,
                file_id,
                int(sym.line_start),
                int(sym.line_end),
                sym.sig_hash,
                sym.body_hash,
                is_test
            ],
        )?;
        let symbol_id = tx.last_insert_rowid();
        ids.insert(&sym.qname, symbol_id);
        let name_kind = match (sym.kind, is_test) {
            (SymbolKind::Attribute, _) => NameKind::Attribute,
            (_, true) => NameKind::Test,
            _ => NameKind::Callable,
        };
        for (pos, occ) in classify_name(name, name_kind).iter().enumerate() {
            tx.execute(
                "INSERT INTO occurrence(symbol_id, word_id, pos, slot, segment)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    symbol_id,
                    word_id(tx, &occ.text)?,
                    int(pos),
                    slot_label(occ.slot),
                    occ.segment.map(segment_label)
                ],
            )?;
        }
    }
    Ok(())
}

pub fn index(tree: &dyn SourceTree, store: &mut SqliteStore) -> Result<IndexReport, CoreError> {
    let listed = tree.list()?;
    let stored = load_files(store)?;

    let mut touched: Vec<&SourceFile> = Vec::new();
    let mut pending: Vec<Pending> = Vec::new();
    for file in &listed {
        let known = stored.get(&file.path);
        if known.is_some_and(|k| k.mtime_ns == file.mtime_ns && k.size == file.size) {
            continue;
        }
        let content = tree.read(&file.path)?;
        let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        if known.is_some_and(|k| k.hash == hash) {
            touched.push(file);
        } else {
            pending.push(Pending {
                file: file.clone(),
                hash,
                content,
            });
        }
    }
    let present: HashSet<&str> = listed.iter().map(|f| f.path.as_str()).collect();
    let mut removed: Vec<&String> = stored
        .keys()
        .filter(|p| !present.contains(p.as_str()))
        .collect();
    removed.sort();

    let mut report = IndexReport {
        files_total: listed.len(),
        files_changed: pending.len(),
        files_removed: removed.len(),
        ..IndexReport::default()
    };
    if pending.is_empty() && touched.is_empty() && removed.is_empty() {
        report.files_with_error = store.files_with_error()?;
        return Ok(report);
    }

    let parsed: Vec<_> = pending
        .par_iter()
        .map(|p| extract_python(&module_of(&p.file.path), &p.content))
        .collect();

    let tx = store.conn.transaction()?;
    for path in removed {
        let id = stored[path].id;
        delete_file_symbols(&tx, id)?;
        tx.execute("DELETE FROM file WHERE id = ?1", [id])?;
    }
    for file in touched {
        tx.execute(
            "UPDATE file SET mtime_ns = ?1, size = ?2 WHERE path = ?3",
            params![file.mtime_ns, file.size, file.path],
        )?;
    }
    for (p, result) in pending.iter().zip(parsed) {
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM file WHERE path = ?1", [&p.file.path], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(id) = existing {
            delete_file_symbols(&tx, id)?;
            tx.execute("DELETE FROM file WHERE id = ?1", [id])?;
        }
        let (context, layer) = context_and_layer(&p.file.path);
        let parse_error = result.as_ref().err().map(|e| e.0.clone());
        tx.execute(
            "INSERT INTO file(path, hash, mtime_ns, size, context, layer, parse_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                p.file.path,
                p.hash,
                p.file.mtime_ns,
                p.file.size,
                context,
                layer,
                parse_error
            ],
        )?;
        if let Ok(facts) = &result {
            insert_symbols(&tx, tx.last_insert_rowid(), &p.file.path, facts)?;
        }
    }
    tx.commit()?;

    report.wrote = true;
    report.files_with_error = store.files_with_error()?;
    Ok(report)
}
