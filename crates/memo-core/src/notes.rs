//! Notas ancoradas: um arquivo Markdown por nota, a única fonte de verdade (spec §7).

use crate::error::CoreError;
use crate::query::{resolve_qname, word_stats};
use crate::store::SqliteStore;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const MAX_TEXT: usize = 1200;
const MAX_CONFLICTS: usize = 5;
const SUMMARY_CHARS: usize = 120;
const HASH_CHARS: usize = 12;

const TIPOS: &[&str] = &["decisao", "regra", "restricao", "descartado", "glossario"];
const ORIGENS: &[&str] = &["humano", "recomendacao_aceita", "agente"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Note {
    pub id: String,
    pub tipo: String,
    pub estado: String,
    pub escopo: String,
    pub ancora: String,
    pub secundarias: Vec<String>,
    pub origem: String,
    pub task: Option<String>,
    pub ancora_sig: Option<String>,
    pub ancora_body: Option<String>,
    pub criada: String,
    pub substitui: Option<String>,
    pub estado_anterior: Option<String>,
    pub aliases: Vec<String>,
    pub text: String,
}

fn list_value(values: &[String]) -> String {
    format!("[{}]", values.join(", "))
}

fn parse_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

impl Note {
    /// Markdown com frontmatter, como gravado em `notes/<id>.md`.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        let mut field = |key: &str, value: &str| {
            out.push_str(&format!("{key}: {value}\n"));
        };
        field("id", &self.id);
        field("tipo", &self.tipo);
        field("estado", &self.estado);
        field("escopo", &self.escopo);
        field("ancora", &self.ancora);
        if !self.secundarias.is_empty() {
            field("secundarias", &list_value(&self.secundarias));
        }
        field("origem", &self.origem);
        if let Some(task) = &self.task {
            field("task", task);
        }
        if let Some(sig) = &self.ancora_sig {
            field("ancora_sig", sig);
        }
        if let Some(body) = &self.ancora_body {
            field("ancora_body", body);
        }
        field("criada", &self.criada);
        if let Some(old) = &self.substitui {
            field("substitui", old);
        }
        if let Some(prev) = &self.estado_anterior {
            field("estado_anterior", prev);
        }
        if !self.aliases.is_empty() {
            field("aliases", &list_value(&self.aliases));
        }
        out.push_str("---\n");
        out.push_str(&self.text);
        out.push('\n');
        out
    }

    pub fn parse(text: &str) -> Result<Note, CoreError> {
        let bad = |why: &str| CoreError::Note(why.to_string());
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| bad("missing frontmatter"))?;
        let (front, body) = rest
            .split_once("\n---\n")
            .or_else(|| rest.strip_suffix("\n---").map(|f| (f, "")))
            .ok_or_else(|| bad("unterminated frontmatter"))?;
        let mut note = Note {
            id: String::new(),
            tipo: String::new(),
            estado: String::new(),
            escopo: String::new(),
            ancora: String::new(),
            secundarias: Vec::new(),
            origem: String::new(),
            task: None,
            ancora_sig: None,
            ancora_body: None,
            criada: String::new(),
            substitui: None,
            estado_anterior: None,
            aliases: Vec::new(),
            text: body.strip_suffix('\n').unwrap_or(body).to_string(),
        };
        for line in front.lines() {
            let Some((key, value)) = line.split_once(':') else {
                return Err(bad("malformed frontmatter line"));
            };
            let value = value.trim();
            let own = || value.to_string();
            match key.trim() {
                "id" => note.id = own(),
                "tipo" => note.tipo = own(),
                "estado" => note.estado = own(),
                "escopo" => note.escopo = own(),
                "ancora" => note.ancora = own(),
                "secundarias" => note.secundarias = parse_list(value),
                "origem" => note.origem = own(),
                "task" => note.task = Some(own()),
                "ancora_sig" => note.ancora_sig = Some(own()),
                "ancora_body" => note.ancora_body = Some(own()),
                "criada" => note.criada = own(),
                "substitui" => note.substitui = Some(own()),
                "estado_anterior" => note.estado_anterior = Some(own()),
                "aliases" => note.aliases = parse_list(value),
                _ => {}
            }
        }
        let required = [
            &note.id,
            &note.tipo,
            &note.estado,
            &note.escopo,
            &note.ancora,
            &note.origem,
            &note.criada,
        ];
        if required.iter().any(|v| v.is_empty()) {
            return Err(bad("missing required field"));
        }
        Ok(note)
    }

    /// Linha curta: `id tipo estado  âncora: texto` (texto cortado em 120 caracteres).
    pub fn summary(&self) -> String {
        let text: String = if self.text.chars().count() > SUMMARY_CHARS {
            let cut: String = self.text.chars().take(SUMMARY_CHARS).collect();
            format!("{cut}…")
        } else {
            self.text.replace('\n', " ")
        };
        format!(
            "{} {} {}  {}: {text}",
            self.id,
            self.tipo,
            self.estado,
            short_anchor(&self.ancora)
        )
    }
}

