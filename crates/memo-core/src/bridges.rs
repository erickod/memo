//! Pontes entre palavras: o que liga duas palavras por símbolo, teste, import, chamada, arquivo
//! ou contexto. São derivadas de `occurrence`, `edge`, `import` e `file`, sem tabela própria (D19).
//!
//! Regras (TASK-2): a palavra de um teste só se propaga ao símbolo testado por chamada exata
//! (D11), e a chamada de um teste nunca vira ponte de chamada; ponte aproximada pesa metade; import
//! de alvo fora do repositório usa o peso `import_external`.

use crate::config::BridgeWeights;
use crate::error::CoreError;
use crate::store::SqliteStore;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum BridgeKind {
    Symbol,
    Test,
    Import,
    Call,
    File,
    Context,
}

impl BridgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BridgeKind::Symbol => "símbolo",
            BridgeKind::Test => "teste",
            BridgeKind::Import => "import",
            BridgeKind::Call => "chamada",
            BridgeKind::File => "arquivo",
            BridgeKind::Context => "contexto",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bridge {
    pub other: String,
    pub kind: BridgeKind,
    pub weight: f64,
}

type Acc = BTreeMap<(BridgeKind, String), f64>;

fn add(acc: &mut Acc, kind: BridgeKind, other: &str, word: &str, weight: f64) {
    if other != word && weight > 0.0 {
        *acc.entry((kind, other.to_string())).or_insert(0.0) += weight;
    }
}

