//! Resolve explicit indexed identities before interpreting the rest of a request as words.

use crate::error::CoreError;
use crate::query::resolve_qname;
use crate::store::SqliteStore;
use rusqlite::OptionalExtension;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct Targets {
    pub symbols: BTreeMap<i64, &'static str>,
    pub remaining: String,
}

pub(crate) fn resolve(store: &SqliteStore, request: &str) -> Result<Targets, CoreError> {
    let mut targets = Targets::default();
    for raw in request.split_whitespace() {
        let token = raw
            .trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '.' | '/'))
            .trim_end_matches('.');
        if token.ends_with(".py") {
            let paths: Vec<String> = store.conn.prepare(
                "SELECT path FROM file WHERE path = ?1 OR substr(path, -length(?1)-1) = '/' || ?1 ORDER BY path"
            )?.query_map([token], |r| r.get(0))?.collect::<Result<_, _>>()?;
            let paths = if paths.iter().any(|p| p == token) {
                vec![token.to_string()]
            } else {
                paths
            };
            if paths.len() > 1 {
                return Err(CoreError::Ambiguous {
                    target: token.into(),
                    candidates: paths,
                });
            }
            if let Some(path) = paths.first() {
                let ids = store
                    .conn
                    .prepare(
                        "SELECT s.id FROM symbol s JOIN file f ON f.id=s.file_id WHERE f.path=?1",
                    )?
                    .query_map([path], |r| r.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                for id in ids {
                    targets.symbols.entry(id).or_insert("arquivo");
                }
                continue;
            }
        } else if !token.is_empty() {
            match resolve_qname(store, token) {
                Ok(qname) => {
                    // Bare words that merely name a module remain conceptual requests.
                    let (id, kind): (i64, String) = store.conn.query_row(
                        "SELECT id,kind FROM symbol WHERE qname=?1",
                        [&qname],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?;
                    if kind != "module" || token.contains('.') {
                        targets.symbols.insert(id, "simbolo");
                        continue;
                    }
                }
                Err(CoreError::Ambiguous { target, candidates })
                    if token.contains('.')
                        || token.contains('_')
                        || token.chars().next().is_some_and(char::is_uppercase) =>
                {
                    // Homônimas ligadas por chamada exata (resource → service) viram um grupo.
                    let Some(group) = crate::relevance::exact_group(store, &candidates)? else {
                        return Err(CoreError::Ambiguous { target, candidates });
                    };
                    for qname in group {
                        let id: i64 = store.conn.query_row(
                            "SELECT id FROM symbol WHERE qname = ?1",
                            [&qname],
                            |r| r.get(0),
                        )?;
                        targets.symbols.insert(id, "simbolo");
                    }
                    continue;
                }
                Err(CoreError::Ambiguous { .. } | CoreError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        targets.remaining.push_str(raw);
        targets.remaining.push(' ');
    }
    Ok(targets)
}

/// Optional feature context is passed by the CLI; the ranking layer never opens another DB.
pub fn changed_symbols(
    base: &SqliteStore,
    feature: &SqliteStore,
) -> Result<Vec<String>, CoreError> {
    let base_rows: BTreeMap<String, (String, String)> = base
        .conn
        .prepare("SELECT qname, sig_hash, body_hash FROM symbol WHERE kind != 'module'")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    let rows = feature
        .conn
        .prepare("SELECT qname,sig_hash,body_hash FROM symbol WHERE kind != 'module'")?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, String>(2)?),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(q, hashes)| (base_rows.get(&q) != Some(&hashes)).then_some(q))
        .collect())
}

pub(crate) fn changed_ids(
    store: &SqliteStore,
    changed: &[String],
) -> Result<BTreeSet<i64>, CoreError> {
    let mut stmt = store.conn.prepare("SELECT id FROM symbol WHERE qname=?1")?;
    let mut ids = BTreeSet::new();
    for q in changed {
        if let Some(id) = stmt.query_row([q], |r| r.get(0)).optional()? {
            ids.insert(id);
        }
    }
    Ok(ids)
}
