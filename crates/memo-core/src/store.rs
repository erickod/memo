//! Persistência do grafo em SQLite (spec §4).

use crate::error::CoreError;
use rusqlite::{Connection, OptionalExtension};
use std::fmt::Write;
use std::path::Path;

const SCHEMA_VERSION: &str = "4";

const SCHEMA: &str = "
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE file(
  id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL, hash TEXT NOT NULL,
  mtime_ns INTEGER, size INTEGER, context TEXT, layer TEXT, parse_error TEXT);
CREATE TABLE symbol(
  id INTEGER PRIMARY KEY, qname TEXT UNIQUE NOT NULL, name TEXT NOT NULL,
  kind TEXT NOT NULL,
  nivel INTEGER NOT NULL,
  parent_id INTEGER REFERENCES symbol(id), file_id INTEGER REFERENCES file(id),
  line_start INTEGER, line_end INTEGER,
  sig_hash TEXT, body_hash TEXT, decorators TEXT,
  is_test INTEGER DEFAULT 0, is_entrypoint INTEGER DEFAULT 0, is_protocol INTEGER DEFAULT 0,
  type_text TEXT);
CREATE TABLE word(id INTEGER PRIMARY KEY, text TEXT UNIQUE NOT NULL);
CREATE TABLE occurrence(
  symbol_id INTEGER, word_id INTEGER NOT NULL, pos INTEGER NOT NULL,
  slot TEXT NOT NULL,
  segment TEXT,
  owner_kind TEXT NOT NULL DEFAULT 'symbol',
  owner_id INTEGER NOT NULL,
  source TEXT NOT NULL DEFAULT 'name',
  PRIMARY KEY(owner_kind, owner_id, source, pos));
CREATE TABLE doc(symbol_id INTEGER PRIMARY KEY, text TEXT NOT NULL);
CREATE TABLE edge(
  id INTEGER PRIMARY KEY, kind TEXT NOT NULL,
  src INTEGER NOT NULL REFERENCES symbol(id), dst INTEGER REFERENCES symbol(id),
  dst_text TEXT,
  line INTEGER, attr TEXT, pos INTEGER,
  approx INTEGER DEFAULT 0, count INTEGER DEFAULT 1);
CREATE TABLE raw_ref(id INTEGER PRIMARY KEY, file_id INTEGER, src INTEGER, kind TEXT, callee_text TEXT, receiver TEXT, line INTEGER,
  attr TEXT, pos INTEGER, resolved INTEGER DEFAULT 1);
CREATE TABLE import(file_id INTEGER, local_name TEXT, target TEXT, line INTEGER, type_checking INTEGER DEFAULT 0);
CREATE TABLE word_stats(
  word_id INTEGER PRIMARY KEY, uso INTEGER, alcance INTEGER, grau INTEGER,
  idf REAL, especificidade REAL, centralidade REAL, esqueleto INTEGER, coerencia REAL);
CREATE TABLE event(id INTEGER PRIMARY KEY, batch TEXT NOT NULL, seq INTEGER, kind TEXT NOT NULL, subject TEXT, payload TEXT);
CREATE TABLE session_seen(task TEXT, cmd TEXT, key TEXT, PRIMARY KEY(task, cmd, key));
CREATE INDEX edge_src_kind ON edge(src, kind);
CREATE INDEX edge_dst_kind ON edge(dst, kind);
CREATE INDEX occurrence_word ON occurrence(word_id);
CREATE INDEX occurrence_symbol ON occurrence(symbol_id);
CREATE INDEX symbol_file ON symbol(file_id);
CREATE INDEX symbol_parent ON symbol(parent_id);
CREATE INDEX raw_ref_file ON raw_ref(file_id);
CREATE INDEX import_file ON import(file_id);
";

/// Fato imutável gravado na tabela `event` (spec §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    pub batch: String,
    pub seq: i64,
    pub kind: String,
    pub subject: Option<String>,
    pub payload: Option<String>,
}

