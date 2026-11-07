//! Eventos de indexação e de notas, em lote (spec §10).

use crate::error::CoreError;
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Contagens do lote, para o resumo de uma linha ao fim de `index`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventSummary {
    pub created: usize,
    pub changed: usize,
    pub renamed: usize,
    pub destroyed: usize,
    pub notes_stale: usize,
    pub notes_orphaned: usize,
}

impl EventSummary {
    /// `eventos: +12 símbolos, ~5 alterados, 1 renomeado, 2 notas stale`.
    pub fn line(&self) -> String {
        let renamed = if self.renamed == 1 {
            "renomeado"
        } else {
            "renomeados"
        };
        let notes = if self.notes_stale == 1 {
            "nota"
        } else {
            "notas"
        };
        format!(
            "eventos: +{} símbolos, ~{} alterados, {} {renamed}, {} {notes} stale",
            self.created, self.changed, self.renamed, self.notes_stale
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub kind: &'static str,
    pub subject: Option<String>,
    pub payload: Option<String>,
}

/// O que um lote de indexação mudou, para as transições de nota.
#[derive(Debug, Clone, Default)]
pub struct Delta {
    /// Pares `(de, para)` de renomeação.
    pub renames: Vec<(String, String)>,
    /// Símbolos cuja assinatura mudou.
    pub sig_changed: HashSet<String>,
    /// Lote dos eventos de símbolo, se houve algum.
    pub batch: Option<String>,
}

pub(crate) struct Snapshot {
    /// qname → (kind, sig_hash, body_hash)
    symbols: BTreeMap<String, (String, String, String)>,
    words: BTreeSet<String>,
}

pub(crate) fn snapshot(conn: &Connection) -> Result<Snapshot, CoreError> {
    let mut symbols = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT qname, kind, COALESCE(sig_hash, ''), COALESCE(body_hash, '') FROM symbol",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?)))
    })?;
    for row in rows {
        let (qname, value) = row?;
        symbols.insert(qname, value);
    }
    let mut stmt =
        conn.prepare("SELECT DISTINCT w.text FROM word w JOIN occurrence o ON o.word_id = w.id")?;
    let words = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(Snapshot { symbols, words })
}

pub(crate) struct Diff {
    pub events: Vec<NewEvent>,
    pub summary: EventSummary,
    pub delta: Delta,
}

fn json(value: serde_json::Value) -> Option<String> {
    Some(value.to_string())
}

/// Eventos entre dois instantâneos (spec §10). Renomeação exige `kind`, assinatura e corpo
/// iguais e pareamento único; mudança só de linha não aparece, porque a linha não é hash.
pub(crate) fn diff(before: &Snapshot, after: &Snapshot) -> Diff {
    let gone: Vec<&String> = before
        .symbols
        .keys()
        .filter(|q| !after.symbols.contains_key(*q))
        .collect();
    let born: Vec<&String> = after
        .symbols
        .keys()
        .filter(|q| !before.symbols.contains_key(*q))
        .collect();
    let group = |names: &[&String], snap: &Snapshot| {
        let mut groups: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
        for q in names {
            groups
                .entry(snap.symbols[*q].clone())
                .or_default()
                .push((*q).clone());
        }
        groups
    };
    let (gone_groups, born_groups) = (group(&gone, before), group(&born, after));
    let mut renames: Vec<(String, String)> = Vec::new();
    for (key, old) in &gone_groups {
        let new = born_groups.get(key).map(Vec::as_slice);
        if let ([from], Some([to])) = (old.as_slice(), new) {
            renames.push((from.clone(), to.clone()));
        }
    }
    renames.sort();
    let renamed_from: HashSet<&String> = renames.iter().map(|(d, _)| d).collect();
    let renamed_to: HashSet<&String> = renames.iter().map(|(_, p)| p).collect();

    let mut events = Vec::new();
    let mut summary = EventSummary::default();
    for (de, para) in &renames {
        events.push(NewEvent {
            kind: "SYMBOL_RENAMED",
            subject: Some(de.clone()),
            payload: json(serde_json::json!({ "de": de, "para": para })),
        });
        summary.renamed += 1;
    }
    for q in born.iter().filter(|q| !renamed_to.contains(**q)) {
        events.push(NewEvent {
            kind: "SYMBOL_CREATED",
            subject: Some((*q).clone()),
            payload: None,
        });
        summary.created += 1;
    }
    for q in gone.iter().filter(|q| !renamed_from.contains(**q)) {
        events.push(NewEvent {
            kind: "SYMBOL_DESTROYED",
            subject: Some((*q).clone()),
            payload: None,
        });
        summary.destroyed += 1;
    }
    let mut sig_changed = HashSet::new();
    let mut body_events = Vec::new();
    for (qname, (_, sig_before, body_before)) in &before.symbols {
        let Some((_, sig_after, body_after)) = after.symbols.get(qname) else {
            continue;
        };
        if sig_before != sig_after {
            events.push(NewEvent {
                kind: "SYMBOL_SIGNATURE_CHANGED",
                subject: Some(qname.clone()),
                payload: json(serde_json::json!({ "before": sig_before, "after": sig_after })),
            });
            sig_changed.insert(qname.clone());
            summary.changed += 1;
        } else if body_before != body_after {
            body_events.push(NewEvent {
                kind: "SYMBOL_BODY_CHANGED",
                subject: Some(qname.clone()),
                payload: json(serde_json::json!({ "before": body_before, "after": body_after })),
            });
            summary.changed += 1;
        }
    }
    events.extend(body_events);
    for word in after.words.difference(&before.words) {
        events.push(NewEvent {
            kind: "WORD_CREATED",
            subject: Some(word.clone()),
            payload: None,
        });
    }
    for word in before.words.difference(&after.words) {
        events.push(NewEvent {
            kind: "WORD_DESTROYED",
            subject: Some(word.clone()),
            payload: None,
        });
    }
    Diff {
        events,
        summary,
        delta: Delta {
            renames,
            sig_changed,
            batch: None,
        },
    }
}

/// Próximo lote: `b-<n>`, sequencial, sem relógio.
pub(crate) fn next_batch(conn: &Connection) -> Result<String, CoreError> {
    let n: i64 = conn.query_row("SELECT COUNT(DISTINCT batch) FROM event", [], |r| r.get(0))?;
    Ok(format!("b-{}", n + 1))
}

/// Grava eventos no lote, continuando a sequência dele.
pub(crate) fn record(conn: &Connection, batch: &str, events: &[NewEvent]) -> Result<(), CoreError> {
    let mut seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM event WHERE batch = ?1",
        [batch],
        |r| r.get(0),
    )?;
    for e in events {
        seq += 1;
        conn.execute(
            "INSERT INTO event(batch, seq, kind, subject, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![batch, seq, e.kind, e.subject, e.payload],
        )?;
    }
    Ok(())
}

/// Um lote novo só com estes eventos (comandos de nota).
pub(crate) fn record_new_batch(conn: &Connection, events: &[NewEvent]) -> Result<(), CoreError> {
    let batch = next_batch(conn)?;
    record(conn, &batch, events)
}
