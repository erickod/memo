mod init;
mod lifecycle;
mod render;
mod serve;
mod workspace;

use clap::{Parser, Subcommand};
use memo_core::aliases::suggest as suggest_aliases;
use memo_core::brief::{apply_delta, brief_with_changes};
use memo_core::delta::apply_impact_delta;
use memo_core::error::CoreError;
use memo_core::hide;
use memo_core::notes::{NewNote, NoteDir, format_utc, is_generic_anchor, notes_for};
use memo_core::query::{
    callees, callers, composite_word, coverage, impact, readers, symbol_info, tests_for, type_info,
    word_info, word_parts, writers,
};
use memo_core::store::SqliteStore;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// Esconde os testes da saída; é o padrão (TASK-7, D2), mantido por compatibilidade.
    #[arg(long, global = true, conflicts_with = "tests")]
    no_tests: bool,
    /// Mostra os testes na saída (por padrão ficam ocultos; as estatísticas contam tudo).
    #[arg(long, global = true)]
    tests: bool,
    /// Lista todos os itens, sem agrupar por módulo as seções com mais de 10 itens.
    #[arg(long, global = true)]
    all: bool,
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
    /// Instala a integração com o harness (`claude` ou `codex`) e indexa a base.
    Init {
        /// `[<repo>] <harness>`
        #[arg(num_args = 1..=2, required = true)]
        args: Vec<String>,
        /// Ref a indexar como base (padrão: a branch principal).
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Registra também o servidor `memo serve` na config MCP.
        #[arg(long)]
        mcp: bool,
        /// Mostra o que seria escrito e não escreve nada.
        #[arg(long)]
        print: bool,
    },
    /// Servidor MCP por stdio, sobre os mesmos comandos do CLI.
    Serve,
    /// Reindexa e mostra o resumo dos eventos do lote.
    Index {
        /// Indexa uma ref git sem checkout.
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Reextrai todos os arquivos, mesmo os inalterados.
        #[arg(long)]
        full: bool,
    },
    /// Cria o grafo da feature, copiado da base e reindexado na árvore atual.
    Derive {
        task: String,
        /// Indexa esta ref como base antes de copiar.
        #[arg(long)]
        from: Option<String>,
    },
    /// Depois do merge: reindexa a base pela principal e promove as notas da task.
    Promote { task: String },
    /// Apaga o grafo da feature e as notas da task.
    Drop { task: String },
    /// Base, features, notas por estado e tasks promovíveis.
    Status,
    /// Âncoras, símbolos, contorno, padrão, escritores e notas para um pedido.
    Brief { request: String },
    /// Notas ancoradas nos símbolos.
    Note {
        #[command(subcommand)]
        cmd: NoteCmd,
    },
}

#[derive(Subcommand)]
enum NoteCmd {
    /// Registra uma nota (estado inicial `proposta`).
    Add {
        #[arg(long, value_parser = ["decisao", "regra", "restricao", "descartado", "glossario"])]
        tipo: String,
        /// `sym:<qname>`, `word:<texto>`, `edge:<qname>-><qname>` ou `file:<caminho>`.
        #[arg(long)]
        ancora: String,
        #[arg(long = "secundaria")]
        secundarias: Vec<String>,
        #[arg(long, value_parser = ["humano", "recomendacao_aceita", "agente"])]
        origem: String,
        /// Confirma já na criação (só para decisão vinda de rodada humana).
        #[arg(long)]
        confirm: bool,
        /// Id da nota que esta substitui.
        #[arg(long)]
        substitui: Option<String>,
        /// Sinônimo (nota `glossario`).
        #[arg(long = "alias")]
        aliases: Vec<String>,
        texto: String,
    },
    /// Lista as notas, em qualquer estado.
    List,
    /// Mostra uma nota por inteiro.
    Show { id: String },
    /// Confirma uma nota `proposta`.
    Confirm { id: String },
    /// Atualiza os hashes de uma nota `stale` e volta ao estado anterior.
    Recheck { id: String },
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
    /// Pares de palavras que provavelmente são o mesmo conceito (somente leitura).
    Aliases,
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
            CoreError::Note(_) | CoreError::Locked(_) => Failure::Index(e.to_string()),
            CoreError::Ambiguous { target, candidates } => {
                Failure::NotFound(render::ambiguous_text(&target, &candidates))
            }
            CoreError::NotFound(_) => Failure::NotFound(e.to_string()),
            CoreError::Store(_) | CoreError::Source(_) => Failure::Index(e.to_string()),
            CoreError::Io(_) => Failure::Internal(e.to_string()),
        }
    }
}

