//! Descoberta do repositório, do grafo a usar (base ou feature) e do lock (spec §6).

use crate::{Cli, Failure};
use memo_core::config::Config;
use memo_core::index::{IndexReport, index_with_notes_scoped};
use memo_core::lock::{DbLock, default_timeout};
use memo_core::notes::NoteDir;
use memo_core::source::{GitRef, WorkTree};
use memo_core::store::SqliteStore;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Env {
    pub root: PathBuf,
    pub memo_dir: PathBuf,
    pub config: Config,
}

pub fn git(repo: &Path, args: &[&str]) -> Result<String, Failure> {
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

/// Raiz git, `$COMMON/memo` (criado) e a config do repositório.
pub fn locate(cli: &Cli) -> Result<Env, Failure> {
    let start = cli.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    locate_from(&start, true)
}

/// Como `locate`, a partir de `start`; com `create = false` não cria `$COMMON/memo`
/// (dry-run do `init`) e usa as sementes se ainda não há `memo.toml`.
pub fn locate_from(start: &Path, create: bool) -> Result<Env, Failure> {
    let start = start.to_path_buf();
    let root = PathBuf::from(git(&start, &["rev-parse", "--show-toplevel"])?);
    let common = PathBuf::from(git(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let memo_dir = common.join("memo");
    if create {
        std::fs::create_dir_all(&memo_dir).map_err(|e| Failure::Internal(e.to_string()))?;
    }
    let config = Config::load(&memo_dir)?;
    Ok(Env {
        root,
        memo_dir,
        config,
    })
}

/// Chave da feature: `--task`, ou o nome do branch atual com `/` trocado por `-` (spec §6.1).
fn feature_key(cli: &Cli, env: &Env) -> Option<String> {
    let key = cli.task.clone().or_else(|| {
        let branch = git(&env.root, &["branch", "--show-current"]).ok()?;
        (!branch.is_empty()).then(|| branch.replace('/', "-"))
    });
    // Um branch com caracteres fora do id válido simplesmente não tem feature.
    key.filter(|k| memo_core::task::is_valid_id(k))
}

/// A branch principal: `main_branch` da config, `origin/HEAD`, `main`, `master` (spec §6.1).
pub fn resolve_main(env: &Env) -> Option<String> {
    let verifies = |name: &str| {
        git(
            &env.root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{name}^{{commit}}"),
            ],
        )
        .is_ok()
    };
    let origin_head = git(
        &env.root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok();
    env.config
        .main_branch
        .iter()
        .cloned()
        .chain(origin_head)
        .chain(["main".to_string(), "master".to_string()])
        .find(|name| verifies(name))
}

pub fn lock_path(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

pub fn lock(db: &Path) -> Result<DbLock, Failure> {
    Ok(DbLock::acquire(&lock_path(db), default_timeout())?)
}

pub fn base_path(env: &Env) -> PathBuf {
    env.memo_dir.join("base.db")
}

pub fn feature_path(env: &Env, task: &str) -> PathBuf {
    env.memo_dir.join("features").join(format!("{task}.db"))
}

#[derive(Default)]
pub struct IndexOpts {
    pub git_ref: Option<String>,
    pub full: bool,
}

pub struct Opened {
    pub config: Config,
    pub store: SqliteStore,
    pub memo_dir: PathBuf,
    pub report: Option<IndexReport>,
    pub changed: Vec<String>,
    pub _lock: DbLock,
}

/// Abre o grafo da feature (se a task tem uma derivada) ou a base, com lock, e reindexa
/// antes de responder, salvo com `--no-index`.
pub fn open_indexed(cli: &Cli, opts: &IndexOpts) -> Result<Opened, Failure> {
    let env = locate(cli)?;
    let feature = feature_key(cli, &env)
        .map(|task| (feature_path(&env, &task), task))
        .filter(|(path, _)| path.exists());
    let (db, scope) = match feature {
        Some((path, task)) => (path, format!("task:{task}")),
        None => (base_path(&env), "repo".to_string()),
    };
    let held = lock(&db)?;
    let mut store = SqliteStore::open(&db)?;
    let notes = NoteDir::new(&env.memo_dir.join("notes"));
    let report = if cli.no_index {
        None
    } else {
        if opts.full {
            store.invalidate_files()?;
        }
        // D12: a base vem da ref principal; a árvore de trabalho só alimenta a feature.
        let on_feature = scope != "repo";
        let ref_name = opts
            .git_ref
            .clone()
            .or_else(|| (!on_feature).then(|| resolve_main(&env)).flatten());
        Some(match ref_name {
            Some(name) => {
                let tree = GitRef::new(&env.root, &name)?;
                let report =
                    index_with_notes_scoped(&tree, &mut store, &env.config, &notes, &scope)?;
                store.set_meta("indexed_ref", &name)?;
                store.set_meta("indexed_commit", &tree.commit()?)?;
                report
            }
            None => {
                if !on_feature {
                    eprintln!("aviso: sem branch principal; a base indexa a árvore de trabalho");
                }
                index_with_notes_scoped(
                    &WorkTree::new(&env.root),
                    &mut store,
                    &env.config,
                    &notes,
                    &scope,
                )?
            }
        })
    };
    let changed = if scope != "repo" && matches!(cli.command, crate::Cmd::Brief { .. }) {
        let base = base_path(&env);
        if base.exists() {
            match SqliteStore::open_read_only(&base) {
                Ok(base) => memo_core::structural::changed_symbols(&base, &store)?,
                Err(error) => {
                    eprintln!("aviso: contexto de mudanças indisponível: {error}");
                    Vec::new()
                }
            }
        } else {
            eprintln!("aviso: contexto de mudanças indisponível: índice base ausente");
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Ok(Opened {
        config: env.config,
        store,
        memo_dir: env.memo_dir,
        report,
        changed,
        _lock: held,
    })
}
