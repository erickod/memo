//! Indexação incremental (spec §6.3).

use crate::config::Config;
use crate::error::CoreError;
use crate::events::{Delta, EventSummary};
use crate::extract::{FileFacts, SymbolKind, extract_python_file};
use crate::resolve::resolve_files;
use crate::source::{SourceFile, SourceTree};
use crate::store::SqliteStore;
use crate::words::{NameKind, Segment, Slot, classify_name_with};
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
    pub events: EventSummary,
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
pub fn module_of(path: &str) -> String {
    let stem = path.strip_suffix(".py").unwrap_or(path);
    let stem = stem.strip_suffix("/__init__").unwrap_or(stem);
    stem.replace('/', ".")
}

/// Contexto (segmento após `app/`) e camada (`domain`, `application` ou `infra`), spec §4.
fn context_and_layer(path: &str, config: &Config) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = path.split('/').collect();
    let context = (parts.first() == Some(&config.context_root.as_str()) && parts.len() > 2)
        .then(|| parts[1].to_string());
    let layer = parts
        .iter()
        .find(|p| config.layers.iter().any(|l| l == **p))
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
        "DELETE FROM edge WHERE src IN (SELECT id FROM symbol WHERE file_id = ?1)
            OR dst IN (SELECT id FROM symbol WHERE file_id = ?1)",
        [file_id],
    )?;
    tx.execute("DELETE FROM raw_ref WHERE file_id = ?1", [file_id])?;
    tx.execute("DELETE FROM import WHERE file_id = ?1", [file_id])?;
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
    config: &Config,
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
                                sig_hash, body_hash, is_test, is_protocol, type_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                is_test,
                sym.is_protocol,
                sym.type_text
            ],
        )?;
        let symbol_id = tx.last_insert_rowid();
        ids.insert(&sym.qname, symbol_id);
        let name_kind = match (sym.kind, is_test) {
            (SymbolKind::Attribute, _) => NameKind::Attribute,
            (_, true) => NameKind::Test,
            _ => NameKind::Callable,
        };
        for (pos, occ) in classify_name_with(name, name_kind, &config.vocabulary)
            .iter()
            .enumerate()
        {
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
    for r in &facts.receives {
        if let Some(&src) = ids.get(r.src_qname.as_str()) {
            let raw = NewRawRef {
                file_id,
                src,
                kind: "RECEBE",
                text: r.dst_text.as_deref(),
                line: r.line,
                attr: Some(&r.attr),
                pos: Some(r.pos),
                receiver: None,
            };
            insert_raw_ref(tx, &raw)?;
        }
    }
    for r in &facts.refs {
        if let Some(&src) = ids.get(r.src_qname.as_str()) {
            let raw = NewRawRef {
                file_id,
                src,
                kind: r.kind.as_str(),
                text: Some(&r.text),
                line: r.line,
                attr: None,
                pos: None,
                receiver: r.receiver.as_deref(),
            };
            insert_raw_ref(tx, &raw)?;
        }
    }
    for i in &facts.imports {
        tx.execute(
            "INSERT INTO import(file_id, local_name, target, line, type_checking)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                file_id,
                i.local_name,
                i.target,
                int(i.line),
                i.type_checking
            ],
        )?;
    }
    Ok(())
}

struct NewRawRef<'a> {
    file_id: i64,
    src: i64,
    kind: &'a str,
    text: Option<&'a str>,
    line: usize,
    attr: Option<&'a str>,
    pos: Option<usize>,
    receiver: Option<&'a str>,
}

fn insert_raw_ref(tx: &Transaction, r: &NewRawRef) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO raw_ref(file_id, src, kind, callee_text, line, attr, pos, receiver)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            r.file_id,
            r.src,
            r.kind,
            r.text,
            int(r.line),
            r.attr,
            r.pos.map(int),
            r.receiver
        ],
    )?;
    Ok(())
}

/// Indexa com a configuração padrão (sementes embutidas).
pub fn index(tree: &dyn SourceTree, store: &mut SqliteStore) -> Result<IndexReport, CoreError> {
    index_with(tree, store, &Config::default())
}

/// Como `index_with`, e aplica às notas as transições da §7.3 (`stale`, `orfa`, reancoragem).
///
/// As notas são reavaliadas a cada chamada, também sem mudança de arquivo: as transições
/// comparam estado com estado, então são idempotentes e se recuperam de uma interrupção.
pub fn index_with_notes(
    tree: &dyn SourceTree,
    store: &mut SqliteStore,
    config: &Config,
    notes: &crate::notes::NoteDir,
) -> Result<IndexReport, CoreError> {
    index_with_notes_scoped(tree, store, config, notes, "repo")
}

