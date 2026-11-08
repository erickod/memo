//! Consultas sobre o grafo (spec §8.2).

use crate::error::CoreError;
use crate::index::module_of;
use crate::store::SqliteStore;
use rusqlite::{OptionalExtension, params};
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
    /// Notas visíveis ancoradas no símbolo; preenchidas por quem conhece o diretório de notas.
    pub notes: Vec<crate::notes::Note>,
}

/// Símbolo com as pontes que o ligam à palavra consultada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaggedSymbol {
    #[serde(flatten)]
    pub addr: SymbolAddr,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordInfo {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
    pub symbols: Vec<TaggedSymbol>,
    pub neighbors: Vec<String>,
    /// Vizinhas da palavra por tipo de ponte (a ficha do conceito).
    pub bridges: Vec<crate::bridges::Bridge>,
    /// Palavras vizinhas marcadas `esqueleto`, fora de verbos, papéis e vizinhas.
    pub ignored: Vec<String>,
    pub stats: Option<WordStats>,
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
    /// O outro extremo é um teste ou um símbolo de arquivo de teste.
    pub other_is_test: bool,
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
    /// Itens de teste escondidos pela flag `--no-tests` (preenchido pelo CLI).
    pub testes_ocultos: i64,
    /// Arquivos de nota que não parseiam (preenchido por quem conhece o diretório de notas).
    pub notas_com_erro: i64,
}

const ADDR_COLUMNS: &str =
    "s.qname, s.kind, s.is_test, COALESCE(f.path, ''), s.line_start, s.line_end";

/// Nome relativo ao módulo do arquivo (`Classe.metodo`); sem arquivo, o último segmento.
pub(crate) fn short_name(qname: &str, path: &str) -> String {
    let module = module_of(path);
    qname
        .strip_prefix(&format!("{module}."))
        .unwrap_or_else(|| qname.rsplit('.').next().unwrap_or(qname))
        .to_string()
}

impl SymbolAddr {
    /// Teste, ou símbolo de um arquivo de teste (como o módulo dele).
    pub fn in_tests(&self) -> bool {
        self.is_test || crate::index::is_test_file(&self.path)
    }
}

fn addr_from_row(r: &rusqlite::Row) -> rusqlite::Result<SymbolAddr> {
    let qname: String = r.get(0)?;
    let path: String = r.get(3)?;
    let short = short_name(&qname, &path);
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

/// `qname` completo ou sufixo único; sufixo ambíguo devolve `Ambiguous`, ausente `NotFound`.
pub(crate) fn resolve_qname(store: &SqliteStore, target: &str) -> Result<String, CoreError> {
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
    match found.len() {
        0 => Err(CoreError::NotFound(target.to_string())),
        1 => Ok(found.remove(0)),
        _ => Err(CoreError::Ambiguous {
            target: target.to_string(),
            candidates: found,
        }),
    }
}

pub(crate) fn addr_by_id(store: &SqliteStore, id: i64) -> Result<SymbolAddr, CoreError> {
    let sql = format!(
        "SELECT {ADDR_COLUMNS} FROM symbol s LEFT JOIN file f ON f.id = s.file_id WHERE s.id = ?1"
    );
    Ok(store.conn.query_row(&sql, [id], addr_from_row)?)
}

fn addr_of(store: &SqliteStore, qname: &str) -> Result<SymbolAddr, CoreError> {
    let sql = format!(
        "SELECT {ADDR_COLUMNS} FROM symbol s LEFT JOIN file f ON f.id = s.file_id WHERE s.qname = ?1"
    );
    Ok(store.conn.query_row(&sql, [qname], addr_from_row)?)
}

/// Aceita `qname` completo ou sufixo único; sufixo ambíguo devolve `Ambiguous`.
pub fn symbol_info(store: &SqliteStore, target: &str) -> Result<SymbolInfo, CoreError> {
    let qname = resolve_qname(store, target)?;
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, COALESCE(s.sig_hash, '') FROM symbol s
         LEFT JOIN file f ON f.id = s.file_id WHERE s.qname = ?1"
    );
    let mut info = store.conn.query_row(&sql, [&qname], |r| {
        Ok(SymbolInfo {
            addr: addr_from_row(r)?,
            sig_hash: r.get(6)?,
            edges: Vec::new(),
            notes: Vec::new(),
        })
    })?;
    info.edges = direct_edges(store, &qname)?;
    Ok(info)
}

