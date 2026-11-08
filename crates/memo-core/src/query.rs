//! Consultas sobre o grafo (spec §8.2).

use crate::error::CoreError;
use crate::evidence::Evidence;
use crate::index::module_of;
use crate::store::SqliteStore;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
    /// Trechos que vale ler: o alvo e até 4 chamadores exatos (TASK-7, D4).
    pub ler: Vec<ReadSpot>,
    /// Membros, quando o símbolo é um Enum (TASK-7, D6).
    pub membros: Vec<EnumMember>,
}

/// Membro de Enum: o nome e o valor literal, quando há.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnumMember {
    pub name: String,
    pub value: Option<String>,
}

fn enum_members(store: &SqliteStore, qname: &str) -> Result<Vec<EnumMember>, CoreError> {
    Ok(store
        .conn
        .prepare(
            "SELECT m.name, m.value_text FROM symbol m JOIN symbol c ON c.id = m.parent_id
             WHERE c.qname = ?1 AND m.kind = 'attribute' AND m.value_text IS NOT NULL
             ORDER BY m.line_start, m.name",
        )?
        .query_map([qname], |r| {
            let value: String = r.get(1)?;
            Ok(EnumMember {
                name: r.get(0)?,
                value: (!value.is_empty()).then_some(value),
            })
        })?
        .collect::<Result<_, _>>()?)
}

/// Trecho de código para ler, com o motivo como fato (`alvo`, `chama o alvo em :L<n>`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadSpot {
    pub path: String,
    pub start: i64,
    pub end: i64,
    pub motivo: String,
}

/// Até este tamanho o trecho é o símbolo inteiro; acima, uma janela.
const READ_FULL_MAX: i64 = 60;
const READ_WINDOW: i64 = 20;
const READ_CALLERS: usize = 4;

/// Trecho de um símbolo: inteiro até 60 linhas; acima, 20 linhas a partir do início ou ao redor
/// de `focus` (a linha da chamada), sem sair do símbolo.
pub(crate) fn read_spot(addr: &SymbolAddr, focus: Option<i64>, motivo: String) -> ReadSpot {
    let (start, end) = (addr.line_start, addr.line_end);
    let (from, to) = if end - start < READ_FULL_MAX {
        (start, end)
    } else {
        let from = focus.map_or(start, |f| (f - READ_WINDOW / 2).max(start));
        (from, (from + READ_WINDOW - 1).min(end))
    };
    ReadSpot {
        path: addr.path.clone(),
        start: from,
        end: to,
        motivo,
    }
}

/// Complete intersected statements when the resulting spot stays within forty lines.
pub(crate) fn structural_read_spot(
    store: &SqliteStore,
    addr: &SymbolAddr,
    focus: Option<i64>,
    motivo: String,
) -> Result<ReadSpot, CoreError> {
    let mut spot = read_spot(addr, focus, motivo);
    if addr.line_end - addr.line_start < READ_FULL_MAX {
        return Ok(spot);
    }
    let regions = store.conn.prepare(
        "SELECT r.start,r.end FROM read_region r JOIN symbol s ON s.id=r.symbol_id WHERE s.qname=?1 ORDER BY r.start,r.end DESC"
    )?.query_map([&addr.qname], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
    for (start, end) in regions {
        if start > spot.end || end < spot.start {
            continue;
        }
        let from = spot.start.min(start).max(addr.line_start);
        let to = spot.end.max(end).min(addr.line_end);
        if to - from < READ_WINDOW * 2 {
            spot.start = from;
            spot.end = to;
        }
    }
    Ok(spot)
}

/// `ler:` do `symbol`: o alvo e até 4 chamadores exatos d1 de produção, na ordem dos chamadores.
fn symbol_read_spots(
    store: &SqliteStore,
    qname: &str,
    addr: &SymbolAddr,
) -> Result<Vec<ReadSpot>, CoreError> {
    let mut spots = vec![structural_read_spot(store, addr, None, "alvo".to_string())?];
    for item in callers(store, qname, 1)?.items.iter().filter(|i| !i.approx) {
        if let (Some(caller), Some(line)) = (&item.addr, item.line) {
            if caller.in_tests() {
                continue;
            }
            let motivo = format!("chama o alvo em :L{line}");
            spots.push(structural_read_spot(store, caller, Some(line), motivo)?);
            if spots.len() > READ_CALLERS {
                break;
            }
        }
    }
    Ok(spots)
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
    /// Representatividade em arquivos (só na ficha completa, não no padrão do `brief`).
    pub representativeness: Option<crate::representativeness::Representativeness>,
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
    /// Módulo do outro extremo (`ext` quando não resolvido), para agrupar listas longas.
    pub other_module: String,
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
            ler: Vec::new(),
            membros: Vec::new(),
        })
    })?;
    info.membros = enum_members(store, &qname)?;
    info.edges = direct_edges(store, &qname)?;
    info.ler = symbol_read_spots(store, &qname, &info.addr)?;
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
                other_module: if path.is_empty() {
                    "ext".to_string()
                } else {
                    crate::index::module_of(&path)
                },
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
    let mut items = rows.collect::<Result<Vec<_>, _>>()?;
    rank_relations(&mut items, &addr_of(store, qname)?);
    Ok(items)
}