/// Âncora legível: `Classe.metodo` ou o último segmento do `qname`.
fn short_anchor(anchor: &str) -> String {
    let short = |q: &str| {
        let parts: Vec<&str> = q.split('.').collect();
        match parts.as_slice() {
            [.., class, name] if class.chars().next().is_some_and(char::is_uppercase) => {
                format!("{class}.{name}")
            }
            [.., name] => (*name).to_string(),
            [] => q.to_string(),
        }
    };
    match anchor.split_once(':') {
        Some(("sym", q)) => short(q),
        Some(("edge", e)) => e.split("->").map(short).collect::<Vec<_>>().join("->"),
        Some((_, rest)) => rest.to_string(),
        None => anchor.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    pub tipo: String,
    pub ancora: String,
    pub secundarias: Vec<String>,
    pub origem: String,
    pub text: String,
    pub task: Option<String>,
    pub confirm: bool,
    pub substitui: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddOutcome {
    pub note: Note,
    pub conflicts: Vec<Note>,
}

/// Peso na recuperação: confirmada 1,0; proposta 0,5; stale 0,3; demais 0 (spec §7.4).
pub fn weight(estado: &str) -> f64 {
    match estado {
        "confirmada" => 1.0,
        "proposta" => 0.5,
        "stale" => 0.3,
        _ => 0.0,
    }
}

fn is_active(note: &Note) -> bool {
    weight(&note.estado) > 0.0
}

/// Notas ativas ligadas a algum dos nós, por peso e depois por id.
pub fn notes_for(
    notes: &[Note],
    qnames: &[String],
    words: &[String],
    files: &[String],
) -> Vec<Note> {
    let hits = |anchor: &str| match anchor.split_once(':') {
        Some(("sym", q)) => qnames.iter().any(|n| n == q),
        Some(("word", w)) => words.iter().any(|n| n == w),
        Some(("file", f)) => files.iter().any(|n| n == f),
        Some(("edge", e)) => e.split("->").any(|q| qnames.iter().any(|n| n == q)),
        _ => false,
    };
    let mut found: Vec<Note> = notes
        .iter()
        .filter(|n| is_active(n))
        .filter(|n| hits(&n.ancora) || n.secundarias.iter().any(|a| hits(a)))
        .cloned()
        .collect();
    found.sort_by(|a, b| {
        weight(&b.estado)
            .total_cmp(&weight(&a.estado))
            .then_with(|| a.id.cmp(&b.id))
    });
    found
}

struct Resolved {
    anchor: String,
    sig: Option<String>,
    body: Option<String>,
}

fn exists(graph: &SqliteStore, sql: &str, param: &str) -> Result<bool, CoreError> {
    Ok(graph.conn.query_row(sql, [param], |r| r.get(0))?)
}

/// Confere que a âncora existe no grafo e a normaliza (spec §7.2, regras 1 e 2).
fn resolve_anchor(graph: &SqliteStore, raw: &str) -> Result<Resolved, CoreError> {
    let (kind, rest) = raw.split_once(':').ok_or_else(|| CoreError::Validation {
        code: "ancora_invalida",
        message: format!("anchor needs a kind prefix (sym|word|edge|file): {raw}"),
    })?;
    let plain = |anchor: String| Resolved {
        anchor,
        sig: None,
        body: None,
    };
    match kind {
        "sym" => {
            let qname = resolve_qname(graph, rest)?;
            let (sig, body): (Option<String>, Option<String>) = graph.conn.query_row(
                "SELECT sig_hash, body_hash FROM symbol WHERE qname = ?1",
                [&qname],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let cut = |h: Option<String>| h.map(|h| h.chars().take(HASH_CHARS).collect());
            Ok(Resolved {
                anchor: format!("sym:{qname}"),
                sig: cut(sig),
                body: cut(body),
            })
        }
        "word" => {
            let word = rest.to_lowercase();
            if !exists(
                graph,
                "SELECT EXISTS(SELECT 1 FROM word WHERE text = ?1)",
                &word,
            )? {
                return Err(CoreError::NotFound(format!("word:{word}")));
            }
            if word_stats(graph, &word)?.is_some_and(|s| s.esqueleto) {
                return Err(CoreError::Validation {
                    code: "ancora_generica",
                    message: format!("word '{word}' is skeleton and cannot anchor a note"),
                });
            }
            Ok(plain(format!("word:{word}")))
        }
        "edge" => {
            let (from, to) = rest.split_once("->").ok_or_else(|| CoreError::Validation {
                code: "ancora_invalida",
                message: format!("edge anchor must be edge:<qname>-><qname>: {raw}"),
            })?;
            let (from, to) = (resolve_qname(graph, from)?, resolve_qname(graph, to)?);
            let linked: bool = graph.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM edge e JOIN symbol a ON a.id = e.src
                 JOIN symbol b ON b.id = e.dst WHERE a.qname = ?1 AND b.qname = ?2)",
                [&from, &to],
                |r| r.get(0),
            )?;
            if !linked {
                return Err(CoreError::NotFound(format!("edge:{from}->{to}")));
            }
            Ok(plain(format!("edge:{from}->{to}")))
        }
        "file" => {
            if !exists(
                graph,
                "SELECT EXISTS(SELECT 1 FROM file WHERE path = ?1)",
                rest,
            )? {
                return Err(CoreError::NotFound(format!("file:{rest}")));
            }
            Ok(plain(format!("file:{rest}")))
        }
        _ => Err(CoreError::Validation {
            code: "ancora_invalida",
            message: format!("unknown anchor kind '{kind}'"),
        }),
    }
}

fn validate_choice(value: &str, allowed: &[&str], code: &'static str) -> Result<(), CoreError> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(CoreError::Validation {
        code,
        message: format!("'{value}' is not one of {}", allowed.join(", ")),
    })
}

