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
    format!(
        "{BEGIN}\n\
## memo (memória estrutural)\n\
- Antes de começar uma tarefa: `memo brief \"<pedido>\"`. Leia só os trechos `caminho:Linicio-Lfim` indicados.\n\
- Antes de alterar a assinatura ou o comportamento de um símbolo: `memo query impact <símbolo>`.\n\
- `~` marca relação aproximada. Resultado vazio não prova ausência: confirme com busca no código.\n\
- Ao terminar, registre só conhecimento durável que o código não mostra, com `memo note add`: uma afirmação por nota, ancorada no símbolo mais específico. Não registre o que o diff já mostra.\n\
- Use `--task <id>` em toda chamada durante uma tarefa.\n\
{END}\n"
    )
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
    match id {
        "claude" => Ok(Box::new(Claude)),
        "codex" => Ok(Box::new(Codex)),
        "agy" => Err(HarnessError::Unsupported(id.to_string())),
        other => Err(HarnessError::Unknown(other.to_string())),
    }
}

const TOML_BEGIN: &str = "# memo:begin";
const TOML_END: &str = "# memo:end";

/// Troca o que está entre os marcadores (inclusive); sem marcadores, acrescenta ao fim.
fn apply_block(existing: &str, begin: &str, end: &str, block: &str) -> String {
    let block = block.trim_end();
    if let Some(start) = existing.find(begin)
        && let Some(stop) = existing[start..].find(end)
    {
        let after = &existing[start + stop + end.len()..];
        return format!("{}{block}{after}", &existing[..start]);
    }
    if existing.trim().is_empty() {
        return format!("{block}\n");
    }
    let sep = if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{existing}{sep}{block}\n")
}

/// Tira o bloco entre os marcadores, com a linha em branco que o precede.
fn remove_block(existing: &str, begin: &str, end: &str) -> String {
    let Some(start) = existing.find(begin) else {
        return existing.to_string();
    };
    let Some(stop) = existing[start..].find(end) else {
        return existing.to_string();
    };
    let before = existing[..start].trim_end_matches('\n');
    let after = existing[start + stop + end.len()..].trim_start_matches('\n');
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    }
}

fn read_opt(path: &Path) -> Result<Option<String>, HarnessError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn change(path: PathBuf, before: Option<&str>, after: String) -> Change {
    let changed = before != Some(after.as_str());
    Change {
        path,
        after,
        changed,
    }
}

/// Grava as mudanças que existem; conteúdo vazio remove o arquivo.
fn write_changes(changes: &[Change]) -> Result<(), HarnessError> {
    for c in changes.iter().filter(|c| c.changed) {
        if c.after.is_empty() {
            std::fs::remove_file(&c.path)?;
            continue;
        }
        if let Some(parent) = c.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&c.path, &c.after)?;
    }
    Ok(())
}

fn invalid(path: &Path, message: impl Into<String>) -> HarnessError {
    HarnessError::Invalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn memo_server() -> serde_json::Value {
    serde_json::json!({ "type": "stdio", "command": "memo", "args": ["serve"] })
}

/// `.mcp.json` com o servidor `memo`; mantém os outros e não reescreve se já está igual.
fn claude_mcp_after(path: &Path, existing: Option<&str>) -> Result<String, HarnessError> {
    let mut json: serde_json::Value = match existing {
        Some(text) => serde_json::from_str(text).map_err(|e| invalid(path, e.to_string()))?,
        None => serde_json::json!({}),
    };
    let servers = json
        .as_object_mut()
        .ok_or_else(|| invalid(path, "expected a JSON object"))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| invalid(path, "mcpServers must be an object"))?;
    if let (Some(text), true) = (existing, servers.get("memo") == Some(&memo_server())) {
        return Ok(text.to_string());
    }
    servers.insert("memo".to_string(), memo_server());
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json).map_err(|e| invalid(path, e.to_string()))?
    ))
}

fn claude_mcp_removed(path: &Path, text: &str) -> Result<String, HarnessError> {
    let mut json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| invalid(path, e.to_string()))?;
    let Some(root) = json.as_object_mut() else {
        return Ok(text.to_string());
    };
    if let Some(servers) = root.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        servers.remove("memo");
        if servers.is_empty() {
            root.remove("mcpServers");
        }
    }
    if root.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json).map_err(|e| invalid(path, e.to_string()))?
    ))
}

struct Claude;
struct Codex;

fn md_install(root: &Path, file: &str) -> Result<Change, HarnessError> {
    let path = root.join(file);
    let existing = read_opt(&path)?;
    let after = apply_block(
        existing.as_deref().unwrap_or(""),
        BEGIN,
        END,
        &instruction_block(),
    );
    Ok(change(path, existing.as_deref(), after))
}

