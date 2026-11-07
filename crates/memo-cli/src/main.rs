mod render;

use clap::{Parser, Subcommand};
use memo_core::error::CoreError;
use memo_core::index::index;
use memo_core::query::{symbol_info, word_info};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use render::Coverage;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EXIT_USAGE: u8 = 1;
const EXIT_NOT_FOUND: u8 = 2;
const EXIT_INDEX: u8 = 3;
const EXIT_INTERNAL: u8 = 10;

#[derive(Parser)]
#[command(
    name = "memo",
    version,
    about = "Memória estrutural de repositórios Python"
)]
struct Cli {
    /// Raiz do repositório (padrão: raiz git atual).
    #[arg(long, global = true)]
    repo: Option<PathBuf>,
    /// Id da task (grafo de feature; ainda lê a base).
    #[arg(long, global = true, env = "MEMO_TASK")]
    task: Option<String>,
    #[arg(long, global = true)]
    json: bool,
    /// Não reindexa antes de responder.
    #[arg(long, global = true)]
    no_index: bool,
    #[arg(long, global = true)]
    limit: Option<usize>,
    #[arg(long, global = true, default_value_t = 6000)]
    max_chars: usize,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Consultas ao grafo.
    Query {
        #[command(subcommand)]
        kind: Query,
    },
}

#[derive(Subcommand)]
enum Query {
    /// Endereço, assinatura, arestas e notas de um símbolo.
    Symbol { target: String },
    /// Verbos, papéis, camadas, símbolos e vizinhas de uma palavra.
    Word { word: String },
}

enum Failure {
    NotFound(String),
    Index(String),
    Internal(String),
}

impl From<CoreError> for Failure {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NotFound(_) | CoreError::Ambiguous { .. } => {
                Failure::NotFound(e.to_string())
            }
            CoreError::Store(_) | CoreError::Source(_) => Failure::Index(e.to_string()),
            CoreError::Io(_) => Failure::Internal(e.to_string()),
        }
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String, Failure> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| Failure::Internal(e.to_string()))?;
    if !out.status.success() {
        return Err(Failure::Index(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Abre `$COMMON/memo/base.db` e reindexa a árvore de trabalho, salvo com `--no-index`.
fn open_indexed(cli: &Cli) -> Result<(SqliteStore, usize), Failure> {
    let start = cli.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = PathBuf::from(git(&start, &["rev-parse", "--show-toplevel"])?);
    let common = PathBuf::from(git(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let dir = common.join("memo");
    std::fs::create_dir_all(&dir).map_err(|e| Failure::Internal(e.to_string()))?;
    let mut store = SqliteStore::open(&dir.join("base.db"))?;
    let errors = if cli.no_index {
        store.files_with_error()?.len()
    } else {
        index(&WorkTree::new(&root), &mut store)?
            .files_with_error
            .len()
    };
    Ok((store, errors))
}

fn run(cli: &Cli) -> Result<String, Failure> {
    let (store, errors) = open_indexed(cli)?;
    let cov = Coverage {
        files_with_error: errors,
    };
    let cobertura = serde_json::json!({
        "resolvidas": 0, "aproximadas": 0, "nao_resolvidas": 0, "arquivos_com_erro": errors
    });
    let index_meta = serde_json::json!({ "status": "ok" });
    let (cmd, data, text) = match &cli.command {
        Cmd::Query {
            kind: Query::Symbol { target },
        } => match symbol_info(&store, target) {
            Ok(info) => (
                "query symbol",
                serde_json::to_value(&info).map_err(|e| Failure::Internal(e.to_string()))?,
                render::symbol_text(&info, "ok", &cov, cli.max_chars),
            ),
            Err(CoreError::Ambiguous { target, candidates }) => {
                return Err(Failure::NotFound(render::ambiguous_text(
                    &target,
                    &candidates,
                )));
            }
            Err(e) => return Err(e.into()),
        },
        Cmd::Query {
            kind: Query::Word { word },
        } => {
            let info = word_info(&store, word)?;
            (
                "query word",
                serde_json::to_value(&info).map_err(|e| Failure::Internal(e.to_string()))?,
                render::word_text(&info, "ok", &cov, cli.max_chars, cli.limit),
            )
        }
    };
    if cli.json {
        let envelope = serde_json::json!({
            "schema": 1, "cmd": cmd, "task": cli.task, "index": index_meta,
            "data": data, "cobertura": cobertura
        });
        return Ok(format!("{envelope}\n"));
    }
    Ok(text)
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { EXIT_USAGE } else { 0 };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };
    match run(&cli) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(Failure::NotFound(text)) => {
            print!("{text}");
            ExitCode::from(EXIT_NOT_FOUND)
        }
        Err(Failure::Index(msg)) => {
            eprintln!("índice indisponível: {msg}");
            ExitCode::from(EXIT_INDEX)
        }
        Err(Failure::Internal(msg)) => {
            eprintln!("erro interno: {msg}");
            ExitCode::from(EXIT_INTERNAL)
        }
    }
}
