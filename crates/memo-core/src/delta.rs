//! Deltas por task: só o que é novo ou mudou desde a última chamada (spec §8.4).

use crate::error::CoreError;
use crate::query::Impact;
use crate::store::SqliteStore;

/// Separa os itens ainda não vistos pela `task` para `cmd`; registra os que ficaram.
/// Com `full`, devolve todos e apenas registra. Devolve também quantos foram omitidos.
pub fn filter_new<T>(
    store: &SqliteStore,
    task: &str,
    cmd: &str,
    items: Vec<T>,
    key: impl Fn(&T) -> String,
    full: bool,
) -> Result<(Vec<T>, usize), CoreError> {
    let tx = store.conn.unchecked_transaction()?;
    let mut kept = Vec::new();
    let mut omitted = 0;
    for item in items {
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO session_seen(task, cmd, key) VALUES (?1, ?2, ?3)",
            rusqlite::params![task, cmd, key(&item)],
        )?;
        if inserted == 0 && !full {
            omitted += 1;
        } else {
            kept.push(item);
        }
    }
    tx.commit()?;
    Ok((kept, omitted))
}

/// Delta de `query impact`: chamadores, consumidores, Protocol e testes.
pub fn apply_impact_delta(
    store: &SqliteStore,
    task: &str,
    impact: &mut Impact,
    full: bool,
) -> Result<(), CoreError> {
    let target = impact.target.qname.clone();
    let rel = |i: &crate::query::RelItem| {
        format!(
            "{}|{}-{}|{:?}",
            i.addr.qname, i.addr.line_start, i.addr.line_end, i.line
        )
    };
    let mut omitted = 0;
    let (callers, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.callers),
        |c| {
            let who = c.addr.as_ref().map_or_else(
                || c.ext.clone().unwrap_or_default(),
                |a| format!("{}|{}-{}", a.qname, a.line_start, a.line_end),
            );
            format!("{target}|C|{who}|{}|{:?}", c.depth, c.line)
        },
        full,
    )?;
    impact.callers = callers;
    omitted += n;
    let (consumers, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.consumers),
        |i| format!("{target}|U|{}", rel(i)),
        full,
    )?;
    impact.consumers = consumers;
    omitted += n;
    let (protocol, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.protocol),
        |i| format!("{target}|P|{}", rel(i)),
        full,
    )?;
    impact.protocol = protocol;
    omitted += n;
    let (by_word, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.by_word),
        |i| format!("{target}|W|{}", rel(i)),
        full,
    )?;
    impact.by_word = by_word;
    omitted += n;
    let (exact, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.tests.exact),
        |i| format!("{target}|TE|{}", rel(i)),
        full,
    )?;
    impact.tests.exact = exact;
    omitted += n;
    let (approx, n) = filter_new(
        store,
        task,
        "impact",
        std::mem::take(&mut impact.tests.approx),
        |i| format!("{target}|TA|{}", rel(i)),
        full,
    )?;
    impact.tests.approx = approx;
    omitted += n;
    impact.unchanged = Some(omitted);
    Ok(())
}
