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
    write_methods: &'a [String],
    files: HashMap<i64, FileCtx>,
}

struct RawRef {
    rowid: i64,
    src: i64,
    kind: String,
    text: Option<String>,
    attr: Option<String>,
    pos: Option<i64>,
    receiver: Option<String>,
    line: i64,
}

impl<'a> Resolver<'a> {
    fn new(tx: &'a Transaction<'a>, write_methods: &'a [String]) -> Self {
        Self {
            tx,
            write_methods,
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
    approx: bool,
}

fn insert_edge(tx: &Transaction, e: &NewEdge) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO edge(kind, src, dst, dst_text, line, attr, pos, approx)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            e.kind, e.src, e.dst, e.dst_text, e.line, e.attr, e.pos, e.approx
        ],
    )?;
    Ok(())
}

fn load_refs(tx: &Transaction, file_id: i64) -> Result<Vec<RawRef>, CoreError> {
    let mut stmt = tx.prepare(
        "SELECT rowid, src, kind, callee_text, attr, pos, line, receiver FROM raw_ref
         WHERE file_id = ?1 ORDER BY rowid",
    )?;
    let refs = stmt
        .query_map([file_id], |r| {
            Ok(RawRef {
                rowid: r.get(0)?,
                src: r.get(1)?,
                kind: r.get(2)?,
                text: r.get(3)?,
                attr: r.get(4)?,
                pos: r.get(5)?,
                line: r.get(6)?,
                receiver: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(refs)
}

fn resolve_imports(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
) -> Result<(), CoreError> {
    let module = resolver.file_ctx(file_id)?.module.clone();
    let Some((src, _)) = resolver.symbol(&module)? else {
        return Ok(());
    };
    let mut stmt = tx.prepare(
        "SELECT target, line, type_checking FROM import WHERE file_id = ?1 ORDER BY rowid",
    )?;
    let imports: Vec<(String, i64, i64)> = stmt
        .query_map([file_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (target, line, type_checking) in imports {
        let dst = resolver.symbol(&target)?.map(|(id, _)| id);
        let edge = NewEdge {
            kind: "IMPORTA",
            src,
            dst,
            dst_text: dst.is_none().then_some(target.as_str()),
            line,
            attr: (type_checking != 0).then_some("type_checking"),
            pos: None,
            approx: false,
        };
        insert_edge(tx, &edge)?;
    }
    Ok(())
}

/// Recria as arestas de saída dos arquivos, a partir de `raw_ref` e `import` (sem reparsear).
///
/// Duas fases: tipos, imports e efeitos primeiro; chamadas depois, já com herança e Protocols.
pub fn resolve_files(
    tx: &Transaction,
    file_ids: &[i64],
    write_methods: &[String],
) -> Result<(), CoreError> {
    let mut resolver = Resolver::new(tx, write_methods);
    for &file_id in file_ids {
        tx.execute(
            "DELETE FROM edge WHERE src IN (SELECT id FROM symbol WHERE file_id = ?1)",
            [file_id],
        )?;
        resolve_imports(tx, &mut resolver, file_id)?;
        for raw in load_refs(tx, file_id)?
            .iter()
            .filter(|r| !matches!(r.kind.as_str(), "CHAMA" | "CHAMA_CONSTRUIDA"))
        {
            resolve_ref(tx, &mut resolver, file_id, raw)?;
        }
    }
    resolve_structural(tx, &mut resolver)?;
    for &file_id in file_ids {
        for raw in load_refs(tx, file_id)?
            .iter()
            .filter(|r| matches!(r.kind.as_str(), "CHAMA" | "CHAMA_CONSTRUIDA"))
        {
            resolve_call(tx, &mut resolver, file_id, raw)?;
        }
    }
    Ok(())
}

impl Resolver<'_> {
    fn class_of(&self, symbol_id: i64) -> Result<Option<i64>, CoreError> {
        Ok(self
            .tx
            .query_row(
                "SELECT p.id FROM symbol s JOIN symbol p ON p.id = s.parent_id
                 WHERE s.id = ?1 AND p.kind = 'class'",
                [symbol_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Método da classe ou de uma base do repo (herança explícita).
    fn method_in_class(
        &self,
        class_id: i64,
        name: &str,
        depth: u8,
    ) -> Result<Option<i64>, CoreError> {
        let own: Option<i64> = self
            .tx
            .query_row(
                "SELECT id FROM symbol WHERE parent_id = ?1 AND name = ?2 AND kind = 'method'",
                params![class_id, name],
                |r| r.get(0),
            )
            .optional()?;
        if own.is_some() || depth == 0 {
            return Ok(own);
        }
        let mut stmt = self.tx.prepare(
            "SELECT dst FROM edge WHERE src = ?1 AND kind = 'ESPECIALIZA'
             AND dst IS NOT NULL AND approx = 0 ORDER BY id",
        )?;
        let bases: Vec<i64> = stmt
            .query_map([class_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for base in bases {
            if let Some(found) = self.method_in_class(base, name, depth - 1)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn is_protocol(&self, class_id: i64) -> Result<bool, CoreError> {
        let flag: i64 = self.tx.query_row(
            "SELECT is_protocol FROM symbol WHERE id = ?1",
            [class_id],
            |r| r.get(0),
        )?;
        Ok(flag != 0)
    }

    /// Classes com `ESPECIALIZA` (explícita ou estrutural) para o Protocol, em ordem estável.
    fn implementations(&self, protocol_id: i64) -> Result<Vec<i64>, CoreError> {
        let mut stmt = self.tx.prepare(
            "SELECT DISTINCT e.src FROM edge e JOIN symbol s ON s.id = e.src
             WHERE e.kind = 'ESPECIALIZA' AND e.dst = ?1 ORDER BY s.qname",
        )?;
        let ids = stmt
            .query_map([protocol_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    fn class_name(&self, class_id: i64) -> Result<String, CoreError> {
        let qname: String =
            self.tx
                .query_row("SELECT qname FROM symbol WHERE id = ?1", [class_id], |r| {
                    r.get(0)
                })?;
        Ok(qname.rsplit('.').next().unwrap_or(&qname).to_string())
    }

    /// Classe do tipo anotado de um atributo.
    fn attr_class(&mut self, attr_id: i64) -> Result<Option<i64>, CoreError> {
        let row: (Option<String>, i64) = self.tx.query_row(
            "SELECT type_text, file_id FROM symbol WHERE id = ?1",
            [attr_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        match row {
            (Some(text), file_id) => self.resolve_type(file_id, &text),
            _ => Ok(None),
        }
    }

    fn arity(&self, method_id: i64) -> Result<i64, CoreError> {
        Ok(self.tx.query_row(
            "SELECT COUNT(*) FROM edge WHERE kind = 'RECEBE' AND src = ?1",
            [method_id],
            |r| r.get(0),
        )?)
    }
}

/// `ESPECIALIZA` estrutural (`~`): a classe tem todos os métodos públicos do Protocol, com a
/// mesma aridade, sem herdar dele (spec §4).
fn resolve_structural(tx: &Transaction, resolver: &mut Resolver) -> Result<(), CoreError> {
    tx.execute(
        "DELETE FROM edge WHERE kind = 'ESPECIALIZA' AND approx = 1",
        [],
    )?;
    let mut stmt = tx
        .prepare("SELECT id FROM symbol WHERE kind = 'class' AND is_protocol = 1 ORDER BY qname")?;
    let protocols: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut stmt = tx
        .prepare("SELECT id FROM symbol WHERE kind = 'class' AND is_protocol = 0 ORDER BY qname")?;
    let classes: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for protocol in protocols {
        let mut stmt = tx.prepare(
            "SELECT id, name FROM symbol WHERE parent_id = ?1 AND kind = 'method'
             AND substr(name, 1, 1) != '_' ORDER BY name",
        )?;
        let methods: Vec<(i64, String)> = stmt
            .query_map([protocol], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if methods.is_empty() {
            continue;
        }
        for &class in &classes {
            let explicit: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM edge WHERE kind = 'ESPECIALIZA' AND src = ?1 AND dst = ?2)",
                params![class, protocol],
                |r| r.get(0),
            )?;
            if explicit {
                continue;
            }
            let mut implements = true;
            for (method_id, name) in &methods {
                let found = resolver.method_in_class(class, name, 3)?;
                implements &= match found {
                    Some(m) => resolver.arity(m)? == resolver.arity(*method_id)?,
                    None => false,
                };
            }
            if implements {
                let edge = NewEdge {
                    kind: "ESPECIALIZA",
                    src: class,
                    dst: Some(protocol),
                    dst_text: None,
                    line: 0,
                    attr: None,
                    pos: None,
                    approx: true,
                };
                insert_edge(tx, &edge)?;
            }
        }
    }
    Ok(())
}

struct CallTarget {
    kind: &'static str,
    dst: i64,
}

/// Procura o chamado por escopo do módulo e imports (regras 1, 2 e 5).
fn find_named(
    resolver: &mut Resolver,
    file_id: i64,
    text: &str,
) -> Result<Option<CallTarget>, CoreError> {
    let ctx = resolver.file_ctx(file_id)?;
    let mut candidates = vec![format!("{}.{text}", ctx.module)];
    for (local, target) in &ctx.imports {
        if text == local {
            candidates.push(target.clone());
        } else if let Some(rest) = text.strip_prefix(&format!("{local}.")) {
            candidates.push(format!("{target}.{rest}"));
        }
    }
    for qname in candidates {
        if let Some((dst, kind)) = resolver.symbol(&qname)? {
            match kind.as_str() {
                "function" | "method" => return Ok(Some(CallTarget { kind: "CHAMA", dst })),
                "class" => return Ok(Some(CallTarget { kind: "CRIA", dst })),
                _ => {}
            }
        }
    }
    Ok(None)
}

fn call_edge<'a>(
    raw: &'a RawRef,
    kind: &'a str,
    dst: Option<i64>,
    text: Option<&'a str>,
    approx: bool,
) -> NewEdge<'a> {
    NewEdge {
        kind,
        src: raw.src,
        dst,
        dst_text: text,
        line: raw.line,
        attr: None,
        pos: None,
        approx,
    }
}

fn mark(tx: &Transaction, raw: &RawRef, state: i64) -> Result<(), CoreError> {
    tx.execute(
        "UPDATE raw_ref SET resolved = ?1 WHERE rowid = ?2",
        params![state, raw.rowid],
    )?;
    Ok(())
}

/// Chamada a método de um tipo conhecido: aresta ao método do tipo, `~` a cada implementação
/// se o tipo é Protocol, e `ESCREVE` externo (`~`) para métodos de escrita (spec §5.4, §5.5).
fn call_on_type(
    tx: &Transaction,
    resolver: &mut Resolver,
    raw: &RawRef,
    class_id: i64,
    method: &str,
    receiver_approx: bool,
) -> Result<bool, CoreError> {
    let Some(target) = resolver.method_in_class(class_id, method, 3)? else {
        return Ok(false);
    };
    insert_edge(
        tx,
        &call_edge(raw, "CHAMA", Some(target), None, receiver_approx),
    )?;
    if resolver.is_protocol(class_id)? {
        for implementation in resolver.implementations(class_id)? {
            if let Some(m) = resolver.method_in_class(implementation, method, 3)? {
                insert_edge(tx, &call_edge(raw, "CHAMA", Some(m), None, true))?;
            }
        }
    }
    if resolver.write_methods.iter().any(|w| w == method) {
        let text = format!("{}.{method}", resolver.class_name(class_id)?);
        insert_edge(tx, &call_edge(raw, "ESCREVE", None, Some(&text), true))?;
    }
    Ok(true)
}

/// O método `name` da única classe importada pelo arquivo que o tem (herança incluída). Com duas
/// ou mais candidatas diferentes, ou nenhuma, devolve `None`: o import não decide.
fn imported_method(
    resolver: &mut Resolver,
    file_id: i64,
    name: &str,
) -> Result<Option<i64>, CoreError> {
    let targets: Vec<String> = resolver
        .file_ctx(file_id)?
        .imports
        .iter()
        .map(|(_, target)| target.clone())
        .collect();
    let mut methods = std::collections::BTreeSet::new();
    for target in targets {
        let Some((class_id, kind)) = resolver.symbol(&target)? else {
            continue;
        };
        if kind != "class" {
            continue;
        }
        if let Some(method) = resolver.method_in_class(class_id, name, 3)? {
            methods.insert(method);
        }
    }
    let mut found = methods.into_iter();
    Ok(match (found.next(), found.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    })
}

fn resolve_call(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let text = raw.text.clone().unwrap_or_default();
    let segs: Vec<&str> = text.split('.').collect();
    let src_class = resolver.class_of(raw.src)?;
    let last = segs[segs.len() - 1];

    if raw.kind == "CHAMA_CONSTRUIDA" {
        if let Some(receiver) = raw.receiver.as_deref()
            && let Some(class) = resolver.resolve_type(file_id, receiver)?
            && call_on_type(tx, resolver, raw, class, last, false)?
        {
            return mark(tx, raw, 1);
        }
        // A factory or an external constructor cannot be made exact from its spelling.
        insert_edge(tx, &call_edge(raw, "CHAMA", None, Some(&text), false))?;
        return mark(tx, raw, 2);
    }

    let handled = if matches!(segs[0], "self" | "cls") {
        match (segs.len(), src_class) {
            (2, Some(class)) => match resolver.method_in_class(class, segs[1], 3)? {
                Some(m) => {
                    insert_edge(tx, &call_edge(raw, "CHAMA", Some(m), None, false))?;
                    true
                }
                None => false,
            },
            (3, Some(class)) => match resolver.resolve_path(class, segs[1])? {
                Some(attr) => match resolver.attr_class(attr)? {
                    Some(t) => call_on_type(tx, resolver, raw, t, segs[2], false)?,
                    None => false,
                },
                None => false,
            },
            _ => false,
        }
    } else if let Some(found) = find_named(resolver, file_id, &text)? {
        insert_edge(
            tx,
            &call_edge(raw, found.kind, Some(found.dst), None, false),
        )?;
        true
    } else if let (Some(receiver), true) = (raw.receiver.as_deref(), segs.len() == 2) {
        match resolver.resolve_type(file_id, receiver)? {
            Some(class) => call_on_type(tx, resolver, raw, class, last, true)?,
            None => false,
        }
    } else {
        false
    };
    if handled {
        return mark(tx, raw, 1);
    }

    // Evidência de import (TASK-2, D4): o arquivo importa uma única classe que tem o método.
    if segs.len() >= 2
        && let Some(method) = imported_method(resolver, file_id, last)?
    {
        insert_edge(tx, &call_edge(raw, "CHAMA", Some(method), None, true))?;
        return mark(tx, raw, 1);
    }

    // Regra 7: só o nome do método.
    let mut stmt = tx.prepare(
        "SELECT id FROM symbol WHERE name = ?1 AND kind IN ('function', 'method') ORDER BY qname",
    )?;
    let named: Vec<i64> = stmt
        .query_map([last], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    match named.as_slice() {
        [only] => {
            insert_edge(tx, &call_edge(raw, "CHAMA", Some(*only), None, true))?;
            mark(tx, raw, 1)
        }
        [] => {
            insert_edge(tx, &call_edge(raw, "CHAMA", None, Some(&text), false))?;
            mark(tx, raw, 2)
        }
        _ => mark(tx, raw, 0),
    }
}

/// Leitura `X.attr` (TASK-7, D5 e D5a): `X` (ou o tipo local do receptor) resolve para uma
/// classe com o atributo → `LE` exato; não resolve → `LE` aproximado só se um único atributo do
/// repositório tem esse nome; nenhum ou vários → nada (estado 2, como referência externa).
fn resolve_attr_read(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let Some((owner, attr)) = raw.text.as_deref().and_then(|t| t.rsplit_once('.')) else {
        return mark(tx, raw, 2);
    };
    let class = match raw.receiver.as_deref() {
        Some(receiver) => resolver.resolve_type(file_id, receiver)?,
        None => None,
    };
    let class = match class {
        Some(c) => Some(c),
        None => resolver.resolve_type(file_id, owner)?,
    };
    let edge = |dst: i64, approx: bool| NewEdge {
        kind: "LE",
        src: raw.src,
        dst: Some(dst),
        dst_text: None,
        line: raw.line,
        attr: None,
        pos: None,
        approx,
    };
    if let Some(class) = class {
        return match resolver.resolve_path(class, attr)? {
            Some(dst) => {
                insert_edge(tx, &edge(dst, false))?;
                mark(tx, raw, 1)
            }
            None => mark(tx, raw, 2),
        };
    }
    let mut stmt =
        tx.prepare("SELECT id FROM symbol WHERE kind = 'attribute' AND name = ?1 ORDER BY qname")?;
    let candidates: Vec<i64> = stmt
        .query_map([attr], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    // Nome comum (`id`, `status`) em várias classes viraria ruído: só o nome único conta.
    if let [only] = candidates.as_slice() {
        insert_edge(tx, &edge(*only, true))?;
        return mark(tx, raw, 1);
    }
    mark(tx, raw, 2)
}

fn resolve_attr_write(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let Some((owner, name)) = raw.text.as_deref().and_then(|t| t.rsplit_once('.')) else {
        return mark(tx, raw, 2);
    };
    let class = resolver.resolve_type(file_id, raw.receiver.as_deref().unwrap_or(owner))?;
    let Some(dst) = class
        .map(|c| resolver.resolve_path(c, name))
        .transpose()?
        .flatten()
    else {
        return mark(tx, raw, 2);
    };
    let origin = match raw.kind.as_str() {
        "INICIALIZA" => "inicializacao",
        "SQL_UPDATE" => "sql:update",
        "SQL_INSERT" => "sql:insert",
        _ => "atribuicao",
    };
    insert_edge(
        tx,
        &NewEdge {
            kind: "ESCREVE",
            src: raw.src,
            dst: Some(dst),
            dst_text: None,
            line: raw.line,
            attr: Some(origin),
            pos: None,
            approx: false,
        },
    )?;
    mark(tx, raw, 1)
}

/// Nome simples que é variável do próprio módulo ou importada de outro: aresta `LE`/`ESCREVE`
/// exata ao símbolo `Attribute`. Qualquer outro nome (função, classe, externo) não gera aresta
/// nem conta como não resolvido (TASK-10).
fn resolve_variable(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let Some(text) = raw.text.as_deref() else {
        return mark(tx, raw, 2);
    };
    let ctx = resolver.file_ctx(file_id)?;
    let write = raw.kind == "ESCREVE_VAR";
    let mut candidates = vec![format!("{}.{text}", ctx.module)];
    // `global X` religa o nome do próprio módulo, nunca a variável de onde `X` foi importado.
    if !write {
        for (local, target) in &ctx.imports {
            if text == local {
                candidates.push(target.clone());
            }
        }
    }
    for qname in candidates {
        if let Some((dst, kind)) = resolver.symbol(&qname)?
            && kind == "attribute"
        {
            insert_edge(
                tx,
                &NewEdge {
                    kind: if write { "ESCREVE" } else { "LE" },
                    src: raw.src,
                    dst: Some(dst),
                    dst_text: None,
                    line: raw.line,
                    attr: write.then_some("atribuicao"),
                    pos: None,
                    approx: false,
                },
            )?;
            return mark(tx, raw, 1);
        }
    }
    mark(tx, raw, 2)
}

fn resolve_ref(
    tx: &Transaction,
    resolver: &mut Resolver,
    file_id: i64,
    raw: &RawRef,
) -> Result<(), CoreError> {
    let text = raw.text.as_deref();
    match raw.kind.as_str() {
        "ESCREVE_ATTR" | "INICIALIZA" | "SQL_UPDATE" | "SQL_INSERT" => {
            resolve_attr_write(tx, resolver, file_id, raw)?;
        }
        // Resolvidas numa passada global (`resolve_requests`): a rota pode estar em outro arquivo.
        "REQUISITA" => {}
        "LE_ATTR" => resolve_attr_read(tx, resolver, file_id, raw)?,
        "LE_VAR" | "ESCREVE_VAR" => resolve_variable(tx, resolver, file_id, raw)?,
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
                    approx: false,
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
                approx: false,
            };
            insert_edge(tx, &edge)?;
        }
    }
    Ok(())
}

/// Refaz todas as arestas `REQUISITA`: cada requisição HTTP literal liga ao endpoint cuja rota
/// casa (método e caminho, `{}` casa um segmento). Uma só rota → exata; várias → `~` em cada.
/// Global porque uma rota nova num arquivo muda o destino de requisições em outros arquivos.
pub fn resolve_requests(tx: &Transaction) -> Result<(), CoreError> {
    tx.execute("DELETE FROM edge WHERE kind = 'REQUISITA'", [])?;
    let routes: Vec<(i64, String, String)> = tx
        .prepare("SELECT id, route FROM symbol WHERE route IS NOT NULL ORDER BY id")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|row| {
            row.map(|(id, route)| {
                route
                    .split_once(' ')
                    .map(|(m, p)| (id, m.to_string(), p.to_string()))
            })
            .transpose()
        })
        .collect::<Result<_, _>>()?;
    let requests: Vec<(i64, i64, String, Option<String>, i64)> = tx
        .prepare(
            "SELECT rowid, src, callee_text, receiver, line FROM raw_ref
             WHERE kind = 'REQUISITA' ORDER BY rowid",
        )?
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    for (rowid, src, path, method, line) in requests {
        let matches: Vec<i64> = routes
            .iter()
            .filter(|(_, m, p)| method.as_deref() == Some(m.as_str()) && path_matches(p, &path))
            .map(|(id, _, _)| *id)
            .collect();
        for &dst in &matches {
            let edge = NewEdge {
                kind: "REQUISITA",
                src,
                dst: Some(dst),
                dst_text: None,
                line,
                attr: Some(&path),
                pos: None,
                approx: matches.len() > 1,
            };
            insert_edge(tx, &edge)?;
        }
        let state = if matches.is_empty() { 2 } else { 1 };
        tx.execute(
            "UPDATE raw_ref SET resolved = ?1 WHERE rowid = ?2",
            params![state, rowid],
        )?;
    }
    Ok(())
}

/// Rota e requisição normalizadas casam segmento a segmento; `{}` em qualquer lado casa tudo.
fn path_matches(route: &str, request: &str) -> bool {
    let (a, b): (Vec<&str>, Vec<&str>) = (route.split('/').collect(), request.split('/').collect());
    a.len() == b.len()
        && a.iter()
            .zip(&b)
            .all(|(x, y)| x == y || *x == "{}" || *y == "{}")
}