/// Como `index_with_notes`, só para as notas do `escopo` (`repo` ou `task:<id>`).
pub fn index_with_notes_scoped(
    tree: &dyn SourceTree,
    store: &mut SqliteStore,
    config: &Config,
    notes: &crate::notes::NoteDir,
    scope: &str,
) -> Result<IndexReport, CoreError> {
    let (mut report, delta) = run_index(tree, store, config)?;
    let ctx = crate::notes::Transitions {
        config,
        delta: &delta,
        scope,
    };
    if crate::notes::apply_transitions(store, notes, &ctx, &mut report.events)? {
        report.wrote = true;
    }
    Ok(report)
}

pub fn index_with(
    tree: &dyn SourceTree,
    store: &mut SqliteStore,
    config: &Config,
) -> Result<IndexReport, CoreError> {
    Ok(run_index(tree, store, config)?.0)
}

fn run_index(
    tree: &dyn SourceTree,
    store: &mut SqliteStore,
    config: &Config,
) -> Result<(IndexReport, Delta), CoreError> {
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
        return Ok((report, Delta::default()));
    }

    let parsed: Vec<_> = pending
        .par_iter()
        .map(|p| {
            let is_package = p.file.path.ends_with("__init__.py");
            extract_python_file(&module_of(&p.file.path), is_package, &p.content)
        })
        .collect();

    let tx = store.conn.transaction()?;
    let before = crate::events::snapshot(&tx)?;
    let affected = affected_files(&tx, &stored, &pending, &removed)?;
    let mut to_resolve: Vec<i64> = Vec::new();
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
        let (context, layer) = context_and_layer(&p.file.path, config);
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
        let file_id = tx.last_insert_rowid();
        if let Ok(facts) = &result {
            insert_symbols(&tx, file_id, &p.file.path, facts, config)?;
        }
        to_resolve.push(file_id);
    }
    for id in affected {
        if !to_resolve.contains(&id) {
            to_resolve.push(id);
        }
    }
    resolve_files(&tx, &to_resolve, &config.write_methods)?;
    crate::stats::recompute(&tx, config)?;
    let after = crate::events::snapshot(&tx)?;
    let mut diff = crate::events::diff(&before, &after);
    if !diff.events.is_empty() {
        let batch = crate::events::next_batch(&tx)?;
        crate::events::record(&tx, &batch, &diff.events)?;
        diff.delta.batch = Some(batch);
    }
    tx.commit()?;

    report.wrote = true;
    report.events = diff.summary;
    report.files_with_error = store.files_with_error()?;
    Ok((report, diff.delta))
}

/// Arquivos não alterados cujas arestas dependem dos que mudam: quem aponta para os símbolos
/// deles ou importa os módulos deles (spec §6.3, passo 3).
fn affected_files(
    tx: &Transaction,
    stored: &HashMap<String, StoredFile>,
    pending: &[Pending],
    removed: &[&String],
) -> Result<Vec<i64>, CoreError> {
    let mut changed_ids: Vec<i64> = removed.iter().map(|p| stored[*p].id).collect();
    changed_ids.extend(
        pending
            .iter()
            .filter_map(|p| stored.get(&p.file.path))
            .map(|s| s.id),
    );
    let modules: Vec<String> = removed
        .iter()
        .map(|p| module_of(p))
        .chain(pending.iter().map(|p| module_of(&p.file.path)))
        .collect();

    let mut out: HashSet<i64> = HashSet::new();
    for id in &changed_ids {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT s.file_id FROM edge e
             JOIN symbol s ON s.id = e.src JOIN symbol d ON d.id = e.dst
             WHERE d.file_id = ?1",
        )?;
        for row in stmt.query_map([id], |r| r.get::<_, i64>(0))? {
            out.insert(row?);
        }
    }
    // Chamadas via Protocol dependem das implementações conhecidas, que podem mudar em outro arquivo.
    let mut stmt = tx.prepare(
        "SELECT DISTINCT s.file_id FROM edge e
         JOIN symbol s ON s.id = e.src JOIN symbol d ON d.id = e.dst
         JOIN symbol p ON p.id = d.parent_id
         WHERE p.is_protocol = 1 AND e.kind = 'CHAMA' AND e.approx = 0",
    )?;
    for row in stmt.query_map([], |r| r.get::<_, i64>(0))? {
        out.insert(row?);
    }
    for module in &modules {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT file_id FROM import
             WHERE target = ?1 OR substr(target, 1, length(?1) + 1) = ?1 || '.'",
        )?;
        for row in stmt.query_map([module], |r| r.get::<_, i64>(0))? {
            out.insert(row?);
        }
    }
    let mut ids: Vec<i64> = out
        .into_iter()
        .filter(|id| !changed_ids.contains(id))
        .collect();
    ids.sort_unstable();
    Ok(ids)
}