fn base36_id(seed: &str) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let hash = blake3::hash(seed.as_bytes());
    let id: String = hash
        .as_bytes()
        .iter()
        .take(8)
        .map(|b| DIGITS[usize::from(*b) % DIGITS.len()] as char)
        .collect();
    format!("n-{id}")
}

fn valid_id(id: &str) -> bool {
    id.strip_prefix("n-")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub struct NoteDir {
    dir: PathBuf,
}

impl NoteDir {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.md"))
    }

    /// Grava por arquivo temporário e rename: uma interrupção nunca deixa `.md` truncado.
    fn write(&self, note: &Note) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{}.md.tmp", note.id));
        std::fs::write(&tmp, note.render())?;
        std::fs::rename(&tmp, self.path(&note.id))?;
        Ok(())
    }

    /// Valida e grava a nota (spec §7.2); `now` é o carimbo `criada`.
    pub fn add(
        &self,
        graph: &SqliteStore,
        new: NewNote,
        now: &str,
    ) -> Result<AddOutcome, CoreError> {
        validate_choice(&new.tipo, TIPOS, "tipo_invalido")?;
        validate_choice(&new.origem, ORIGENS, "origem_invalida")?;
        let text = new.text.trim().to_string();
        if text.is_empty() {
            return Err(CoreError::Validation {
                code: "texto_vazio",
                message: "note text is empty".into(),
            });
        }
        if text.chars().count() > MAX_TEXT {
            return Err(CoreError::Validation {
                code: "texto_longo",
                message: format!("note text exceeds {MAX_TEXT} characters"),
            });
        }
        let main = resolve_anchor(graph, &new.ancora)?;
        let secondary = new
            .secundarias
            .iter()
            .map(|a| resolve_anchor(graph, a).map(|r| r.anchor))
            .collect::<Result<Vec<_>, _>>()?;
        let replaced = new
            .substitui
            .as_deref()
            .map(|id| self.get(id))
            .transpose()?;

        let existing = self.list()?;
        let conflicts: Vec<Note> = existing
            .iter()
            .filter(|n| n.ancora == main.anchor && is_active(n))
            .take(MAX_CONFLICTS)
            .cloned()
            .collect();

        let mut attempt = 0;
        let id = loop {
            let id = base36_id(&format!("{}|{text}|{now}|{attempt}", main.anchor));
            if !self.path(&id).exists() {
                break id;
            }
            attempt += 1;
        };
        let note = Note {
            id,
            tipo: new.tipo,
            estado: if new.confirm {
                "confirmada"
            } else {
                "proposta"
            }
            .to_string(),
            escopo: new
                .task
                .as_ref()
                .map_or("repo".to_string(), |t| format!("task:{t}")),
            ancora: main.anchor,
            secundarias: secondary,
            origem: new.origem,
            task: new.task,
            ancora_sig: main.sig,
            ancora_body: main.body,
            criada: now.to_string(),
            substitui: new.substitui,
            estado_anterior: None,
            aliases: new.aliases,
            text,
        };
        self.write(&note)?;
        if let Some(mut old) = replaced {
            old.estado = "substituida".to_string();
            self.write(&old)?;
        }
        Ok(AddOutcome { note, conflicts })
    }

    /// Todas as notas, por id; arquivos temporários e estranhos são ignorados.
    pub fn list(&self) -> Result<Vec<Note>, CoreError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut notes = Vec::new();
        for entry in entries {
            let path = entry?.path();
            let is_note = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("n-") && n.ends_with(".md"));
            if !is_note {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let note = Note::parse(&text)
                .map_err(|e| CoreError::Note(format!("{}: {e}", path.display())))?;
            notes.push(note);
        }
        notes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(notes)
    }

    pub fn get(&self, id: &str) -> Result<Note, CoreError> {
        if !valid_id(id) {
            return Err(CoreError::NotFound(id.to_string()));
        }
        match std::fs::read_to_string(self.path(id)) {
            Ok(text) => Note::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NotFound(id.to_string()))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// `proposta` vira `confirmada`; `confirmada` segue igual; outros estados não confirmam.
    pub fn confirm(&self, id: &str) -> Result<Note, CoreError> {
        let mut note = self.get(id)?;
        match note.estado.as_str() {
            "confirmada" => Ok(note),
            "proposta" => {
                note.estado = "confirmada".to_string();
                self.write(&note)?;
                Ok(note)
            }
            other => Err(CoreError::Validation {
                code: "estado_invalido",
                message: format!("a note in state '{other}' cannot be confirmed"),
            }),
        }
    }

    /// Como `confirm`, e grava `NOTE_CONFIRMED`.
    pub fn confirm_recorded(&self, _graph: &SqliteStore, id: &str) -> Result<Note, CoreError> {
        self.confirm(id)
    }

    /// Atualiza os hashes da âncora e volta ao estado anterior ao `stale` (spec §7.3).
    pub fn recheck(&self, _graph: &SqliteStore, _id: &str) -> Result<Note, CoreError> {
        todo!("spec §7.3 recheck")
    }

    /// Notas de escopo `repo` e, com `task`, as de `task:<id>`; em qualquer estado.
    pub fn visible(&self, task: Option<&str>) -> Result<Vec<Note>, CoreError> {
        let scoped = task.map(|t| format!("task:{t}"));
        Ok(self
            .list()?
            .into_iter()
            .filter(|n| n.escopo == "repo" || scoped.as_deref() == Some(n.escopo.as_str()))
            .collect())
    }
}

/// Carimbo ISO 8601 em UTC (`2026-11-07T14:05:00Z`) para os segundos desde a época Unix.
pub fn format_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let rem = epoch_secs.rem_euclid(86_400);
    // Conversão de dias para data civil (algoritmo de Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
