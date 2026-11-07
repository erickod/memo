mod render;

use clap::{Parser, Subcommand};
use memo_core::brief::{apply_delta, brief};
use memo_core::config::Config;
use memo_core::delta::apply_impact_delta;
use memo_core::error::CoreError;
use memo_core::index::index_with;
use memo_core::query::{
    callees, callers, coverage, impact, readers, symbol_info, tests_for, type_info, word_info,
    writers,
};
use memo_core::source::WorkTree;
use memo_core::store::SqliteStore;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EXIT_USAGE: u8 = 1;
const EXIT_NOT_FOUND: u8 = 2;
const EXIT_INDEX: u8 = 3;
const EXIT_VALIDATION: u8 = 4;
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
    /// Só o que é novo desde a última chamada, mesmo sem `--task`.
    #[arg(long, global = true)]
    since_last: bool,
    /// Tudo, ignorando o delta por task.
    #[arg(long, global = true)]
    full: bool,
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
    /// Âncoras, símbolos, contorno, padrão, escritores e notas para um pedido.
    Brief { request: String },
}

#[derive(Subcommand)]
enum Query {
    /// Endereço, assinatura, arestas e notas de um símbolo.
    Symbol { target: String },
    /// Verbos, papéis, camadas, símbolos e vizinhas de uma palavra.
    Word { word: String },
    /// Símbolos que escrevem o atributo.
    Writers { attr: String },
    /// Símbolos que leem o atributo.
    Readers { attr: String },
    /// Quem produz e quem consome o tipo.
    Type { target: String },
    /// Quem chama o símbolo.
    Callers {
        target: String,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(i64).range(1..=3))]
        depth: i64,
    },
    /// O que o símbolo chama.
    Callees {
        target: String,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(i64).range(1..=3))]
        depth: i64,
    },
    /// Chamadores, consumidores do tipo, Protocol, testes e notas.
    Impact { target: String },
    /// Testes que chamam o símbolo (exato) ou se sobrepõem por palavras (~).
    Tests { target: String },
}

enum Failure {
    Usage(String),
    Validation(String),
    NotFound(String),
    Index(String),
    Internal(String),
}

impl From<CoreError> for Failure {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Config(_) => Failure::Usage(e.to_string()),
            CoreError::Validation { .. } => Failure::Validation(e.to_string()),
            CoreError::Note(_) => Failure::Index(e.to_string()),
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
fn open_indexed(cli: &Cli) -> Result<SqliteStore, Failure> {
    let start = cli.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    let root = PathBuf::from(git(&start, &["rev-parse", "--show-toplevel"])?);
    let common = PathBuf::from(git(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let dir = common.join("memo");
    std::fs::create_dir_all(&dir).map_err(|e| Failure::Internal(e.to_string()))?;
    let config = Config::load(&dir)?;
    let mut store = SqliteStore::open(&dir.join("base.db"))?;
    if !cli.no_index {
        index_with(&WorkTree::new(&root), &mut store, &config)?;
    }
    Ok(store)
}

fn json_value<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, Failure> {
    serde_json::to_value(value).map_err(|e| Failure::Internal(e.to_string()))
}

fn run(cli: &Cli) -> Result<String, Failure> {
    let store = open_indexed(cli)?;
    let cov = coverage(&store)?;
    let (max, limit) = (cli.max_chars, cli.limit);
    let delta_task = cli
        .task
        .clone()
        .or_else(|| cli.since_last.then(|| "-".to_string()));
    let kind = match &cli.command {
        Cmd::Query { kind } => kind,
        Cmd::Brief { request } => {
            let mut b = brief(&store, request)?;
            if let Some(task) = &delta_task {
                apply_delta(&store, task, &mut b, cli.full)?;
            }
            let text = render::brief_text(&b, "ok", cli.task.as_deref(), &cov, max, limit);
            return finish(cli, "brief", json_value(&b)?, &cov, text);
        }
    };
    let (cmd, data, text) = match kind {
        Query::Symbol { target } => match symbol_info(&store, target) {
            Ok(info) => (
                "query symbol",
                json_value(&info)?,
                render::symbol_text(&info, "ok", &cov, max, limit),
            ),
            Err(CoreError::Ambiguous { target, candidates }) => {
                return Err(Failure::NotFound(render::ambiguous_text(
                    &target,
                    &candidates,
                )));
            }
            Err(e) => return Err(e.into()),
        },
        Query::Word { word } => {
            let info = word_info(&store, word)?;
            (
                "query word",
                json_value(&info)?,
                render::word_text(&info, "ok", &cov, max, limit),
            )
        }
        Query::Writers { attr } => {
            let rel = writers(&store, attr)?;
            (
                "query writers",
                json_value(&rel)?,
                render::relation_text("writers", &rel, "ok", &cov, max, limit),
            )
        }
        Query::Readers { attr } => {
            let rel = readers(&store, attr)?;
            (
                "query readers",
                json_value(&rel)?,
                render::relation_text("readers", &rel, "ok", &cov, max, limit),
            )
        }
        Query::Callers { target, depth } => {
            let chain = callers(&store, target, *depth)?;
            (
                "query callers",
                json_value(&chain)?,
                render::chain_text("callers", &chain, "ok", &cov, max, limit),
            )
        }
        Query::Callees { target, depth } => {
            let chain = callees(&store, target, *depth)?;
            (
                "query callees",
                json_value(&chain)?,
                render::chain_text("callees", &chain, "ok", &cov, max, limit),
            )
        }
        Query::Impact { target } => {
            let mut info = impact(&store, target)?;
            if let Some(task) = &delta_task {
                apply_impact_delta(&store, task, &mut info, cli.full)?;
            }
            (
                "query impact",
                json_value(&info)?,
                render::impact_text(&info, "ok", &cov, max, limit),
            )
        }
        Query::Tests { target } => {
            let tests = tests_for(&store, target)?;
            let addr = symbol_info(&store, target)?.addr;
            (
                "query tests",
                json_value(&tests)?,
                render::tests_text(&addr, &tests, "ok", &cov, max, limit),
            )
        }
        Query::Type { target } => {
            let info = type_info(&store, target)?;
            (
                "query type",
                json_value(&info)?,
                render::type_text(&info, "ok", &cov, max, limit),
            )
        }
    };
    finish(cli, cmd, data, &cov, text)
}

fn finish(
    cli: &Cli,
    cmd: &str,
    data: serde_json::Value,
    cov: &memo_core::query::Coverage,
    text: String,
) -> Result<String, Failure> {
    if cli.json {
        let envelope = serde_json::json!({
            "schema": 1, "cmd": cmd, "task": cli.task, "index": { "status": "ok" },
            "data": data, "cobertura": json_value(cov)?
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
        Err(Failure::Usage(msg)) => {
            eprintln!("uso inválido: {msg}");
            ExitCode::from(EXIT_USAGE)
        }
        Err(Failure::Validation(msg)) => {
            println!("erro: {msg}");
            ExitCode::from(EXIT_VALIDATION)
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
