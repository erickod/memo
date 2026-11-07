//! Notas ancoradas: um arquivo Markdown por nota, a única fonte de verdade (spec §7).

use crate::error::CoreError;
use crate::store::SqliteStore;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const MAX_TEXT: usize = 1200;

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

impl Note {
    /// Markdown com frontmatter, como gravado em `notes/<id>.md`.
    pub fn render(&self) -> String {
        todo!("spec §7")
    }

    pub fn parse(_text: &str) -> Result<Note, CoreError> {
        todo!("spec §7")
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
pub fn weight(_estado: &str) -> f64 {
    todo!("spec §7.4")
}

/// Notas ativas ligadas a algum dos nós, por peso e depois por id.
pub fn notes_for(
    _notes: &[Note],
    _qnames: &[String],
    _words: &[String],
    _files: &[String],
) -> Vec<Note> {
    todo!("spec §7.4")
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

    /// Valida e grava a nota (spec §7.2); `now` é o carimbo `criada`.
    pub fn add(
        &self,
        _graph: &SqliteStore,
        _new: NewNote,
        _now: &str,
    ) -> Result<AddOutcome, CoreError> {
        let _ = &self.dir;
        todo!("spec §7.2")
    }

    /// Todas as notas, por id; arquivos temporários e estranhos são ignorados.
    pub fn list(&self) -> Result<Vec<Note>, CoreError> {
        todo!("spec §7")
    }

    pub fn get(&self, _id: &str) -> Result<Note, CoreError> {
        todo!("spec §7")
    }

    pub fn confirm(&self, _id: &str) -> Result<Note, CoreError> {
        todo!("spec §7.2")
    }

    /// Notas de escopo `repo` e, com `task`, as de `task:<id>`; em qualquer estado.
    pub fn visible(&self, _task: Option<&str>) -> Result<Vec<Note>, CoreError> {
        todo!("spec §7.1")
    }
}

/// Carimbo ISO 8601 em UTC (`2026-11-07T14:05:00Z`) para os segundos desde a época Unix.
pub fn format_utc(_epoch_secs: i64) -> String {
    todo!("spec §7")
}
