//! `brief`: âncoras, símbolos, contorno, padrão, escritores e notas para um pedido (spec §8.5).

use crate::delta::filter_new;
use crate::error::CoreError;
use crate::notes::{Note, notes_for};
use crate::query::{SymbolAddr, addr_by_id, short_name, word_info};
use crate::store::SqliteStore;
use serde::Serialize;
use std::collections::BTreeMap;

const MAX_ANCHORS: usize = 5;
const MAX_SYMBOLS: usize = 15;
const OUTLINE_SYMBOLS: usize = 5;
const MIN_PREFIX: usize = 5;
const HOP_FACTOR: f64 = 0.5;

const STOPWORDS: &[&str] = &[
    // pt
    "de", "da", "do", "das", "dos", "em", "na", "no", "nas", "nos", "um", "uma", "uns", "umas",
    "os", "as", "ao", "aos", "pelo", "pela", "pelos", "pelas", "por", "para", "com", "sem", "que",
    "se", "ou", "ser", "ter", "mais", "como", "mas", "nao", "sao", "ha", "foi", "isso", "esse",
    "essa", "este", "esta", "seu", "sua", "ele", "ela", "ja", "so", // en
    "the", "an", "of", "to", "in", "on", "at", "for", "with", "and", "is", "are", "be", "it",
    "this", "that", "by", "from", "as", "not", "or", "was", "were", "has", "have", "its", "into",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Anchor {
    pub word: String,
    /// Peso normalizado pelo maior (o primeiro vale 1,0).
    pub score: f64,
    /// Casou só por prefixo (`~`).
    pub approx: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoredSymbol {
    pub addr: SymbolAddr,
    pub score: f64,
}

/// Uma linha do contorno: arestas de chamada de um símbolo listado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outline {
    pub symbol: String,
    /// `-> nome :Llinha` (chama) e `<- nome :Llinha` (é chamado), com `~` quando aproximada.
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub word: String,
    pub verbs: Vec<String>,
    pub roles: Vec<String>,
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriterLine {
    pub attr: String,
    pub writers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Brief {
    pub request: String,
    pub anchors: Vec<Anchor>,
    pub ignored: Vec<String>,
    pub symbols: Vec<ScoredSymbol>,
    pub outline: Vec<Outline>,
    pub pattern: Option<Pattern>,
    pub writers: Vec<WriterLine>,
    pub notes: Vec<String>,
    /// Itens omitidos por já terem sido entregues nesta task (spec §8.4).
    pub unchanged: Option<usize>,
}

fn strip_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

/// Minúsculas, sem acentos, separado por não letras, sem stopwords pt e en.
pub fn tokenize(text: &str) -> Vec<String> {
    let folded: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphabetic() {
                strip_accent(c)
            } else {
                ' '
            }
        })
        .collect();
    let mut tokens: Vec<String> = Vec::new();
    for token in folded.split_whitespace() {
        let keep = token.chars().count() >= 2 && !STOPWORDS.contains(&token);
        if keep && !tokens.iter().any(|t| t == token) {
            tokens.push(token.to_string());
        }
    }
    tokens
}

struct Candidate {
    id: i64,
    text: String,
    approx: bool,
    weight: f64,
    skeleton: bool,
}

fn slot_bonus(slot: &str) -> f64 {
    match slot {
        "object" => 1.0,
        "action" => 0.7,
        _ => 0.2,
    }
}

/// Casa cada token com uma palavra: igualdade exata, ou prefixo de ao menos 5 caracteres.
fn match_words(store: &SqliteStore, tokens: &[String]) -> Result<Vec<Candidate>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT w.id, w.text, COALESCE(ws.especificidade, 0), COALESCE(ws.centralidade, 0),
                COALESCE(ws.esqueleto, 0)
         FROM word w LEFT JOIN word_stats ws ON ws.word_id = w.id ORDER BY w.text",
    )?;
    let words: Vec<(i64, String, f64, bool)> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, f64>(2)? * r.get::<_, f64>(3)?,
                r.get::<_, i64>(4)? != 0,
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut found: Vec<Candidate> = Vec::new();
    for token in tokens {
        let exact = words.iter().any(|w| &w.1 == token);
        for (id, text, weight, skeleton) in &words {
            let hit = if exact {
                text == token
            } else {
                let (short, long) = if text.len() <= token.len() {
                    (text, token)
                } else {
                    (token, text)
                };
                short.len() >= MIN_PREFIX && long.starts_with(short.as_str())
            };
            if !hit {
                continue;
            }
            match found.iter_mut().find(|c| c.id == *id) {
                Some(existing) => existing.approx &= !exact,
                None => found.push(Candidate {
                    id: *id,
                    text: text.clone(),
                    approx: !exact,
                    weight: *weight,
                    skeleton: *skeleton,
                }),
            }
        }
    }
    Ok(found)
}