fn md_uninstall(root: &Path, file: &str) -> Result<Option<Change>, HarnessError> {
    let path = root.join(file);
    let Some(existing) = read_opt(&path)? else {
        return Ok(None);
    };
    let after = remove_block(&existing, BEGIN, END);
    Ok(Some(change(path, Some(&existing), after)))
}

fn md_check(root: &Path, file: &str) -> Check {
    let current = read_opt(&root.join(file)).ok().flatten().is_some_and(|t| {
        let block = instruction_block();
        t.contains(block.trim_end())
    });
    Check {
        name: "instructions",
        ok: current,
        detail: if current {
            format!("{file} has the current memo block")
        } else {
            format!("{file} is missing the current memo block")
        },
    }
}

impl HarnessAdapter for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn detect(&self, root: &Path) -> bool {
        ["CLAUDE.md", ".claude", ".mcp.json"]
            .iter()
            .any(|p| root.join(p).exists())
    }

    fn install(&self, root: &Path, opts: &InstallOpts) -> Result<Vec<Change>, HarnessError> {
        let mut changes = vec![md_install(root, "CLAUDE.md")?];
        if opts.mcp {
            let path = root.join(".mcp.json");
            let existing = read_opt(&path)?;
            let after = claude_mcp_after(&path, existing.as_deref())?;
            changes.push(change(path, existing.as_deref(), after));
        }
        if !opts.print {
            write_changes(&changes)?;
        }
        Ok(changes)
    }

    fn uninstall(&self, root: &Path) -> Result<Vec<Change>, HarnessError> {
        let mut changes: Vec<Change> = md_uninstall(root, "CLAUDE.md")?.into_iter().collect();
        let path = root.join(".mcp.json");
        if let Some(existing) = read_opt(&path)? {
            let after = claude_mcp_removed(&path, &existing)?;
            changes.push(change(path, Some(&existing), after));
        }
        write_changes(&changes)?;
        Ok(changes)
    }

    fn doctor(&self, root: &Path) -> Vec<Check> {
        let registered = read_opt(&root.join(".mcp.json"))
            .ok()
            .flatten()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .is_some_and(|j| j["mcpServers"]["memo"] == memo_server());
        vec![
            md_check(root, "CLAUDE.md"),
            Check {
                name: "mcp",
                ok: registered,
                detail: ".mcp.json registers `memo serve`".to_string(),
            },
        ]
    }
}

fn codex_block() -> String {
    format!(
        "{TOML_BEGIN}\n[mcp_servers.memo]\ncommand = \"memo\"\nargs = [\"serve\"]\n{TOML_END}\n"
    )
}

/// A tabela `[mcp_servers.memo]` fora dos nossos marcadores é do usuário: não duplicar.
fn codex_conflict(path: &Path, text: &str) -> Result<(), HarnessError> {
    let outside = remove_block(text, TOML_BEGIN, TOML_END);
    if outside.contains("[mcp_servers.memo]") {
        return Err(invalid(
            path,
            "[mcp_servers.memo] already defined outside the memo markers",
        ));
    }
    Ok(())
}

impl HarnessAdapter for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn detect(&self, root: &Path) -> bool {
        ["AGENTS.md", ".codex"]
            .iter()
            .any(|p| root.join(p).exists())
    }

    fn install(&self, root: &Path, opts: &InstallOpts) -> Result<Vec<Change>, HarnessError> {
        let mut changes = vec![md_install(root, "AGENTS.md")?];
        if opts.mcp {
            let path = root.join(".codex/config.toml");
            let existing = read_opt(&path)?;
            codex_conflict(&path, existing.as_deref().unwrap_or(""))?;
            let after = apply_block(
                existing.as_deref().unwrap_or(""),
                TOML_BEGIN,
                TOML_END,
                &codex_block(),
            );
            changes.push(change(path, existing.as_deref(), after));
        }
        if !opts.print {
            write_changes(&changes)?;
        }
        Ok(changes)
    }

    fn uninstall(&self, root: &Path) -> Result<Vec<Change>, HarnessError> {
        let mut changes: Vec<Change> = md_uninstall(root, "AGENTS.md")?.into_iter().collect();
        let path = root.join(".codex/config.toml");
        if let Some(existing) = read_opt(&path)? {
            let after = remove_block(&existing, TOML_BEGIN, TOML_END);
            changes.push(change(path, Some(&existing), after));
        }
        write_changes(&changes)?;
        Ok(changes)
    }

    fn doctor(&self, root: &Path) -> Vec<Check> {
        let registered = read_opt(&root.join(".codex/config.toml"))
            .ok()
            .flatten()
            .is_some_and(|t| t.contains(codex_block().trim_end()));
        vec![
            md_check(root, "AGENTS.md"),
            Check {
                name: "mcp",
                ok: registered,
                detail: ".codex/config.toml registers `memo serve`".to_string(),
            },
        ]
    }
}