/// Confidence precedes structural proximity; names and lines make ties reproducible.
fn proximity(target: &SymbolAddr, other: &SymbolAddr) -> usize {
    if target.path == other.path {
        return usize::MAX;
    }
    let target_dir = target.path.rsplit_once('/').map_or("", |(d, _)| d);
    let other_dir = other.path.rsplit_once('/').map_or("", |(d, _)| d);
    target_dir
        .split('/')
        .zip(other_dir.split('/'))
        .take_while(|(a, b)| !a.is_empty() && a == b)
        .count()
}

fn rank_relations(items: &mut [RelItem], target: &SymbolAddr) {
    items.sort_by(|a, b| {
        a.approx
            .cmp(&b.approx)
            .then_with(|| proximity(target, &b.addr).cmp(&proximity(target, &a.addr)))
            .then_with(|| a.addr.qname.cmp(&b.addr.qname))
            .then_with(|| a.line.cmp(&b.line))
    });
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

/// Partes de uma entrada de `query word` (TASK-7, D7): o mesmo classificador das palavras de
/// nome, que divide em `_`, `.` e caixa e descarta os `_` do começo e do fim; sem repetição.
pub fn word_parts(input: &str, config: &crate::config::Config) -> Vec<String> {
    // `__init__` e `_order_`: o classificador ignora nomes com `_` nas pontas; tirá-los antes.
    let trimmed: Vec<&str> = input.split('.').map(|p| p.trim_matches('_')).collect();
    let mut parts: Vec<String> = Vec::new();
    for occ in crate::sources::use_words(&trimmed.join("."), config) {
        if !parts.contains(&occ.text) {
            parts.push(occ.text);
        }
    }
    parts
}

/// Palavra composta: a ficha curta de cada parte conhecida, as partes desconhecidas e os
/// símbolos cujo nome tem todas as partes conhecidas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompositeWord {
    pub input: String,
    pub parts: Vec<String>,
    pub cards: Vec<WordInfo>,
    pub unknown: Vec<String>,
    pub symbols: Vec<SymbolAddr>,
}