/// Carimbo da nota: `MEMO_NOW` (testes) ou o relógio, em UTC.
fn now_stamp() -> String {
    if let Ok(fixed) = std::env::var("MEMO_NOW") {
        return fixed;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    format_utc(secs)
}

/// A nota em JSON, com o campo calculado `ancora_generica`.
fn with_flag(note: &memo_core::notes::Note, generic: bool) -> Result<serde_json::Value, Failure> {
    let mut value = json_value(note)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "ancora_generica".to_string(),
            serde_json::Value::Bool(generic),
        );
    }
    Ok(value)
}

fn count(hidden: usize) -> i64 {
    i64::try_from(hidden).unwrap_or(i64::MAX)
}

fn json_value<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, Failure> {
    serde_json::to_value(value).map_err(|e| Failure::Internal(e.to_string()))
}

/// Ids de task viram nomes de arquivo: recusar na borda, antes de qualquer E/S.
fn check_task_ids(cli: &Cli) -> Result<(), Failure> {
    let from_command = match &cli.command {
        Cmd::Derive { task, .. } | Cmd::Promote { task } | Cmd::Drop { task } => Some(task),
        _ => None,
    };
    for task in cli.task.iter().chain(from_command) {
        if !memo_core::task::is_valid_id(task) {
            return Err(Failure::Usage(format!(
                "invalid task id '{}' (use letters, digits, '.', '_' or '-')",
                task.escape_debug()
            )));
        }
    }
    Ok(())
}

