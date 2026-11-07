//! Consultas sobre o grafo (spec §8.2).

use crate::error::CoreError;
use crate::index::module_of;
use crate::store::SqliteStore;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolAddr {
    pub qname: String,
    /// Nome relativo ao módulo do arquivo (`Classe.metodo`), ou o próprio módulo.
    pub short: String,
    pub kind: String,
    pub is_test: bool,
    pub path: String,
    pub line_start: i64,
    pub line_end: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolInfo {
    #[serde(flatten)]
    pub addr: SymbolAddr,
    pub sig_hash: String,
    pub edges: Vec<EdgeView>,
}

/// Aresta direta de um símbolo: `out` (ele aponta) ou `in` (apontam para ele).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EdgeView {
    pub dir: String,
    pub kind: String,
    /// Nome curto do outro extremo, ou o texto externo/não resolvido.
    pub other: String,
    pub attr: Option<String>,
    pub line: Option<i64>,
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelItem {
    pub addr: SymbolAddr,
    pub attr: Option<String>,
    pub line: Option<i64>,
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Relation {
    pub target: SymbolAddr,
    pub items: Vec<RelItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeInfo {
    pub target: SymbolAddr,
    pub producers: Vec<RelItem>,
    pub consumers: Vec<RelItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
    pub resolvidas: i64,
    pub aproximadas: i64,
    pub nao_resolvidas: i64,
    pub arquivos_com_erro: i64,
}

/// Símbolos que escrevem o atributo (`ESCREVE`).
pub fn writers(_store: &SqliteStore, _target: &str) -> Result<Relation, CoreError> {
    todo!("spec §8.2 query writers")
}

/// Símbolos que leem o atributo (`LE`).
pub fn readers(_store: &SqliteStore, _target: &str) -> Result<Relation, CoreError> {
    todo!("spec §8.2 query readers")
}

/// Quem produz (`RETORNA`) e quem consome (`RECEBE`) o tipo.
pub fn type_info(_store: &SqliteStore, _target: &str) -> Result<TypeInfo, CoreError> {
    todo!("spec §8.2 query type")
}

pub fn coverage(_store: &SqliteStore) -> Result<Coverage, CoreError> {
    todo!("spec §4 cobertura")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WordInfo {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
    pub symbols: Vec<SymbolAddr>,
    pub neighbors: Vec<String>,
}

const ADDR_COLUMNS: &str =
    "s.qname, s.kind, s.is_test, COALESCE(f.path, ''), s.line_start, s.line_end";

fn addr_from_row(r: &rusqlite::Row) -> rusqlite::Result<SymbolAddr> {
    let qname: String = r.get(0)?;
    let path: String = r.get(3)?;
    let module = module_of(&path);
    let short = qname
        .strip_prefix(&format!("{module}."))
        .unwrap_or_else(|| qname.rsplit('.').next().unwrap_or(&qname))
        .to_string();
    Ok(SymbolAddr {
        short,
        qname,
        kind: r.get(1)?,
        is_test: r.get::<_, i64>(2)? != 0,
        path,
        line_start: r.get(4)?,
        line_end: r.get(5)?,
    })
}

/// Aceita `qname` completo ou sufixo único; sufixo ambíguo devolve `Ambiguous`.
pub fn symbol_info(store: &SqliteStore, target: &str) -> Result<SymbolInfo, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT qname FROM symbol
         WHERE qname = ?1
            OR (length(qname) > length(?1) + 1 AND substr(qname, -length(?1) - 1) = '.' || ?1)
         ORDER BY qname",
    )?;
    let mut found: Vec<String> = stmt
        .query_map([target], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if let Some(exact) = found.iter().find(|q| q.as_str() == target) {
        found = vec![exact.clone()];
    }
    let qname = match found.len() {
        0 => return Err(CoreError::NotFound(target.to_string())),
        1 => found.remove(0),
        _ => {
            return Err(CoreError::Ambiguous {
                target: target.to_string(),
                candidates: found,
            });
        }
    };
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, COALESCE(s.sig_hash, '') FROM symbol s
         LEFT JOIN file f ON f.id = s.file_id WHERE s.qname = ?1"
    );
    Ok(store.conn.query_row(&sql, [&qname], |r| {
        Ok(SymbolInfo {
            addr: addr_from_row(r)?,
            sig_hash: r.get(6)?,
            edges: Vec::new(),
        })
    })?)
}

pub fn word_info(store: &SqliteStore, word: &str) -> Result<WordInfo, CoreError> {
    let word = word.to_lowercase();
    let word_id: i64 = store
        .conn
        .query_row("SELECT id FROM word WHERE text = ?1", [&word], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound(word.clone()),
            other => other.into(),
        })?;

    let sql = format!(
        "SELECT DISTINCT {ADDR_COLUMNS} FROM occurrence o
         JOIN symbol s ON s.id = o.symbol_id LEFT JOIN file f ON f.id = s.file_id
         WHERE o.word_id = ?1 ORDER BY s.qname"
    );
    let symbols = store
        .conn
        .prepare(&sql)?
        .query_map([word_id], addr_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut verbs = Vec::new();
    let mut roles = Vec::new();
    let mut neighbors = Vec::new();
    let mut stmt = store.conn.prepare(
        "SELECT w2.text, o2.slot FROM occurrence o1
         JOIN occurrence o2 ON o2.symbol_id = o1.symbol_id AND o2.word_id != o1.word_id
         JOIN word w2 ON w2.id = o2.word_id
         WHERE o1.word_id = ?1
         GROUP BY w2.text, o2.slot
         ORDER BY COUNT(DISTINCT o2.symbol_id) DESC, w2.text",
    )?;
    let rows = stmt.query_map(params![word_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (text, slot) = row?;
        match slot.as_str() {
            "action" => verbs.push(text),
            "role" => roles.push(text),
            _ => neighbors.push(text),
        }
    }
    let layers = store
        .conn
        .prepare(
            "SELECT DISTINCT f.layer FROM occurrence o
             JOIN symbol s ON s.id = o.symbol_id JOIN file f ON f.id = s.file_id
             WHERE o.word_id = ?1 AND f.layer IS NOT NULL ORDER BY f.layer",
        )?
        .query_map([word_id], |r| r.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(WordInfo {
        word,
        verbs,
        roles,
        layers,
        symbols,
        neighbors,
    })
}