fn neighbors(store: &SqliteStore, id: i64) -> Result<Vec<i64>, CoreError> {
    let mut stmt = store.conn.prepare(
        "SELECT dst FROM edge WHERE src = ?1 AND dst IS NOT NULL AND kind IN ('CHAMA', 'RECEBE', 'RETORNA')
         UNION
         SELECT src FROM edge WHERE dst = ?1 AND kind IN ('CHAMA', 'RECEBE', 'RETORNA')
         ORDER BY 1",
    )?;
    let ids = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn outline_for(store: &SqliteStore, id: i64) -> Result<Vec<String>, CoreError> {
    let mut links = Vec::new();
    for (arrow, near, far) in [("->", "src", "dst"), ("<-", "dst", "src")] {
        let sql = format!(
            "SELECT o.qname, COALESCE(f.path, ''), e.line, e.approx FROM edge e
             JOIN symbol o ON o.id = e.{far} LEFT JOIN file f ON f.id = o.file_id
             WHERE e.{near} = ?1 AND e.kind IN ('CHAMA', 'CRIA') ORDER BY o.qname, e.line"
        );
        let mut stmt = store.conn.prepare(&sql)?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, i64>(3)? != 0,
            ))
        })?;
        for row in rows {
            let (qname, path, line, approx) = row?;
            let approx = if approx { " ~" } else { "" };
            let line = line.map(|l| format!(" :L{l}")).unwrap_or_default();
            links.push(format!(
                "{arrow} {}{approx}{line}",
                short_name(&qname, &path)
            ));
        }
    }
    Ok(links)
}

/// Atributos tocados (nos símbolos listados ou por `ESCREVE`/`LE` deles) e quem os escreve.
fn writers_of_touched(store: &SqliteStore, ids: &[i64]) -> Result<Vec<WriterLine>, CoreError> {
    let mut attrs: BTreeMap<String, i64> = BTreeMap::new();
    let mut touch = |id: i64| -> Result<(), CoreError> {
        let addr = addr_by_id(store, id)?;
        if addr.kind == "attribute" {
            attrs.insert(addr.qname, id);
        }
        Ok(())
    };
    for &id in ids {
        touch(id)?;
        let mut stmt = store.conn.prepare(
            "SELECT dst FROM edge WHERE src = ?1 AND dst IS NOT NULL AND kind IN ('ESCREVE', 'LE')",
        )?;
        let dsts: Vec<i64> = stmt
            .query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for dst in dsts {
            touch(dst)?;
        }
    }
    let mut lines = Vec::new();
    for id in attrs.into_values() {
        let mut stmt = store.conn.prepare(
            "SELECT DISTINCT e.src, s.qname FROM edge e JOIN symbol s ON s.id = e.src
             WHERE e.dst = ?1 AND e.kind = 'ESCREVE' ORDER BY s.qname",
        )?;
        let writers: Vec<i64> = stmt
            .query_map([id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        if writers.is_empty() {
            continue;
        }
        lines.push(WriterLine {
            attr: addr_by_id(store, id)?.short,
            writers: writers
                .into_iter()
                .map(|w| addr_by_id(store, w).map(|a| a.short))
                .collect::<Result<_, _>>()?,
        });
    }
    Ok(lines)
}

/// `brief` sem notas.
/// Troca tokens que são alias de uma nota `glossario` (âncora `word:`) pela palavra.
fn expand_aliases(tokens: Vec<String>, notes: &[Note]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let word = notes
            .iter()
            .filter(|n| n.tipo == "glossario" && n.estado != "orfa" && n.estado != "substituida")
            .filter_map(|n| n.ancora.strip_prefix("word:").map(|w| (n, w)))
            .find(|(n, _)| {
                n.aliases
                    .iter()
                    .any(|a| tokenize(a).first() == Some(&token))
            })
            .map(|(_, w)| w.to_string());
        let token = word.unwrap_or(token);
        if !out.contains(&token) {
            out.push(token);
        }
    }
    out
}

pub fn brief(store: &SqliteStore, request: &str) -> Result<Brief, CoreError> {
    brief_with(store, request, &[])
}

/// `brief` com as notas visíveis: o alias de nota `glossario` casa tokens e as notas dos nós
/// listados entram na seção `notas`.
pub fn brief_with(store: &SqliteStore, request: &str, notes: &[Note]) -> Result<Brief, CoreError> {
    let tokens = expand_aliases(tokenize(request), notes);
    let matched = match_words(store, &tokens)?;
    let mut ignored: Vec<String> = Vec::new();
    let mut usable: Vec<&Candidate> = Vec::new();
    for c in &matched {
        if c.skeleton {
            ignored.push(c.text.clone());
        } else {
            usable.push(c);
        }
    }
    usable.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then_with(|| a.text.cmp(&b.text))
    });
    usable.truncate(MAX_ANCHORS);
    let top = usable.first().map_or(0.0, |c| c.weight);
    let normalized = |w: f64| if top > 0.0 { w / top } else { 1.0 };
    let anchors: Vec<Anchor> = usable
        .iter()
        .map(|c| Anchor {
            word: c.text.clone(),
            score: normalized(c.weight),
            approx: c.approx,
        })
        .collect();

    let mut scores: BTreeMap<i64, f64> = BTreeMap::new();
    for c in &usable {
        let mut stmt = store
            .conn
            .prepare("SELECT symbol_id, slot FROM occurrence WHERE word_id = ?1")?;
        let rows = stmt.query_map([c.id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (symbol, slot) = row?;
            *scores.entry(symbol).or_insert(0.0) += normalized(c.weight) * slot_bonus(&slot);
        }
    }
    let direct = scores.clone();
    for (&id, &score) in &direct {
        for n in neighbors(store, id)? {
            *scores.entry(n).or_insert(0.0) += HOP_FACTOR * score;
        }
    }
    let mut ranked: Vec<(i64, f64, SymbolAddr)> = scores
        .into_iter()
        .map(|(id, s)| addr_by_id(store, id).map(|a| (id, s, a)))
        .collect::<Result<_, _>>()?;
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.2.qname.cmp(&b.2.qname)));
    ranked.truncate(MAX_SYMBOLS);

    let mut outline = Vec::new();
    for (id, _, addr) in ranked.iter().take(OUTLINE_SYMBOLS) {
        let links = outline_for(store, *id)?;
        if !links.is_empty() {
            outline.push(Outline {
                symbol: addr.short.clone(),
                links,
            });
        }
    }
    let ids: Vec<i64> = ranked.iter().map(|(id, _, _)| *id).collect();
    let ranked_addrs: Vec<SymbolAddr> = ranked.iter().map(|(_, _, a)| a.clone()).collect();
    let pattern = match anchors.first() {
        Some(a) => {
            let info = word_info(store, &a.word)?;
            Some(Pattern {
                word: a.word.clone(),
                verbs: info.verbs,
                roles: info.roles,
                layers: info.layers,
            })
        }
        None => None,
    };
    let notes_lines = related_notes(notes, &ranked_addrs, &anchors);
    Ok(Brief {
        request: request.to_string(),
        anchors,
        ignored,
        writers: writers_of_touched(store, &ids)?,
        symbols: ranked
            .into_iter()
            .map(|(_, score, addr)| ScoredSymbol { addr, score })
            .collect(),
        outline,
        pattern,
        notes: notes_lines,
        unchanged: None,
    })
}