fn column(
    conn: &Connection,
    sql: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<String>, CoreError> {
    let rows = conn
        .prepare_cached(sql)?
        .query_map(args, |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Palavras de nome do símbolo, e as de docstring de teste quando `with_doc`.
fn symbol_words(
    conn: &Connection,
    symbol_id: i64,
    with_doc: bool,
) -> Result<Vec<String>, CoreError> {
    let sql = if with_doc {
        "SELECT DISTINCT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
         WHERE o.owner_kind = 'symbol' AND o.owner_id = ?1 ORDER BY w.text"
    } else {
        "SELECT DISTINCT w.text FROM occurrence o JOIN word w ON w.id = o.word_id
         WHERE o.owner_kind = 'symbol' AND o.owner_id = ?1 AND o.source = 'name' ORDER BY w.text"
    };
    column(conn, sql, [symbol_id])
}

fn symbol_ids_with_word(
    conn: &Connection,
    word_id: i64,
    with_doc: bool,
) -> Result<Vec<i64>, CoreError> {
    let filter = if with_doc { "" } else { "AND source = 'name'" };
    let sql = format!(
        "SELECT DISTINCT owner_id FROM occurrence
         WHERE word_id = ?1 AND owner_kind = 'symbol' {filter} ORDER BY owner_id"
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map([word_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn symbol_bridges(
    conn: &Connection,
    word: &str,
    word_id: i64,
    w: &BridgeWeights,
    acc: &mut Acc,
) -> Result<(), CoreError> {
    for symbol in symbol_ids_with_word(conn, word_id, false)? {
        for other in symbol_words(conn, symbol, false)? {
            add(acc, BridgeKind::Symbol, &other, word, w.symbol);
        }
    }
    Ok(())
}

/// Arestas de chamada (`CHAMA`, `CRIA`) com a marca de teste de quem chama.
struct Call {
    src: i64,
    dst: i64,
    approx: bool,
    src_is_test: bool,
}

fn calls(conn: &Connection) -> Result<Vec<Call>, CoreError> {
    let rows = conn
        .prepare_cached(
            "SELECT DISTINCT e.src, e.dst, e.approx, s.is_test FROM edge e JOIN symbol s ON s.id = e.src
             WHERE e.kind IN ('CHAMA', 'CRIA') AND e.dst IS NOT NULL ORDER BY e.src, e.dst, e.approx",
        )?
        .query_map([], |r| {
            Ok(Call {
                src: r.get(0)?,
                dst: r.get(1)?,
                approx: r.get::<_, i64>(2)? != 0,
                src_is_test: r.get::<_, i64>(3)? != 0,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

fn call_bridges(
    conn: &Connection,
    word: &str,
    word_id: i64,
    w: &BridgeWeights,
    acc: &mut Acc,
) -> Result<(), CoreError> {
    let with_word_name = symbol_ids_with_word(conn, word_id, false)?;
    let with_word_any = symbol_ids_with_word(conn, word_id, true)?;
    for call in calls(conn)? {
        if call.src_is_test {
            // D11: o teste só leva as palavras ao símbolo testado por chamada exata.
            if call.approx {
                continue;
            }
            if with_word_any.contains(&call.src) {
                for other in symbol_words(conn, call.dst, false)? {
                    add(acc, BridgeKind::Test, &other, word, w.test);
                }
            }
            if with_word_name.contains(&call.dst) {
                for other in symbol_words(conn, call.src, true)? {
                    add(acc, BridgeKind::Test, &other, word, w.test);
                }
            }
            continue;
        }
        let weight = w.call * if call.approx { w.approximate } else { 1.0 };
        if with_word_name.contains(&call.src) {
            for other in symbol_words(conn, call.dst, false)? {
                add(acc, BridgeKind::Call, &other, word, weight);
            }
        }
        if with_word_name.contains(&call.dst) {
            for other in symbol_words(conn, call.src, false)? {
                add(acc, BridgeKind::Call, &other, word, weight);
            }
        }
    }
    Ok(())
}

/// Palavras e donos carregados de uma vez (uma consulta por tipo), para as pontes de import,
/// arquivo e contexto trabalharem em memória em vez de uma consulta por import ou por arquivo.
struct Corpus {
    /// arquivo -> palavras de nome dos símbolos dele
    file_names: BTreeMap<i64, BTreeSet<i64>>,
    /// arquivo -> palavras do caminho e do módulo
    file_words: BTreeMap<i64, BTreeSet<i64>>,
    /// arquivo -> contexto
    file_context: BTreeMap<i64, String>,
    /// import (rowid) -> palavras
    imported: BTreeMap<i64, BTreeSet<i64>>,
    texts: BTreeMap<i64, String>,
    /// prefixos de `qname` que existem no repositório (para saber se um import é interno)
    prefixes: HashSet<String>,
}

fn pairs(conn: &Connection, sql: &str) -> Result<BTreeMap<i64, BTreeSet<i64>>, CoreError> {
    let mut map: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (owner, word) = row?;
        map.entry(owner).or_default().insert(word);
    }
    Ok(map)
}

fn qname_prefixes(conn: &Connection) -> Result<HashSet<String>, CoreError> {
    let mut prefixes = HashSet::new();
    for qname in column(conn, "SELECT qname FROM symbol", [])? {
        let mut end = 0;
        for part in qname.split('.') {
            end += part.len();
            prefixes.insert(qname[..end].to_string());
            end += 1;
        }
    }
    Ok(prefixes)
}

fn load_corpus(conn: &Connection) -> Result<Corpus, CoreError> {
    let file_names = pairs(
        conn,
        "SELECT s.file_id, o.word_id FROM occurrence o JOIN symbol s ON s.id = o.owner_id
         WHERE o.owner_kind = 'symbol' AND o.source = 'name' AND s.file_id IS NOT NULL
         ORDER BY s.file_id, o.word_id",
    )?;
    let mut file_words = pairs(
        conn,
        "SELECT owner_id, word_id FROM occurrence
         WHERE owner_kind = 'file' AND source = 'path' ORDER BY owner_id, word_id",
    )?;
    let module_words = pairs(
        conn,
        "SELECT s.file_id, o.word_id FROM occurrence o JOIN symbol s ON s.id = o.owner_id
         WHERE o.owner_kind = 'symbol' AND o.source = 'name' AND s.kind = 'module'
         ORDER BY s.file_id, o.word_id",
    )?;
    for (file, words) in module_words {
        file_words.entry(file).or_default().extend(words);
    }
    let file_context: BTreeMap<i64, String> = conn
        .prepare("SELECT id, context FROM file WHERE context IS NOT NULL ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let imported = pairs(
        conn,
        "SELECT owner_id, word_id FROM occurrence WHERE owner_kind = 'import' ORDER BY owner_id, word_id",
    )?;
    let texts: BTreeMap<i64, String> = conn
        .prepare("SELECT id, text FROM word ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(Corpus {
        file_names,
        file_words,
        file_context,
        imported,
        texts,
        prefixes: qname_prefixes(conn)?,
    })
}

fn import_bridges(
    conn: &Connection,
    corpus: &Corpus,
    word_id: i64,
    w: &BridgeWeights,
    acc: &mut Acc,
) -> Result<(), CoreError> {
    let word = corpus.texts[&word_id].as_str();
    let imports: Vec<(i64, i64, String)> = conn
        .prepare_cached("SELECT rowid, file_id, target FROM import ORDER BY rowid")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let empty = BTreeSet::new();
    for (rowid, file_id, target) in imports {
        let imported = corpus.imported.get(&rowid).unwrap_or(&empty);
        let in_file = corpus.file_words.get(&file_id).unwrap_or(&empty);
        let (word_in_file, word_imported) =
            (in_file.contains(&word_id), imported.contains(&word_id));
        if !word_in_file && !word_imported {
            continue;
        }
        let weight = if corpus.prefixes.contains(&target) {
            w.import
        } else {
            w.import_external
        };
        if word_in_file {
            for other in imported {
                add(acc, BridgeKind::Import, &corpus.texts[other], word, weight);
            }
        }
        if word_imported {
            for other in in_file {
                add(acc, BridgeKind::Import, &corpus.texts[other], word, weight);
            }
        }
    }
    Ok(())
}

/// Arquivo: palavras de símbolos do mesmo arquivo, `file` por arquivo em comum. Contexto: o mesmo
/// para os arquivos do mesmo contexto, `context` por contexto em comum.
fn file_and_context_bridges(corpus: &Corpus, word_id: i64, w: &BridgeWeights, acc: &mut Acc) {
    let word = corpus.texts[&word_id].as_str();
    let mut by_file: BTreeMap<i64, usize> = BTreeMap::new();
    let mut contexts: BTreeMap<&str, BTreeSet<i64>> = BTreeMap::new();
    let mut files_with_word: BTreeSet<&str> = BTreeSet::new();
    for (file, words) in &corpus.file_names {
        if !words.contains(&word_id) {
            continue;
        }
        for other in words.iter().filter(|o| **o != word_id) {
            *by_file.entry(*other).or_insert(0) += 1;
        }
        if let Some(context) = corpus.file_context.get(file) {
            files_with_word.insert(context.as_str());
        }
    }
    for (other, count) in by_file {
        add(
            acc,
            BridgeKind::File,
            &corpus.texts[&other],
            word,
            w.file * count as f64,
        );
    }
    for (file, words) in &corpus.file_names {
        if let Some(context) = corpus.file_context.get(file)
            && files_with_word.contains(context.as_str())
        {
            contexts.entry(context.as_str()).or_default().extend(words);
        }
    }
    let mut by_context: BTreeMap<i64, usize> = BTreeMap::new();
    for words in contexts.values() {
        for other in words.iter().filter(|o| **o != word_id) {
            *by_context.entry(*other).or_insert(0) += 1;
        }
    }
    for (other, count) in by_context {
        add(
            acc,
            BridgeKind::Context,
            &corpus.texts[&other],
            word,
            w.context * count as f64,
        );
    }
}

/// Vizinhas de uma palavra por tipo de ponte, da mais pesada para a mais leve (desempate: tipo,
/// depois ordem alfabética). Palavra desconhecida devolve lista vazia.
pub fn word_bridges(
    store: &SqliteStore,
    word: &str,
    weights: &BridgeWeights,
) -> Result<Vec<Bridge>, CoreError> {
    let conn = &store.conn;
    let word = word.to_lowercase();
    let Some(word_id): Option<i64> = conn
        .query_row("SELECT id FROM word WHERE text = ?1", [&word], |r| r.get(0))
        .ok()
    else {
        return Ok(Vec::new());
    };
    let mut acc = Acc::new();
    symbol_bridges(conn, &word, word_id, weights, &mut acc)?;
    call_bridges(conn, &word, word_id, weights, &mut acc)?;
    let corpus = load_corpus(conn)?;
    import_bridges(conn, &corpus, word_id, weights, &mut acc)?;
    file_and_context_bridges(&corpus, word_id, weights, &mut acc);
    let mut out: Vec<Bridge> = acc
        .into_iter()
        .map(|((kind, other), weight)| Bridge {
            other,
            kind,
            weight,
        })
        .collect();
    out.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.other.cmp(&b.other))
    });
    Ok(out)
}

type Neighborhoods = BTreeMap<i64, BTreeMap<i64, f64>>;

fn add_pair(out: &mut Neighborhoods, x: i64, y: i64, weight: f64) {
    *out.entry(x).or_default().entry(y).or_insert(0.0) += weight;
    *out.entry(y).or_default().entry(x).or_insert(0.0) += weight;
}

/// Liga cada palavra de `a` a cada palavra de `b` (as que coincidem ficam de fora).
fn link(out: &mut Neighborhoods, a: &BTreeSet<i64>, b: &BTreeSet<i64>, weight: f64) {
    if weight <= 0.0 {
        return;
    }
    for &x in a {
        for &y in b {
            if x != y {
                add_pair(out, x, y, weight);
            }
        }
    }
}

/// Vizinhança de todas as palavras pelas pontes de símbolo, teste, import e chamada, em lote:
/// `palavra -> (outra palavra -> peso)`. Arquivo e contexto ficam de fora (formariam pares demais).
///
/// Usa as mesmas regras de `word_bridges`; a ordem de soma é estável (mapas ordenados).
pub(crate) fn core_neighborhoods(
    conn: &Connection,
    w: &BridgeWeights,
) -> Result<BTreeMap<i64, BTreeMap<i64, f64>>, CoreError> {
    let words_of = |sql: &str| -> Result<BTreeMap<i64, BTreeSet<i64>>, CoreError> {
        let mut map: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (owner, word) = row?;
            map.entry(owner).or_default().insert(word);
        }
        Ok(map)
    };
    let names = words_of(
        "SELECT owner_id, word_id FROM occurrence
         WHERE owner_kind = 'symbol' AND source = 'name' ORDER BY owner_id, word_id",
    )?;
    let docs = words_of(
        "SELECT owner_id, word_id FROM occurrence
         WHERE owner_kind = 'symbol' AND source = 'test_doc' ORDER BY owner_id, word_id",
    )?;
    let imported = words_of(
        "SELECT owner_id, word_id FROM occurrence WHERE owner_kind = 'import' ORDER BY owner_id, word_id",
    )?;
    let mut file_words = words_of(
        "SELECT owner_id, word_id FROM occurrence
         WHERE owner_kind = 'file' AND source = 'path' ORDER BY owner_id, word_id",
    )?;
    let module_words: Vec<(i64, i64)> = conn
        .prepare(
            "SELECT s.file_id, o.word_id FROM symbol s JOIN occurrence o
               ON o.owner_kind = 'symbol' AND o.owner_id = s.id AND o.source = 'name'
             WHERE s.kind = 'module' ORDER BY s.file_id, o.word_id",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (file, word) in module_words {
        file_words.entry(file).or_default().insert(word);
    }

    let mut out = Neighborhoods::new();
    // No mesmo símbolo cada par entra uma vez, nos dois sentidos.
    for words in names.values() {
        let v: Vec<i64> = words.iter().copied().collect();
        for (i, &x) in v.iter().enumerate() {
            for &y in &v[i + 1..] {
                add_pair(&mut out, x, y, w.symbol);
            }
        }
    }
    let empty = BTreeSet::new();
    for call in calls(conn)? {
        let src_name = names.get(&call.src).unwrap_or(&empty);
        let dst_name = names.get(&call.dst).unwrap_or(&empty);
        if call.src_is_test {
            if call.approx {
                continue;
            }
            let mut src_all = src_name.clone();
            src_all.extend(docs.get(&call.src).unwrap_or(&empty));
            link(&mut out, &src_all, dst_name, w.test);
        } else {
            let factor = if call.approx { w.approximate } else { 1.0 };
            link(&mut out, src_name, dst_name, w.call * factor);
        }
    }

    let mut prefixes: HashSet<String> = HashSet::new();
    let qnames: Vec<String> = column(conn, "SELECT qname FROM symbol", [])?;
    for qname in &qnames {
        let mut end = 0;
        for part in qname.split('.') {
            end += part.len();
            prefixes.insert(qname[..end].to_string());
            end += 1;
        }
    }
    let imports: Vec<(i64, i64, String)> = conn
        .prepare("SELECT rowid, file_id, target FROM import ORDER BY rowid")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (rowid, file_id, target) in imports {
        let weight = if prefixes.contains(&target) {
            w.import
        } else {
            w.import_external
        };
        if let (Some(imp), Some(file)) = (imported.get(&rowid), file_words.get(&file_id)) {
            link(&mut out, file, imp, weight);
        }
    }
    Ok(out)
}