/// Cruza as partes de uma palavra composta; nenhuma parte conhecida → `NotFound`.
pub fn composite_word(
    store: &SqliteStore,
    input: &str,
    parts: &[String],
) -> Result<CompositeWord, CoreError> {
    let mut cards = Vec::new();
    let mut unknown = Vec::new();
    for part in parts {
        match word_pattern(store, part) {
            Ok(card) => cards.push(card),
            Err(CoreError::NotFound(_)) => unknown.push(part.clone()),
            Err(e) => return Err(e),
        }
    }
    if cards.is_empty() {
        return Err(CoreError::NotFound(input.to_string()));
    }
    let placeholders = vec!["?"; cards.len()].join(",");
    let sql = format!(
        "SELECT {ADDR_COLUMNS} FROM symbol s LEFT JOIN file f ON f.id = s.file_id
         WHERE s.id IN (
           SELECT o.owner_id FROM occurrence o JOIN word w ON w.id = o.word_id
           WHERE o.owner_kind = 'symbol' AND o.source = 'name' AND w.text IN ({placeholders})
           GROUP BY o.owner_id HAVING COUNT(DISTINCT w.text) = {})
         ORDER BY s.qname",
        cards.len()
    );
    let words: Vec<&str> = cards.iter().map(|c| c.word.as_str()).collect();
    let symbols = store
        .conn
        .prepare(&sql)?
        .query_map(rusqlite::params_from_iter(words), addr_from_row)?
        .collect::<Result<_, _>>()?;
    Ok(CompositeWord {
        input: input.to_string(),
        parts: parts.to_vec(),
        cards,
        unknown,
        symbols,
    })
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
    let representativeness = match weights {
        Some(_) => Some(crate::representativeness::representativeness(
            store, word_id,
        )?),
        None => None,
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
        representativeness,
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

/// Um uso do nível por palavra do `impact`: o símbolo que o contém, a linha e o nível.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ByWordItem {
    #[serde(flatten)]
    pub item: RelItem,
    pub nivel: crate::evidence::Evidence,
    pub origem: Origem,
}

/// De onde vem um item do nível por palavra: um uso (`raw_ref`) ou o tipo anotado de um atributo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origem {
    Uso,
    Tipo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Impact {
    pub target: SymbolAddr,
    pub callers: Vec<ChainItem>,
    pub consumers: Vec<RelItem>,
    pub protocol: Vec<RelItem>,
    /// Usos e tipos de produção que contêm as palavras do nome do alvo e que nenhuma aresta cobre,
    /// cada um com a linha e o nível de evidência como rótulo; nada é escondido (TASK-6, D17).
    pub by_word: Vec<ByWordItem>,
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
         WHERE e.{near} = ?1 AND e.kind IN {CALL_KINDS} ORDER BY e.approx, s.qname, e.line"
    );
    let mut stmt = store.conn.prepare(&sql)?;
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::from([start]);
    let mut frontier: Vec<(i64, bool)> = vec![(start, false)];
    let mut items = Vec::new();
    for level in 1..=depth {
        let mut level_seen = std::collections::HashSet::new();
        let mut next = BTreeMap::<i64, bool>::new();
        frontier.sort_by_key(|(_, approx)| *approx);
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
                level_seen.insert(id);
                next.entry(id)
                    .and_modify(|a| *a &= approx)
                    .or_insert(approx);
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
        frontier = next.into_iter().collect();
    }
    let target = addr_by_id(store, start)?;
    items.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| (a.addr.is_none(), a.approx).cmp(&(b.addr.is_none(), b.approx)))
            .then_with(|| {
                let pa = a.addr.as_ref().map_or(0, |s| proximity(&target, s));
                let pb = b.addr.as_ref().map_or(0, |s| proximity(&target, s));
                pb.cmp(&pa)
            })
            .then_with(|| {
                a.addr
                    .as_ref()
                    .map(|s| &s.qname)
                    .cmp(&b.addr.as_ref().map(|s| &s.qname))
            })
            .then_with(|| a.ext.cmp(&b.ext))
            .then_with(|| a.line.cmp(&b.line))
    });
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
    let target = addr_by_id(store, id)?;
    rank_relations(&mut info.exact, &target);
    // Calls inferred from a receiver remain stronger than a lexical overlap with no line.
    info.approx.sort_by(|a, b| {
        a.line
            .is_none()
            .cmp(&b.line.is_none())
            .then_with(|| proximity(&target, &b.addr).cmp(&proximity(&target, &a.addr)))
            .then_with(|| a.addr.qname.cmp(&b.addr.qname))
            .then_with(|| a.line.cmp(&b.line))
    });
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
    let mut rows: Vec<RelItem> = store
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
    rank_relations(&mut rows, &addr_by_id(store, id)?);
    Ok(rows)
}