/// Notas dos nós listados e das palavras âncora, como linhas curtas.
fn related_notes(notes: &[Note], symbols: &[SymbolAddr], anchors: &[Anchor]) -> Vec<String> {
    let qnames: Vec<String> = symbols.iter().map(|s| s.qname.clone()).collect();
    let files: Vec<String> = symbols.iter().map(|s| s.path.clone()).collect();
    let words: Vec<String> = anchors.iter().map(|a| a.word.clone()).collect();
    notes_for(notes, &qnames, &words, &files)
        .iter()
        .map(Note::summary)
        .collect()
}

/// Remove do `Brief` o que já foi entregue à `task`, registra o que ficou e preenche
/// `unchanged`; com `full` devolve tudo, e só registra.
pub fn apply_delta(
    store: &SqliteStore,
    task: &str,
    brief: &mut Brief,
    full: bool,
) -> Result<(), CoreError> {
    let request = brief.request.clone();
    let (symbols, a) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.symbols),
        |s| {
            format!(
                "{request}|S|{}|{}:{}-{}",
                s.addr.qname, s.addr.path, s.addr.line_start, s.addr.line_end
            )
        },
        full,
    )?;
    let (outline, b) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.outline),
        |o| format!("{request}|O|{}|{}", o.symbol, o.links.join(";")),
        full,
    )?;
    let (writers, c) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.writers),
        |w| format!("{request}|W|{}|{}", w.attr, w.writers.join(",")),
        full,
    )?;
    let (notes, d) = filter_new(
        store,
        task,
        "brief",
        std::mem::take(&mut brief.notes),
        |n| format!("{request}|N|{n}"),
        full,
    )?;
    brief.notes = notes;
    brief.symbols = symbols;
    brief.outline = outline;
    brief.writers = writers;
    brief.unchanged = Some(a + b + c + d);
    Ok(())
}
