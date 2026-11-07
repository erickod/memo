//! Adaptadores de harness para `memo init` (spec §9.2).
//!
//! Formatos conferidos na documentação vigente em 2026-11-08:
//! - Claude Code: `CLAUDE.md` e `.mcp.json` (`mcpServers`, `type: "stdio"`, `command`, `args`):
//!   <https://code.claude.com/docs/en/mcp-quickstart> e <https://code.claude.com/docs/en/mcp>.
//! - Codex: `AGENTS.md` (<https://developers.openai.com/codex/guides/agents-md>) e servidores
//!   MCP em tabelas `[mcp_servers.<nome>]` do `config.toml`, no projeto em `.codex/config.toml`
//!   (<https://developers.openai.com/codex/mcp>, <https://developers.openai.com/codex/config-reference>).

use std::path::{Path, PathBuf};

pub const BEGIN: &str = "<!-- memo:begin -->";
pub const END: &str = "<!-- memo:end -->";

/// Bloco de instruções, fixo e versionado com o binário (spec §9.3).
pub fn instruction_block() -> String {
    todo!("spec §9.3")
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("unknown harness '{0}' (supported: claude, codex)")]
    Unknown(String),
    #[error("harness '{0}' is not supported: its config format is not documented")]
    Unsupported(String),
    #[error("{path}: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InstallOpts {
    /// Registra também o servidor `memo serve` na config MCP.
    pub mcp: bool,
    /// Dry-run: devolve as mudanças sem gravar nada.
    pub print: bool,
}

/// Um arquivo que a instalação grava (ou gravaria).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub after: String,
    /// `false` quando o arquivo já tinha exatamente este conteúdo.
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

pub trait HarnessAdapter {
    fn id(&self) -> &'static str;
    /// O harness parece estar em uso no repositório.
    fn detect(&self, root: &Path) -> bool;
    fn install(&self, root: &Path, opts: &InstallOpts) -> Result<Vec<Change>, HarnessError>;
    fn uninstall(&self, root: &Path) -> Result<Vec<Change>, HarnessError>;
    fn doctor(&self, root: &Path) -> Vec<Check>;
}

/// O adaptador do harness; `agy` e desconhecidos dão erro claro, nunca um palpite.
pub fn adapter(id: &str) -> Result<Box<dyn HarnessAdapter>, HarnessError> {
    let _ = id;
    todo!("spec §9.2")
}