pub struct SqliteStore {
    pub(crate) conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Abre o banco; um banco derivado com outra `schema_version` é descartado e recriado
    /// (o grafo se reconstrói do código; as notas ficam em arquivos e não são tocadas).
    fn init(conn: Connection) -> Result<Self, CoreError> {
        let has_meta: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
            [],
            |r| r.get(0),
        )?;
        let current: Option<String> = if has_meta {
            conn.query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()?
        } else {
            None
        };
        if current.as_deref() == Some(SCHEMA_VERSION) {
            return Ok(Self { conn });
        }
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        // As chaves estrangeiras impedem apagar `file` antes de `symbol`: desligar durante a troca.
        conn.execute_batch("PRAGMA foreign_keys = OFF")?;
        for table in tables {
            conn.execute_batch(&format!("DROP TABLE \"{table}\""))?;
        }
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )?;
        Ok(Self { conn })
    }

    /// Total de linhas alteradas na conexão; não muda quando nada é escrito.
    pub fn total_changes(&self) -> u64 {
        self.conn.total_changes()
    }

    pub fn event_count(&self) -> Result<u64, CoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM event", [], |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    pub fn symbol_exists(&self, qname: &str) -> Result<bool, CoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM symbol WHERE qname = ?1)",
            [qname],
            |r| r.get(0),
        )?)
    }

    /// Grava uma chave de `meta`; não escreve se o valor já é o mesmo.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), CoreError> {
        if self.get_meta(key)?.as_deref() == Some(value) {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, CoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// Copia o banco inteiro para `path` pela API de backup do SQLite (spec §6.4).
    pub fn backup_to(&self, path: &Path) -> Result<(), CoreError> {
        let mut target = Connection::open(path)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut target)?;
        backup.run_to_completion(64, std::time::Duration::ZERO, None)?;
        Ok(())
    }

    /// `(arquivos, símbolos)` do grafo.
    pub fn counts(&self) -> Result<(i64, i64), CoreError> {
        let count = |sql: &str| -> Result<i64, CoreError> {
            Ok(self.conn.query_row(sql, [], |r| r.get(0))?)
        };
        Ok((
            count("SELECT COUNT(*) FROM file")?,
            count("SELECT COUNT(*) FROM symbol")?,
        ))
    }

    /// Marca todos os arquivos como desatualizados, para reextrair na próxima indexação.
    pub fn invalidate_files(&self) -> Result<(), CoreError> {
        self.conn
            .execute("UPDATE file SET hash = '', mtime_ns = -1, size = -1", [])?;
        Ok(())
    }

    /// Todos os eventos, na ordem em que foram gravados.
    pub fn events(&self) -> Result<Vec<EventRow>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT batch, seq, kind, subject, payload FROM event ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(EventRow {
                batch: r.get(0)?,
                seq: r.get(1)?,
                kind: r.get(2)?,
                subject: r.get(3)?,
                payload: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Arquivos que não parsearam, em ordem de caminho (spec §2, regra 7).
    pub fn files_with_error(&self) -> Result<Vec<String>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM file WHERE parse_error IS NOT NULL ORDER BY path")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Texto canônico do grafo (sem ids nem mtime), usado para comparar bytes entre execuções.
    pub fn dump(&self) -> Result<String, CoreError> {
        let mut out = String::new();
        let mut files = self.conn.prepare(
            "SELECT path, hash, COALESCE(context,''), COALESCE(layer,''), COALESCE(parse_error,'')
             FROM file ORDER BY path",
        )?;
        let rows = files.query_map([], |r| {
            Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?])
        })?;
        for row in rows {
            let [path, hash, ctx, layer, err]: [String; 5] = row?;
            let _ = writeln!(out, "file|{path}|{hash}|{ctx}|{layer}|{err}");
        }
        let mut symbols = self.conn.prepare(
            "SELECT s.id, s.qname, s.kind, s.nivel, COALESCE(f.path,''), s.line_start, s.line_end,
                    COALESCE(s.sig_hash,''), COALESCE(s.body_hash,''), COALESCE(p.qname,''), s.is_test
             FROM symbol s LEFT JOIN file f ON f.id = s.file_id LEFT JOIN symbol p ON p.id = s.parent_id
             ORDER BY s.qname",
        )?;
        let mut words = self.conn.prepare(
            "SELECT w.text, o.slot, COALESCE(o.segment,'') FROM occurrence o
             JOIN word w ON w.id = o.word_id WHERE o.symbol_id = ?1 ORDER BY o.pos",
        )?;
        let mut rows = symbols.query([])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let _ = write!(
                out,
                "symbol|{}|{}|{}|{}|{}-{}|{}|{}|{}|{}|",
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, i64>(10)?,
            );
            let occ = words.query_map([id], |w| {
                Ok(format!(
                    "{}:{}:{}",
                    w.get::<_, String>(0)?,
                    w.get::<_, String>(1)?,
                    w.get::<_, String>(2)?
                ))
            })?;
            let occ: Vec<String> = occ.collect::<Result<_, _>>()?;
            let _ = writeln!(out, "{}", occ.join(","));
        }
        for e in self.events()? {
            let _ = writeln!(
                out,
                "event|{}|{}|{}|{}|{}",
                e.batch,
                e.seq,
                e.kind,
                e.subject.unwrap_or_default(),
                e.payload.unwrap_or_default()
            );
        }
        let mut owned = self.conn.prepare(
            "SELECT o.owner_kind, o.source, w.text, o.slot, COALESCE(o.segment, ''), COUNT(*)
             FROM occurrence o JOIN word w ON w.id = o.word_id
             WHERE o.source != 'name'
             GROUP BY o.owner_kind, o.source, w.text, o.slot, COALESCE(o.segment, '')
             ORDER BY o.owner_kind, o.source, w.text, o.slot, COALESCE(o.segment, '')",
        )?;
        let mut rows = owned.query([])?;
        while let Some(r) = rows.next()? {
            let _ = writeln!(
                out,
                "occ|{}|{}|{}|{}|{}|{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            );
        }
        let mut stats = self.conn.prepare(
            "SELECT w.text, ws.uso, ws.alcance, ws.grau, ws.idf, ws.especificidade, ws.centralidade, ws.esqueleto, COALESCE(ws.coerencia, 0.0)
             FROM word_stats ws JOIN word w ON w.id = ws.word_id ORDER BY w.text",
        )?;
        let mut rows = stats.query([])?;
        while let Some(r) = rows.next()? {
            let _ = writeln!(
                out,
                "stats|{}|{}|{}|{}|{:.6}|{:.6}|{:.6}|{}|{:.6}",
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, f64>(8)?,
            );
        }
        let mut edges = self.conn.prepare(
            "SELECT e.kind, s.qname, COALESCE(d.qname, ''), COALESCE(e.dst_text, ''),
                    COALESCE(e.line, 0), COALESCE(e.attr, ''), e.approx
             FROM edge e JOIN symbol s ON s.id = e.src LEFT JOIN symbol d ON d.id = e.dst
             ORDER BY e.kind, s.qname, COALESCE(d.qname, ''), COALESCE(e.dst_text, ''), e.line, e.attr",
        )?;
        let mut rows = edges.query([])?;
        while let Some(r) = rows.next()? {
            let _ = writeln!(
                out,
                "edge|{}|{}|{}|{}|{}|{}|{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
            );
        }
        Ok(out)
    }
}
