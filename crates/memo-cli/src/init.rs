//! `memo init <harness>`: `$COMMON/memo`, `memo.toml`, base indexada e integração (spec §9.1).

use crate::workspace::{self, Env};
use crate::{Cli, Failure};
use memo_core::index::index_with_notes_scoped;
use memo_core::notes::NoteDir;
use memo_core::source::GitRef;
use memo_core::store::SqliteStore;
use memo_harness::{Change, InstallOpts, adapter};
use std::path::{Path, PathBuf};

pub struct InitArgs<'a> {
    pub args: &'a [String],
    pub git_ref: Option<&'a str>,
    pub mcp: bool,
    pub print: bool,
}

fn show(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn describe(root: &Path, changes: &[Change], print: bool) -> String {
    let mut out = String::new();
    for c in changes {
        let verdict = match (c.changed, print) {
            (false, _) => "sem mudança",
            (true, true) => "seria gravado",
            (true, false) => "gravado",
        };
        out.push_str(&format!("{}: {verdict}\n", show(root, &c.path)));
    }
    out
}

/// Ordem idempotente da §9.1; com `--print` não escreve nada, nem `$COMMON/memo`.
pub fn init(cli: &Cli, init: &InitArgs) -> Result<String, Failure> {
    let (repo, harness) = match init.args {
        [harness] => (None, harness),
        [repo, harness] => (Some(PathBuf::from(repo)), harness),
        _ => {
            return Err(Failure::Usage(
                "usage: memo init [<repo>] <harness>".to_string(),
            ));
        }
    };
    // Valida o harness antes de qualquer escrita: formato desconhecido é erro, nunca palpite.
    let adapter = adapter(harness).map_err(|e| Failure::Usage(e.to_string()))?;
    let start = repo
        .or_else(|| cli.repo.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let env = workspace::locate_from(&start, !init.print)?;
    let opts = InstallOpts {
        mcp: init.mcp,
        print: init.print,
    };
    let main = init
        .git_ref
        .map(str::to_string)
        .or_else(|| workspace::resolve_main(&env));
    let toml_path = env.memo_dir.join("memo.toml");
    let mut out = if init.print {
        format!("init {harness} --print: nada foi gravado\n")
    } else {
        format!("init {harness}\n")
    };

    let toml_state = match (toml_path.exists(), init.print) {
        (true, _) => "já existe",
        (false, true) => "seria gravado",
        (false, false) => {
            std::fs::write(&toml_path, env.config.to_toml())
                .map_err(|e| Failure::Internal(e.to_string()))?;
            "gravado"
        }
    };
    out.push_str(&format!("{}: {toml_state}\n", show(&env.root, &toml_path)));
    out.push_str(&index_base(&env, main.as_deref(), init.print)?);
    let changes = adapter
        .install(&env.root, &opts)
        .map_err(|e| Failure::Usage(e.to_string()))?;
    out.push_str(&describe(&env.root, &changes, init.print));
    Ok(out)
}

/// Passo 3: a base vem da branch principal por ref, mesmo com outra branch no checkout.
fn index_base(env: &Env, main: Option<&str>, print: bool) -> Result<String, Failure> {
    let Some(main) = main else {
        return Ok("base: adiada (sem branch principal)\n".to_string());
    };
    if print {
        return Ok(format!("base: seria indexada de {main}\n"));
    }
    let db = workspace::base_path(env);
    let _held = workspace::lock(&db)?;
    let mut store = SqliteStore::open(&db)?;
    let notes = NoteDir::new(&env.memo_dir.join("notes"));
    let tree = GitRef::new(&env.root, main)?;
    let report = index_with_notes_scoped(&tree, &mut store, &env.config, &notes, "repo")?;
    store.set_meta("indexed_ref", main)?;
    store.set_meta("indexed_commit", &tree.commit()?)?;
    Ok(format!(
        "base: indexada de {main} (arquivos={})\n",
        report.files_total
    ))
}
