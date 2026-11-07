//! Resolução de referências brutas em arestas (spec §4 e §5.4).

use crate::error::CoreError;
use crate::index::module_of;
use rusqlite::{OptionalExtension, Transaction, params};
use std::collections::HashMap;

struct FileCtx {
    module: String,
    imports: Vec<(String, String)>,
}

struct Resolver<'a> {
    tx: &'a Transaction<'a>,
    files: HashMap<i64, FileCtx>,
}

struct RawRef {
    rowid: i64,
    src: i64,
    kind: String,
    text: Option<String>,
    attr: Option<String>,
    pos: Option<i64>,
    line: i64,
}

impl<'a> Resolver<'a> {
    fn new(tx: &'a Transaction<'a>) -> Self {
        Self {
            tx,
            files: HashMap::new(),
        }
    }

    fn file_ctx(&mut self, file_id: i64) -> Result<&FileCtx, CoreError> {
        if !self.files.contains_key(&file_id) {
            let path: String =
                self.tx
                    .query_row("SELECT path FROM file WHERE id = ?1", [file_id], |r| {
                        r.get(0)
                    })?;
            let mut stmt = self.tx.prepare(
                "SELECT local_name, target FROM import WHERE file_id = ?1 ORDER BY rowid",
            )?;
            let imports = stmt
                .query_map([file_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            self.files.insert(
                file_id,
                FileCtx {
                    module: module_of(&path),
                    imports,
                },
            );
        }
        Ok(&self.files[&file_id])
    }

    fn symbol(&self, qname: &str) -> Result<Option<(i64, String)>, CoreError> {
        Ok(self
            .tx
            .query_row(
                "SELECT id, kind FROM symbol WHERE qname = ?1",
                [qname],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// Resolve um nome de tipo, no contexto de um arquivo, para uma classe do repo.
    fn resolve_type(&mut self, file_id: i64, text: &str) -> Result<Option<i64>, CoreError> {
        let text = text.trim_matches(|c| c == '\'' || c == '"');
        let ctx = self.file_ctx(file_id)?;
        let mut candidates: Vec<String> = Vec::new();
        for (local, target) in &ctx.imports {
            if text == local {
                candidates.push(target.clone());
            } else if let Some(rest) = text.strip_prefix(&format!("{local}.")) {
                candidates.push(format!("{target}.{rest}"));
            }
        }
        candidates.push(format!("{}.{text}", ctx.module));
        candidates.push(text.to_string());
        for qname in candidates {
            if let Some((id, kind)) = self.symbol(&qname)?
                && kind == "class"
            {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Segue `a.b.c` a partir de uma classe: atributos e seus tipos anotados.
    fn resolve_path(&mut self, class_id: i64, path: &str) -> Result<Option<i64>, CoreError> {
        let names: Vec<&str> = path.split('.').collect();
        let mut current = class_id;
        for (i, name) in names.iter().enumerate() {
            let row: Option<(i64, Option<String>, i64)> = self
                .tx
                .query_row(
                    "SELECT id, type_text, file_id FROM symbol
                     WHERE parent_id = ?1 AND name = ?2 AND kind = 'attribute'",
                    params![current, name],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((id, type_text, file_id)) = row else {
                return Ok(None);
            };
            if i + 1 == names.len() {
                return Ok(Some(id));
            }
            let Some(next) = type_text
                .map(|t| self.resolve_type(file_id, &t))
                .transpose()?
                .flatten()
            else {
                return Ok(None);
            };
            current = next;
        }
        Ok(None)
    }
}

struct NewEdge<'a> {
    kind: &'a str,
    src: i64,
    dst: Option<i64>,
    dst_text: Option<&'a str>,
    line: i64,
    attr: Option<&'a str>,
    pos: Option<i64>,
}

fn insert_edge(tx: &Transaction, e: &NewEdge) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO edge(kind, src, dst, dst_text, line, attr, pos, approx)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
        params![e.kind, e.src, e.dst, e.dst_text, e.line, e.attr, e.pos],
    )?;
    Ok(())
}

/// Recria as arestas de saída dos arquivos, a partir de `raw_ref` e `import` (sem reparsear).
pub fn resolve_files(tx: &Transaction, file_ids: &[i64]) -> Result<(), CoreError> {
    let mut resolver = Resolver::new(tx);
    for &file_id in file_ids {
        tx.execute(
            "DELETE FROM edge WHERE src IN (SELECT id FROM symbol WHERE file_id = ?1)",
            [file_id],
        )?;
        let module = resolver.file_ctx(file_id)?.module.clone();
        let module_id = resolver.symbol(&module)?.map(|(id, _)| id);

        if let Some(src) = module_id {
            let mut stmt = tx.prepare(
                "SELECT target, line, type_checking FROM import WHERE file_id = ?1 ORDER BY rowid",
            )?;
            let imports: Vec<(String, i64, i64)> = stmt
                .query_map([file_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<_, _>>()?;
            for (target, line, type_checking) in imports {
                let dst = resolver.symbol(&target)?.map(|(id, _)| id);
                let text = dst.is_none().then_some(target.as_str());
                let attr = (type_checking != 0).then_some("type_checking");
                let edge = NewEdge {
                    kind: "IMPORTA",
                    src,
                    dst,
                    dst_text: text,
                    line,
                    attr,
                    pos: None,
                };
                insert_edge(tx, &edge)?;
            }
        }

        let mut stmt = tx.prepare(
            "SELECT rowid, src, kind, callee_text, attr, pos, line FROM raw_ref
             WHERE file_id = ?1 ORDER BY rowid",
        )?;
        let refs: Vec<RawRef> = stmt
            .query_map([file_id], |r| {
                Ok(RawRef {
                    rowid: r.get(0)?,
                    src: r.get(1)?,
                    kind: r.get(2)?,
                    text: r.get(3)?,
                    attr: r.get(4)?,
                    pos: r.get(5)?,
                    line: r.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        for raw in refs {
            resolve_ref(tx, &mut resolver, file_id, &raw)?;
        }
    }
    Ok(())
}

fn resolve_ref(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let text = raw.text.as_deref();
    match raw.kind.as_str() {
        "ESCREVE" | "LE" => {
            let class_id: Option<i64> = tx.query_row(
                "SELECT parent_id FROM symbol WHERE id = ?1",
                [raw.src],
                |r| r.get(0),
            )?;
            let dst = match (class_id, text) {
                (Some(class), Some(path)) => resolver.resolve_path(class, path)?,
                _ => None,
            };
            let state = match (dst, raw.kind.as_str()) {
                (Some(_), _) => 1,
                (None, "ESCREVE") => 0,
                (None, _) => 2,
            };
            tx.execute(
                "UPDATE raw_ref SET resolved = ?1 WHERE rowid = ?2",
                params![state, raw.rowid],
            )?;
            if dst.is_some() || raw.kind == "ESCREVE" {
                let dst_text = dst
                    .is_none()
                    .then(|| format!("self.{}", text.unwrap_or("")));
                let edge = NewEdge {
                    kind: &raw.kind,
                    src: raw.src,
                    dst,
                    dst_text: dst_text.as_deref(),
                    line: raw.line,
                    attr: None,
                    pos: None,
                };
                insert_edge(tx, &edge)?;
            }
        }
        _ => {
            let dst = match text {
                Some(t) => resolver.resolve_type(file_id, t)?,
                None => None,
            };
            let dst_text = if dst.is_none() { text } else { None };
            let edge = NewEdge {
                kind: &raw.kind,
                src: raw.src,
                dst,
                dst_text,
                line: raw.line,
                attr: raw.attr.as_deref(),
                pos: raw.pos,
            };
            insert_edge(tx, &edge)?;
        }
    }
    Ok(())
}