/// Ids dos símbolos ligados a `start` por `ESPECIALIZA` exata, no sentido dado, de forma
/// transitiva; `start` incluso.
fn hierarchy(store: &SqliteStore, start: i64, up: bool) -> Result<BTreeSet<i64>, CoreError> {
    let sql = if up {
        "SELECT dst FROM edge WHERE kind = 'ESPECIALIZA' AND approx = 0 AND src = ?1 AND dst IS NOT NULL"
    } else {
        "SELECT src FROM edge WHERE kind = 'ESPECIALIZA' AND approx = 0 AND dst = ?1"
    };
    let mut stmt = store.conn.prepare(sql)?;
    let mut seen = BTreeSet::from([start]);
    let mut queue = vec![start];
    while let Some(class) = queue.pop() {
        let next: Vec<i64> = stmt
            .query_map([class], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for id in next {
            if seen.insert(id) {
                queue.push(id);
            }
        }
    }
    Ok(seen)
}

/// Família da classe dona de um método (TASK-6, D9 e D9a): a dona, as bases e as subclasses por
/// `ESPECIALIZA` exata, e os Protocols que a dona ou as bases implementam, também por estrutura.
fn family(store: &SqliteStore, owner: i64) -> Result<BTreeSet<i64>, CoreError> {
    let up = hierarchy(store, owner, true)?;
    let mut members = hierarchy(store, owner, false)?;
    let mut stmt = store.conn.prepare(
        "SELECT e.dst FROM edge e JOIN symbol p ON p.id = e.dst
         WHERE e.kind = 'ESPECIALIZA' AND e.src = ?1 AND p.is_protocol = 1",
    )?;
    for class in &up {
        let protocols: Vec<i64> = stmt
            .query_map([class], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        members.extend(protocols);
    }
    members.extend(up);
    Ok(members)
}

fn id_list(ids: impl IntoIterator<Item = i64>) -> String {
    ids.into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Ids das palavras de nome do alvo.
fn name_word_ids(store: &SqliteStore, id: i64) -> Result<Vec<i64>, CoreError> {
    Ok(store
        .conn
        .prepare(
            "SELECT DISTINCT word_id FROM occurrence
             WHERE owner_kind = 'symbol' AND owner_id = ?1 AND source = 'name' ORDER BY word_id",
        )?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Símbolo candidato do nível por palavra.
struct Candidate {
    addr: SymbolAddr,
    id: i64,
    file_id: Option<i64>,
    type_text: Option<String>,
}

/// Símbolos de produção cujas palavras de uso (`ref` e `type`), somadas, contêm todas as palavras
/// do nome do alvo: a regra da primeira rodada, que a inferência aditiva preserva (D17).
fn candidate_symbols(
    store: &SqliteStore,
    id: i64,
    words: &[i64],
) -> Result<Vec<Candidate>, CoreError> {
    let list = id_list(words.iter().copied());
    let sql = format!(
        "SELECT {ADDR_COLUMNS}, s.id, s.file_id, s.type_text FROM symbol s
         LEFT JOIN file f ON f.id = s.file_id
         WHERE s.is_test = 0 AND s.id != ?1 AND s.id IN (
           SELECT sym FROM (
             SELECT r.src AS sym, o.word_id FROM occurrence o
             JOIN raw_ref r ON o.owner_kind = 'ref' AND o.owner_id = r.id
             WHERE o.source = 'ref' AND o.word_id IN ({list})
             UNION
             SELECT owner_id, word_id FROM occurrence
             WHERE owner_kind = 'symbol' AND source = 'type' AND word_id IN ({list}))
           GROUP BY sym HAVING COUNT(DISTINCT word_id) = {})
         ORDER BY s.qname",
        words.len()
    );
    Ok(store
        .conn
        .prepare(&sql)?
        .query_map([id], |r| {
            Ok(Candidate {
                addr: addr_from_row(r)?,
                id: r.get(6)?,
                file_id: r.get(7)?,
                type_text: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

/// Um uso de um símbolo candidato: linha, textos e quantas palavras do nome do alvo ele tem.
struct Use {
    line: i64,
    texts: Vec<String>,
    name_hits: usize,
}

/// Usos de todos os candidatos, em duas consultas (D20), agrupados pelo símbolo.
fn uses_of(
    store: &SqliteStore,
    symbols: &[i64],
    words: &[i64],
) -> Result<HashMap<i64, Vec<Use>>, CoreError> {
    let syms = id_list(symbols.iter().copied());
    let hits: HashMap<i64, i64> = store
        .conn
        .prepare(&format!(
            "SELECT r.id, COUNT(DISTINCT o.word_id) FROM raw_ref r
             JOIN occurrence o ON o.owner_kind = 'ref' AND o.owner_id = r.id AND o.source = 'ref'
             WHERE r.src IN ({syms}) AND o.word_id IN ({}) GROUP BY r.id",
            id_list(words.iter().copied())
        ))?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut uses: HashMap<i64, Vec<Use>> = HashMap::new();
    let mut stmt = store.conn.prepare(&format!(
        "SELECT src, id, line, callee_text, receiver FROM raw_ref
         WHERE src IN ({syms}) ORDER BY src, line, id"
    ))?;
    let rows = stmt.query_map([], |r| {
        let texts: Vec<String> = [r.get::<_, Option<String>>(3)?, r.get(4)?]
            .into_iter()
            .flatten()
            .collect();
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get(2)?, texts))
    })?;
    for row in rows {
        let (src, id, line, texts) = row?;
        let name_hits = usize::try_from(hits.get(&id).copied().unwrap_or(0)).unwrap_or(0);
        uses.entry(src).or_default().push(Use {
            line,
            texts,
            name_hits,
        });
    }
    Ok(uses)
}

/// Candidatos cujo tipo anotado contém todas as palavras do nome do alvo (D19).
fn typed_with_name(
    store: &SqliteStore,
    symbols: &[i64],
    words: &[i64],
) -> Result<HashSet<i64>, CoreError> {
    Ok(store
        .conn
        .prepare(&format!(
            "SELECT owner_id FROM occurrence
             WHERE owner_kind = 'symbol' AND source = 'type' AND owner_id IN ({})
               AND word_id IN ({}) GROUP BY owner_id HAVING COUNT(DISTINCT word_id) = {}",
            id_list(symbols.iter().copied()),
            id_list(words.iter().copied()),
            words.len()
        ))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Dados da evidência para o alvo: nomes da família (método) ou módulo e imports (demais).
enum AnchorData {
    Family(Vec<String>),
    Module {
        target: String,
        target_module: String,
        imports: HashMap<i64, Vec<String>>,
    },
}

fn anchor_for(
    store: &SqliteStore,
    id: i64,
    files: &BTreeSet<i64>,
) -> Result<AnchorData, CoreError> {
    let (kind, parent, qname, path): (String, Option<i64>, String, String) = store.conn.query_row(
        "SELECT s.kind, s.parent_id, s.qname, COALESCE(f.path, '') FROM symbol s
             LEFT JOIN file f ON f.id = s.file_id WHERE s.id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    if let ("method", Some(owner)) = (kind.as_str(), parent) {
        let names = store
            .conn
            .prepare(&format!(
                "SELECT name FROM symbol WHERE id IN ({}) ORDER BY name",
                id_list(family(store, owner)?)
            ))?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        return Ok(AnchorData::Family(names));
    }
    let mut imports: HashMap<i64, Vec<String>> = HashMap::new();
    let mut stmt = store.conn.prepare(&format!(
        "SELECT file_id, target FROM import WHERE file_id IN ({}) ORDER BY file_id, target",
        id_list(files.iter().copied())
    ))?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?)))? {
        let (file, target) = row?;
        imports.entry(file).or_default().push(target);
    }
    let target_module = if kind == "module" {
        qname.clone()
    } else {
        module_of(&path)
    };
    Ok(AnchorData::Module {
        target: qname,
        target_module,
        imports,
    })
}

/// Nível de um item: textos do próprio item (e da mesma linha, D14) contra os demais do símbolo.
fn item_level(
    anchor: &AnchorData,
    c: &Candidate,
    uses: &[Use],
    line: i64,
    own: &[&str],
) -> Evidence {
    use crate::evidence::{Anchor, classify};
    match anchor {
        AnchorData::Family(names) => {
            let mut same: Vec<&str> = own.to_vec();
            let mut rest: Vec<&str> = Vec::new();
            for u in uses {
                let texts = u.texts.iter().map(String::as_str);
                if u.line == line {
                    same.extend(texts)
                } else {
                    rest.extend(texts)
                }
            }
            rest.extend(c.type_text.as_deref().filter(|t| !own.contains(t)));
            classify(&same, &rest, &Anchor::Family(names))
        }
        AnchorData::Module {
            target,
            target_module,
            imports,
        } => {
            let none = Vec::new();
            let file_imports = c.file_id.and_then(|f| imports.get(&f)).unwrap_or(&none);
            let use_module = module_of(&c.addr.path);
            let anchor = Anchor::Module {
                target,
                target_module,
                use_module: &use_module,
                imports: file_imports,
            };
            classify(&[], &[], &anchor)
        }
    }
}

/// Itens de um candidato: cada uso com todas as palavras do nome, o tipo anotado que as tem, e,
/// se as palavras só aparecem somadas, o primeiro uso que tem alguma (D17).
fn candidate_items(
    anchor: &AnchorData,
    c: &Candidate,
    uses: &[Use],
    typed: bool,
    words: usize,
) -> Vec<ByWordItem> {
    let item = |line: i64, origem: Origem, own: &[&str]| ByWordItem {
        item: RelItem {
            addr: c.addr.clone(),
            attr: None,
            line: Some(line),
            approx: true,
        },
        nivel: item_level(anchor, c, uses, line, own),
        origem,
    };
    let mut items: Vec<ByWordItem> = uses
        .iter()
        .filter(|u| u.name_hits == words)
        .map(|u| item(u.line, Origem::Uso, &[]))
        .collect();
    if typed {
        let own: Vec<&str> = c.type_text.iter().map(String::as_str).collect();
        items.push(item(c.addr.line_start, Origem::Tipo, &own));
    }
    if items.is_empty() {
        let line = uses
            .iter()
            .find(|u| u.name_hits > 0)
            .map_or(c.addr.line_start, |u| u.line);
        items.push(item(line, Origem::Uso, &[]));
    }
    items
}

/// Camada relacional do `impact`: todo símbolo de produção cujas palavras de uso contêm as do nome
/// do alvo, menos os que as arestas cobrem, com um item por uso (e por tipo) e o nível como rótulo
/// e ordem, nunca como filtro (TASK-6, D17 a D20).
fn by_word(
    store: &SqliteStore,
    id: i64,
    covered: &HashSet<String>,
) -> Result<Vec<ByWordItem>, CoreError> {
    let words = name_word_ids(store, id)?;
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let candidates: Vec<Candidate> = candidate_symbols(store, id, &words)?
        .into_iter()
        .filter(|c| !covered.contains(&c.addr.qname))
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = candidates.iter().map(|c| c.id).collect();
    let uses = uses_of(store, &ids, &words)?;
    let typed = typed_with_name(store, &ids, &words)?;
    let files: BTreeSet<i64> = candidates.iter().filter_map(|c| c.file_id).collect();
    let anchor = anchor_for(store, id, &files)?;
    let none = Vec::new();
    let mut best: BTreeMap<(String, i64, Origem), ByWordItem> = BTreeMap::new();
    for c in &candidates {
        let symbol_uses = uses.get(&c.id).unwrap_or(&none);
        for item in candidate_items(&anchor, c, symbol_uses, typed.contains(&c.id), words.len()) {
            let key = (
                c.addr.qname.clone(),
                item.item.line.unwrap_or(0),
                item.origem,
            );
            // Dois usos na mesma linha do mesmo símbolo: vale o de evidência mais forte.
            if best.get(&key).is_none_or(|kept| item.nivel < kept.nivel) {
                best.insert(key, item);
            }
        }
    }
    let mut items: Vec<ByWordItem> = best.into_values().collect();
    let target = addr_by_id(store, id)?;
    items.sort_by(|a, b| {
        a.nivel
            .cmp(&b.nivel)
            .then_with(|| proximity(&target, &b.item.addr).cmp(&proximity(&target, &a.item.addr)))
            .then_with(|| {
                (&a.item.addr.qname, a.item.line, a.origem).cmp(&(
                    &b.item.addr.qname,
                    b.item.line,
                    b.origem,
                ))
            })
    });
    Ok(items)
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
    let callers = walk_calls(store, id, 2, true)?;
    let protocol = protocol_counterparts(store, id)?;
    let covered: HashSet<String> = callers
        .iter()
        .filter_map(|c| c.addr.as_ref().map(|a| a.qname.clone()))
        .chain(
            consumers
                .iter()
                .chain(&protocol)
                .map(|c| c.addr.qname.clone()),
        )
        .collect();
    let by_word = by_word(store, id, &covered)?;
    Ok(Impact {
        by_word,
        callers,
        consumers,
        protocol,
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