fn run(cli: &Cli) -> Result<String, Failure> {
    check_task_ids(cli)?;
    let opts = match &cli.command {
        Cmd::Index { git_ref, full } => workspace::IndexOpts {
            git_ref: git_ref.clone(),
            full: *full,
        },
        _ => workspace::IndexOpts::default(),
    };
    match &cli.command {
        Cmd::Serve => {
            serve::serve(cli)?;
            return Ok(String::new());
        }
        Cmd::Init {
            args,
            git_ref,
            mcp,
            print,
        } => {
            let text = init::init(
                cli,
                &init::InitArgs {
                    args,
                    git_ref: git_ref.as_deref(),
                    mcp: *mcp,
                    print: *print,
                },
            )?;
            return finish_plain(cli, "init", &text);
        }
        Cmd::Derive { task, from } => {
            let text = lifecycle::derive(cli, task, from.as_deref())?;
            return finish_plain(cli, "derive", &text);
        }
        Cmd::Promote { task } => {
            let text = lifecycle::promote(cli, task)?;
            return finish_plain(cli, "promote", &text);
        }
        Cmd::Drop { task } => {
            let text = lifecycle::drop(cli, task)?;
            return finish_plain(cli, "drop", &text);
        }
        Cmd::Status => {
            let (text, data) = lifecycle::status_text(cli)?;
            return Ok(if cli.json {
                format!(
                    "{}\n",
                    serde_json::json!({ "schema": 1, "cmd": "status", "task": cli.task, "data": data })
                )
            } else {
                text
            });
        }
        _ => {}
    }
    let workspace::Opened {
        config,
        store,
        memo_dir,
        report,
        changed,
        _lock,
    } = workspace::open_indexed(cli, &opts)?;
    let notes_dir = NoteDir::new(&memo_dir.join("notes"));
    let mut cov = coverage(&store)?;
    cov.notas_com_erro = i64::try_from(notes_dir.scan()?.1.len()).unwrap_or(i64::MAX);
    let (max, limit) = (cli.max_chars, cli.limit);
    let delta_task = cli
        .task
        .clone()
        .or_else(|| cli.since_last.then(|| "-".to_string()));
    let kind = match &cli.command {
        Cmd::Query { kind } => kind,
        Cmd::Serve
        | Cmd::Init { .. }
        | Cmd::Derive { .. }
        | Cmd::Promote { .. }
        | Cmd::Drop { .. }
        | Cmd::Status => {
            unreachable!()
        }
        Cmd::Index { .. } => {
            let text = render::index_text(report.as_ref(), &cov);
            return finish(cli, "index", json_value(&report.is_some())?, &cov, text);
        }
        Cmd::Note { cmd } => return run_note(cli, cmd, &store, &notes_dir, &cov),
        Cmd::Brief { request } => {
            let visible = notes_dir.visible(cli.task.as_deref())?;
            let mut b = brief_with_changes(&store, request, &visible, &config.bridges, &changed)?;
            if let Some(task) = &delta_task {
                apply_delta(&store, task, &mut b, cli.full)?;
            }
            if !cli.tests {
                cov.testes_ocultos = count(hide::brief(&mut b));
            }
            let text = render::brief_text(&b, "ok", cli.task.as_deref(), &cov, max, limit);
            return finish(cli, "brief", json_value(&b)?, &cov, text);
        }
    };
    let (cmd, data, text) = match kind {
        Query::Symbol { target } => match symbol_info(&store, target) {
            Ok(mut info) => {
                let visible = notes_dir.visible(cli.task.as_deref())?;
                info.notes = notes_for(&visible, &[info.addr.qname.clone()], &[], &[]);
                if !cli.tests {
                    cov.testes_ocultos = count(hide::symbol(&mut info));
                }
                (
                    "query symbol",
                    json_value(&info)?,
                    render::symbol_text(&info, "ok", &cov, max, limit, cli.all),
                )
            }
            Err(CoreError::Ambiguous { target, candidates }) => {
                return Err(Failure::NotFound(render::ambiguous_text(
                    &target,
                    &candidates,
                )));
            }
            Err(e) => return Err(e.into()),
        },
        Query::Word { word } if word_parts(word, &config).len() > 1 => {
            let parts = word_parts(word, &config);
            let mut info = composite_word(&store, word, &parts)?;
            if !cli.tests {
                let before = info.symbols.len();
                info.symbols.retain(|s| !s.in_tests());
                cov.testes_ocultos = count(before - info.symbols.len());
            }
            (
                "query word",
                json_value(&info)?,
                render::composite_word_text(&info, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Word { word } => {
            let parts = word_parts(word, &config);
            let word = parts.first().unwrap_or(word);
            let mut info = word_info(&store, word, &config.bridges)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::word(&mut info));
            }
            (
                "query word",
                json_value(&info)?,
                render::word_text(&info, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Writers { attr } => {
            let mut rel = writers(&store, attr)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::relation(&mut rel));
            }
            (
                "query writers",
                json_value(&rel)?,
                render::relation_text("writers", &rel, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Readers { attr } => {
            let mut rel = readers(&store, attr)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::relation(&mut rel));
            }
            (
                "query readers",
                json_value(&rel)?,
                render::relation_text("readers", &rel, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Callers { target, depth } => {
            let mut chain = callers(&store, target, *depth)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::chain(&mut chain));
            }
            (
                "query callers",
                json_value(&chain)?,
                render::chain_text("callers", &chain, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Callees { target, depth } => {
            let mut chain = callees(&store, target, *depth)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::chain(&mut chain));
            }
            (
                "query callees",
                json_value(&chain)?,
                render::chain_text("callees", &chain, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Impact { target } => {
            let mut info = impact(&store, target)?;
            let visible = notes_dir.visible(cli.task.as_deref())?;
            info.notes = notes_for(&visible, &[info.target.qname.clone()], &[], &[]);
            if !cli.tests {
                cov.testes_ocultos = count(hide::impact(&mut info));
            }
            if let Some(task) = &delta_task {
                apply_impact_delta(&store, task, &mut info, cli.full)?;
            }
            (
                "query impact",
                json_value(&info)?,
                render::impact_text(&info, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Tests { target } => {
            if cli.no_tests {
                return Err(Failure::Usage(
                    "--no-tests contradicts `query tests`, which lists tests".to_string(),
                ));
            }
            let tests = tests_for(&store, target)?;
            let addr = symbol_info(&store, target)?.addr;
            (
                "query tests",
                json_value(&tests)?,
                render::tests_text(&addr, &tests, "ok", &cov, max, limit, cli.all),
            )
        }
        Query::Aliases => {
            let glossary = notes_dir.visible(cli.task.as_deref())?;
            let mut pairs = suggest_aliases(&store, &config.bridges, &glossary)?;
            if let Some(n) = limit {
                pairs.truncate(n);
            }
            (
                "query aliases",
                json_value(&pairs)?,
                render::aliases_text(&pairs, "ok", &cov),
            )
        }
        Query::Type { target } => {
            let mut info = type_info(&store, target)?;
            if !cli.tests {
                cov.testes_ocultos = count(hide::type_info(&mut info));
            }
            (
                "query type",
                json_value(&info)?,
                render::type_text(&info, "ok", &cov, max, limit, cli.all),
            )
        }
    };
    finish(cli, cmd, data, &cov, text)
}

fn run_note(
    cli: &Cli,
    cmd: &NoteCmd,
    store: &SqliteStore,
    notes: &NoteDir,
    cov: &memo_core::query::Coverage,
) -> Result<String, Failure> {
    let (name, data, text) = match cmd {
        NoteCmd::Add {
            tipo,
            ancora,
            secundarias,
            origem,
            confirm,
            substitui,
            aliases,
            texto,
        } => {
            let new = NewNote {
                tipo: tipo.clone(),
                ancora: ancora.clone(),
                secundarias: secundarias.clone(),
                origem: origem.clone(),
                text: texto.clone(),
                task: cli.task.clone(),
                confirm: *confirm,
                substitui: substitui.clone(),
                aliases: aliases.clone(),
            };
            let out = notes.add(store, new, &now_stamp())?;
            (
                "note add",
                json_value(&serde_json::json!({ "note": out.note, "conflitos": out.conflicts }))?,
                render::note_added_text(&out.note, &out.conflicts),
            )
        }
        NoteCmd::List => {
            let all = notes.visible(cli.task.as_deref())?;
            let mut generic = Vec::new();
            let mut listed = Vec::new();
            for note in &all {
                let is_generic = is_generic_anchor(store, note)?;
                if is_generic {
                    generic.push(note.id.clone());
                }
                listed.push(with_flag(note, is_generic)?);
            }
            (
                "note list",
                serde_json::Value::Array(listed),
                render::note_list_text(&all, &generic),
            )
        }
        NoteCmd::Show { id } => {
            let note = notes.get(id)?;
            let is_generic = is_generic_anchor(store, &note)?;
            let mut text = note.render();
            if is_generic {
                text.push_str("âncora genérica: sim\n");
            }
            ("note show", with_flag(&note, is_generic)?, text)
        }
        NoteCmd::Recheck { id } => {
            let note = notes.recheck(store, id)?;
            (
                "note recheck",
                json_value(&note)?,
                format!("note {} {}\n", note.id, note.estado),
            )
        }
        NoteCmd::Confirm { id } => {
            let note = notes.confirm_recorded(store, id)?;
            (
                "note confirm",
                json_value(&note)?,
                format!("note {} {}\n", note.id, note.estado),
            )
        }
    };
    finish(cli, name, data, cov, text)
}

/// Saída de comandos sem índice nem cobertura: o texto, ou um envelope JSON com ele.
fn finish_plain(cli: &Cli, cmd: &str, text: &str) -> Result<String, Failure> {
    if cli.json {
        let envelope =
            serde_json::json!({ "schema": 1, "cmd": cmd, "task": cli.task, "data": text });
        return Ok(format!("{envelope}\n"));
    }
    Ok(text.to_string())
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

/// Saída de um comando: código, stdout e stderr, para rodar em processo ou pelo servidor MCP.
pub struct Outcome {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

/// Executa o CLI com `args` (o primeiro é o nome do programa).
pub fn execute(args: &[String]) -> Outcome {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let text = e.render().to_string();
            return if e.use_stderr() {
                Outcome {
                    code: EXIT_USAGE,
                    stdout: String::new(),
                    stderr: text,
                }
            } else {
                Outcome {
                    code: 0,
                    stdout: text,
                    stderr: String::new(),
                }
            };
        }
    };
    let (code, stdout, stderr) = match run(&cli) {
        Ok(text) => (0, text, String::new()),
        Err(Failure::Usage(msg)) => (EXIT_USAGE, String::new(), format!("uso inválido: {msg}\n")),
        Err(Failure::Validation(msg)) => (EXIT_VALIDATION, format!("erro: {msg}\n"), String::new()),
        Err(Failure::NotFound(text)) => (EXIT_NOT_FOUND, text, String::new()),
        Err(Failure::Index(msg)) => (
            EXIT_INDEX,
            String::new(),
            format!("índice indisponível: {msg}\n"),
        ),
        Err(Failure::Internal(msg)) => (
            EXIT_INTERNAL,
            String::new(),
            format!("erro interno: {msg}\n"),
        ),
    };
    Outcome {
        code,
        stdout,
        stderr,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let outcome = execute(&args);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    ExitCode::from(outcome.code)
}