/// Arestas diretas, de saída e de entrada, em ordem estável.
fn direct_edges(store: &SqliteStore, qname: &str) -> Result<Vec<EdgeView>, CoreError> {
    let mut out = Vec::new();
    for (dir, near, far) in [("out", "src", "dst"), ("in", "dst", "src")] {
        let sql = format!(
            "SELECT e.kind, e.dst_text, e.attr, e.line, e.approx, o.qname, COALESCE(of.path, ''),
                    COALESCE(o.is_test, 0)
             FROM edge e JOIN symbol me ON me.id = e.{near}
             LEFT JOIN symbol o ON o.id = e.{far} LEFT JOIN file of ON of.id = o.file_id
             WHERE me.qname = ?1
             ORDER BY e.kind, COALESCE(o.qname, e.dst_text, ''), e.line"
        );
        let mut stmt = store.conn.prepare(&sql)?;
        let rows = stmt.query_map([qname], |r| {
            let other_q: Option<String> = r.get(5)?;
            let path: String = r.get(6)?;
            let dst_text: Option<String> = r.get(1)?;
            let tested: i64 = r.get(7)?;
            let other_in_test_file = !path.is_empty() && crate::index::is_test_file(&path);
            Ok(EdgeView {
                dir: dir.to_string(),
                kind: r.get(0)?,
                other: other_q
                    .map(|q| short_name(&q, &path))
                    .or(dst_text)
                    .unwrap_or_default(),
                attr: r.get(2)?,
                line: r.get(3)?,
                approx: r.get::<_, i64>(4)? != 0,
                other_is_test: tested != 0 || other_in_test_file,
            })
        })?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

fn related(store: &SqliteStore, qname: &str, edge_kind: &str) -> Result<Vec<RelItem>, CoreError> {
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, e.line, e.approx, e.attr FROM edge e
         JOIN symbol s ON s.id = e.src LEFT JOIN file f ON f.id = s.file_id
         JOIN symbol t ON t.id = e.dst
         WHERE t.qname = ?1 AND e.kind = ?2 ORDER BY s.qname, e.line"
    );
    let mut stmt = store.conn.prepare(&sql)?;
    let rows = stmt.query_map(params![qname, edge_kind], |r| {
        Ok(RelItem {
            addr: addr_from_row(r)?,
            line: r.get(6)?,
            approx: r.get::<_, i64>(7)? != 0,
            attr: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Símbolos que escrevem o atributo (`ESCREVE`).
pub fn writers(store: &SqliteStore, target: &str) -> Result<Relation, CoreError> {
    relation(store, target, "ESCREVE")
}

/// Símbolos que leem o atributo (`LE`).
pub fn readers(store: &SqliteStore, target: &str) -> Result<Relation, CoreError> {
    relation(store, target, "LE")
}

fn relation(store: &SqliteStore, target: &str, kind: &str) -> Result<Relation, CoreError> {
    let qname = resolve_qname(store, target)?;
    Ok(Relation {
        target: addr_of(store, &qname)?,
        items: related(store, &qname, kind)?,
    })
}

/// Quem produz (`RETORNA`) e quem consome (`RECEBE`) o tipo.
pub fn type_info(store: &SqliteStore, target: &str) -> Result<TypeInfo, CoreError> {
    let qname = resolve_qname(store, target)?;
    Ok(TypeInfo {
        target: addr_of(store, &qname)?,
        producers: related(store, &qname, "RETORNA")?,
        consumers: related(store, &qname, "RECEBE")?,
    })
}

/// Resolvidas e aproximadas vêm das arestas; não resolvidas, das referências sem destino no repo.
pub fn coverage(store: &SqliteStore) -> Result<Coverage, CoreError> {
    let count =
        |sql: &str| -> Result<i64, CoreError> { Ok(store.conn.query_row(sql, [], |r| r.get(0))?) };
    Ok(Coverage {
        resolvidas: count("SELECT COUNT(*) FROM edge WHERE dst IS NOT NULL AND approx = 0")?,
        aproximadas: count("SELECT COUNT(*) FROM edge WHERE approx = 1")?,
        nao_resolvidas: count("SELECT COUNT(*) FROM raw_ref WHERE resolved = 0")?,
        arquivos_com_erro: count("SELECT COUNT(*) FROM file WHERE parse_error IS NOT NULL")?,
        notas_com_erro: 0,
        testes_ocultos: 0,
    })
}

/// Símbolos da ficha: os que têm a palavra no nome e os que as outras pontes trazem (docstring de
/// teste com a palavra, símbolo testado por teste com ela e módulo cujo arquivo a importa).
fn tagged_symbols(
    store: &SqliteStore,
    word_id: i64,
    by_name: Vec<SymbolAddr>,
) -> Result<Vec<TaggedSymbol>, CoreError> {
    let mut ids: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    let extra = [
        "SELECT owner_id FROM occurrence WHERE word_id = ?1 AND owner_kind = 'symbol' AND source = 'test_doc'",
        "SELECT e.dst FROM edge e JOIN symbol t ON t.id = e.src
         JOIN occurrence o ON o.owner_kind = 'symbol' AND o.owner_id = t.id AND o.source IN ('name', 'test_doc')
         WHERE o.word_id = ?1 AND t.is_test = 1 AND e.kind IN ('CHAMA', 'CRIA') AND e.approx = 0 AND e.dst IS NOT NULL",
        "SELECT s.id FROM occurrence o JOIN import i ON o.owner_kind = 'import' AND o.owner_id = i.rowid
         JOIN symbol s ON s.file_id = i.file_id AND s.kind = 'module' WHERE o.word_id = ?1",
    ];
    for sql in extra {
        let found: Vec<i64> = store
            .conn
            .prepare(sql)?
            .query_map([word_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        ids.extend(found);
    }
    let mut seen: std::collections::HashSet<String> =
        by_name.iter().map(|a| a.qname.clone()).collect();
    let mut all: Vec<SymbolAddr> = by_name;
    for id in ids {
        let addr = addr_by_id(store, id)?;
        if seen.insert(addr.qname.clone()) {
            all.push(addr);
        }
    }
    all.sort_by(|a, b| a.qname.cmp(&b.qname));
    all.into_iter()
        .map(|addr| {
            let id: i64 = store.conn.query_row(
                "SELECT id FROM symbol WHERE qname = ?1",
                [&addr.qname],
                |r| r.get(0),
            )?;
            let tags = crate::tags::bridge_tags(&store.conn, id, &[word_id])?
                .into_iter()
                .map(str::to_string)
                .collect();
            Ok(TaggedSymbol { addr, tags })
        })
        .collect()
}

pub fn word_info(
    store: &SqliteStore,
    word: &str,
    weights: &crate::config::BridgeWeights,
) -> Result<WordInfo, CoreError> {
    word_core(store, word, Some(weights))
}

/// Só verbos, papéis, camadas e vizinhas de símbolo da palavra, sem a ficha de pontes nem os
/// símbolos etiquetados: barato em repositório grande (o `brief` usa para o padrão).
pub fn word_pattern(store: &SqliteStore, word: &str) -> Result<WordInfo, CoreError> {
    word_core(store, word, None)
}

fn word_core(
    store: &SqliteStore,
    word: &str,
    weights: Option<&crate::config::BridgeWeights>,
) -> Result<WordInfo, CoreError> {
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
    let symbols = match weights {
        Some(_) => {
            let by_name = store
                .conn
                .prepare(&sql)?
                .query_map([word_id], addr_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            tagged_symbols(store, word_id, by_name)?
        }
        None => Vec::new(),
    };

    let mut verbs = Vec::new();
    let mut roles = Vec::new();
    let mut neighbors = Vec::new();
    let mut ignored = Vec::new();
    let mut stmt = store.conn.prepare(
        "SELECT w2.text, o2.slot, COALESCE(ws.esqueleto, 0) FROM occurrence o1
         JOIN occurrence o2 ON o2.symbol_id = o1.symbol_id AND o2.word_id != o1.word_id
         JOIN word w2 ON w2.id = o2.word_id LEFT JOIN word_stats ws ON ws.word_id = w2.id
         WHERE o1.word_id = ?1
         GROUP BY w2.text, o2.slot
         ORDER BY COUNT(DISTINCT o2.symbol_id) DESC, w2.text",
    )?;
    let rows = stmt.query_map(params![word_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)? != 0,
        ))
    })?;
    for row in rows {
        let (text, slot, skeleton) = row?;
        if skeleton {
            if !ignored.contains(&text) {
                ignored.push(text);
            }
            continue;
        }
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

    let stats = word_stats(store, &word)?;
    let bridges = match weights {
        Some(w) => crate::bridges::word_bridges(store, &word, w)?,
        None => Vec::new(),
    };
    Ok(WordInfo {
        word,
        verbs,
        roles,
        layers,
        symbols,
        neighbors,
        bridges,
        ignored,
        stats,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainItem {
    /// Símbolo do repo; `None` para chamada a destino externo (`ext`).
    pub addr: Option<SymbolAddr>,
    pub ext: Option<String>,
    pub depth: i64,
    pub line: Option<i64>,
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chain {
    pub target: SymbolAddr,
    pub items: Vec<ChainItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TestsInfo {
    /// Testes com chamada exata ao símbolo.
    pub exact: Vec<RelItem>,
    /// Testes por chamada aproximada ou sobreposição de palavras.
    pub approx: Vec<RelItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Impact {
    pub target: SymbolAddr,
    pub callers: Vec<ChainItem>,
    pub consumers: Vec<RelItem>,
    pub protocol: Vec<RelItem>,
    pub tests: TestsInfo,
    pub notes: Vec<crate::notes::Note>,
    /// Itens omitidos por já terem sido entregues nesta task (spec §8.4).
    pub unchanged: Option<usize>,
}

fn target_id(store: &SqliteStore, qname: &str) -> Result<i64, CoreError> {
    Ok(store
        .conn
        .query_row("SELECT id FROM symbol WHERE qname = ?1", [qname], |r| {
            r.get(0)
        })?)
}

const CALL_KINDS: &str = "('CHAMA', 'CRIA')";

/// Percorre arestas de chamada, nível a nível (cada símbolo aparece no menor nível).
/// `up` segue de quem chama; `!up`, do que é chamado.
fn walk_calls(
    store: &SqliteStore,
    start: i64,
    depth: i64,
    up: bool,
) -> Result<Vec<ChainItem>, CoreError> {
    let (near, far) = if up { ("dst", "src") } else { ("src", "dst") };
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, e.line, e.approx, s.id FROM edge e
         JOIN symbol s ON s.id = e.{far} LEFT JOIN file f ON f.id = s.file_id
         WHERE e.{near} = ?1 AND e.kind IN {CALL_KINDS} ORDER BY s.qname, e.line"
    );
    let mut stmt = store.conn.prepare(&sql)?;
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::from([start]);
    let mut frontier: Vec<(i64, bool)> = vec![(start, false)];
    let mut items = Vec::new();
    for level in 1..=depth {
        let mut level_seen = std::collections::HashSet::new();
        let mut next: Vec<(i64, bool)> = Vec::new();
        for (node, path_approx) in &frontier {
            let rows = stmt.query_map([node], |r| {
                Ok((
                    addr_from_row(r)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, i64>(7)? != 0,
                    r.get::<_, i64>(8)?,
                ))
            })?;
            for row in rows {
                let (addr, line, approx, id) = row?;
                if seen.contains(&id) {
                    continue;
                }
                let approx = approx || *path_approx;
                items.push(ChainItem {
                    addr: Some(addr),
                    ext: None,
                    depth: level,
                    line,
                    approx,
                });
                if level_seen.insert(id) {
                    next.push((id, approx));
                }
            }
            if !up && level == 1 {
                let mut ext = store.conn.prepare(
                    "SELECT dst_text, line FROM edge WHERE src = ?1 AND kind = 'CHAMA'
                     AND dst IS NULL ORDER BY dst_text, line",
                )?;
                let rows = ext.query_map([node], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                })?;
                for row in rows {
                    let (text, line) = row?;
                    items.push(ChainItem {
                        addr: None,
                        ext: Some(text),
                        depth: level,
                        line,
                        approx: false,
                    });
                }
            }
        }
        seen.extend(level_seen);
        frontier = next;
    }
    Ok(items)
}

fn chain(store: &SqliteStore, target: &str, depth: i64, up: bool) -> Result<Chain, CoreError> {
    let qname = resolve_qname(store, target)?;
    let id = target_id(store, &qname)?;
    Ok(Chain {
        target: addr_of(store, &qname)?,
        items: walk_calls(store, id, depth.clamp(1, 3), up)?,
    })
}

/// Quem chama o símbolo, até `depth` níveis (1 a 3).
pub fn callers(store: &SqliteStore, target: &str, depth: i64) -> Result<Chain, CoreError> {
    chain(store, target, depth, true)
}

/// O que o símbolo chama, até `depth` níveis (1 a 3).
pub fn callees(store: &SqliteStore, target: &str, depth: i64) -> Result<Chain, CoreError> {
    chain(store, target, depth, false)
}

/// Testes com chamada direta (exata, ou aproximada) e testes por sobreposição de palavras.
pub fn tests_for(store: &SqliteStore, target: &str) -> Result<TestsInfo, CoreError> {
    let qname = resolve_qname(store, target)?;
    let id = target_id(store, &qname)?;
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, e.line, e.approx, e.attr FROM edge e
         JOIN symbol s ON s.id = e.src LEFT JOIN file f ON f.id = s.file_id
         WHERE e.dst = ?1 AND e.kind IN {CALL_KINDS} AND s.is_test = 1 ORDER BY s.qname, e.line"
    );
    let direct: Vec<RelItem> = store
        .conn
        .prepare(&sql)?
        .query_map([id], |r| {
            Ok(RelItem {
                addr: addr_from_row(r)?,
                line: r.get(6)?,
                approx: r.get::<_, i64>(7)? != 0,
                attr: None,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut info = TestsInfo::default();
    let mut listed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &direct {
        if !item.approx && listed.insert(item.addr.qname.clone()) {
            info.exact.push(item.clone());
        }
    }
    for item in direct {
        if listed.insert(item.addr.qname.clone()) {
            info.approx.push(item);
        }
    }
    // Sobreposição: palavras `object` do símbolo e da classe dele, presentes em nomes de testes.
    let sql = format!(
        "SELECT DISTINCT {ADDR_COLUMNS} FROM symbol t
         JOIN occurrence o ON o.symbol_id = t.id
         JOIN symbol s ON s.id = t.id LEFT JOIN file f ON f.id = s.file_id
         WHERE t.is_test = 1 AND o.word_id IN (
             SELECT o2.word_id FROM occurrence o2
             WHERE o2.slot = 'object'
               AND o2.symbol_id IN (?1, (SELECT parent_id FROM symbol WHERE id = ?1)))
         ORDER BY s.qname"
    );
    let overlap: Vec<SymbolAddr> = store
        .conn
        .prepare(&sql)?
        .query_map([id], addr_from_row)?
        .collect::<Result<_, _>>()?;
    for addr in overlap {
        if listed.insert(addr.qname.clone()) {
            info.approx.push(RelItem {
                addr,
                attr: None,
                line: None,
                approx: true,
            });
        }
    }
    Ok(info)
}

/// Contrapartida de Protocol: o método do Protocol que a classe implementa, ou as implementações
/// do método quando o alvo é de um Protocol.
fn protocol_counterparts(store: &SqliteStore, id: i64) -> Result<Vec<RelItem>, CoreError> {
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, e.approx FROM symbol me
         JOIN edge e ON e.kind = 'ESPECIALIZA'
         JOIN symbol s ON s.parent_id = (CASE WHEN e.src = me.parent_id THEN e.dst ELSE e.src END)
              AND s.name = me.name AND s.kind = 'method'
         LEFT JOIN file f ON f.id = s.file_id
         JOIN symbol p ON p.id = (CASE WHEN e.src = me.parent_id THEN e.dst ELSE e.src END)
         WHERE me.id = ?1 AND me.kind = 'method'
           AND ((e.src = me.parent_id AND (SELECT is_protocol FROM symbol WHERE id = e.dst) = 1)
             OR (e.dst = me.parent_id AND (SELECT is_protocol FROM symbol WHERE id = me.parent_id) = 1))
         ORDER BY s.qname"
    );
    let rows = store
        .conn
        .prepare(&sql)?
        .query_map([id], |r| {
            Ok(RelItem {
                addr: addr_from_row(r)?,
                attr: None,
                line: None,
                approx: r.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// O que muda se o símbolo mudar: chamadores, consumidores do tipo, Protocol, testes.
pub fn impact(store: &SqliteStore, target: &str) -> Result<Impact, CoreError> {
    let qname = resolve_qname(store, target)?;
    let id = target_id(store, &qname)?;
    let addr = addr_of(store, &qname)?;
    let mut consumers = Vec::new();
    if addr.kind == "class" {
        consumers.extend(related(store, &qname, "RECEBE")?);
        consumers.extend(related(store, &qname, "RETORNA")?);
    }
    Ok(Impact {
        callers: walk_calls(store, id, 2, true)?,
        consumers,
        protocol: protocol_counterparts(store, id)?,
        tests: tests_for(store, &qname)?,
        notes: Vec::new(),
        unchanged: None,
        target: addr,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordStats {
    pub uso: i64,
    pub alcance: i64,
    pub grau: i64,
    pub idf: f64,
    pub especificidade: f64,
    pub centralidade: f64,
    pub coerencia: f64,
    pub esqueleto: bool,
}

/// Estatísticas de uma palavra (spec §5.6); `None` se a palavra não existe.
pub fn word_stats(store: &SqliteStore, word: &str) -> Result<Option<WordStats>, CoreError> {
    Ok(store
        .conn
        .query_row(
            "SELECT ws.uso, ws.alcance, ws.grau, ws.idf, ws.especificidade, ws.centralidade, ws.esqueleto, COALESCE(ws.coerencia, 0.0)
             FROM word_stats ws JOIN word w ON w.id = ws.word_id WHERE w.text = ?1",
            [word.to_lowercase()],
            |r| {
                Ok(WordStats {
                    uso: r.get(0)?,
                    alcance: r.get(1)?,
                    grau: r.get(2)?,
                    idf: r.get(3)?,
                    especificidade: r.get(4)?,
                    centralidade: r.get(5)?,
                    coerencia: r.get(7)?,
                    esqueleto: r.get::<_, i64>(6)? != 0,
                })
            },
        )
        .optional()?)
}
