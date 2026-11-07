//! Comandos do ciclo de vida da task: derive, promote, drop e status (spec §6.4).

use crate::workspace::{self, Env};
use crate::{Cli, Failure, render};
use memo_core::index::index_with_notes_scoped;
use memo_core::notes::NoteDir;
use memo_core::query::coverage;
use memo_core::source::{GitRef, WorkTree};
use memo_core::status::status;
use memo_core::store::SqliteStore;

/// Todos os arquivos da base, mais o que a árvore de trabalho mudou: cópia pela API de backup
/// do SQLite, depois reindexação só do que mudou.
pub fn derive(cli: &Cli, task: &str, from: Option<&str>) -> Result<String, Failure> {
    let env = workspace::locate(cli)?;
    let target = workspace::feature_path(&env, task);
    if target.exists() {
        return Err(Failure::Usage(format!(
            "feature '{task}' already exists; drop it first"
        )));
    }
    let base_db = workspace::base_path(&env);
    let mut warning = String::new();
    {
        let _held = workspace::lock(&base_db)?;
        let mut base = SqliteStore::open(&base_db)?;
        let notes = NoteDir::new(&env.memo_dir.join("notes"));
        if let Some(name) = from {
            let tree = GitRef::new(&env.root, name)?;
            index_with_notes_scoped(&tree, &mut base, &env.config, &notes, "repo")?;
            base.set_meta("indexed_ref", name)?;
            base.set_meta("indexed_commit", &tree.commit()?)?;
        }
        warning = behind_main(&env, &base)?.unwrap_or(warning);
        std::fs::create_dir_all(target.parent().unwrap_or(&env.memo_dir))
            .map_err(|e| Failure::Internal(e.to_string()))?;
        base.backup_to(&target)?;
    }
    let _held = workspace::lock(&target)?;
    let mut feature = SqliteStore::open(&target)?;
    let notes = NoteDir::new(&env.memo_dir.join("notes"));
    let scope = format!("task:{task}");
    let report = index_with_notes_scoped(
        &WorkTree::new(&env.root),
        &mut feature,
        &env.config,
        &notes,
        &scope,
    )?;
    let mut out = String::new();
    if !warning.is_empty() {
        out.push_str(&warning);
        out.push('\n');
    }
    out.push_str(&format!(
        "derive {task}  arquivos={}\n{}\n",
        report.files_total,
        report.events.line()
    ));
    Ok(out)
}

/// Uma linha de aviso quando a base indexada está atrás da branch principal.
fn behind_main(env: &Env, base: &SqliteStore) -> Result<Option<String>, Failure> {
    let (Some(indexed), Some(main)) = (
        base.get_meta("indexed_commit")?,
        workspace::resolve_main(env),
    ) else {
        return Ok(None);
    };
    let tip = workspace::git(&env.root, &["rev-parse", &format!("{main}^{{commit}}")])?;
    Ok((indexed != tip).then(|| {
        format!(
            "aviso: base atrás de {main} ({} < {})",
            &indexed[..7],
            &tip[..7]
        )
    }))
}

/// Depois do merge: reindexa a base pela ref principal e promove as notas da task.
pub fn promote(cli: &Cli, task: &str) -> Result<String, Failure> {
    let env = workspace::locate(cli)?;
    let main = workspace::resolve_main(&env).ok_or_else(|| {
        Failure::Usage("main branch not found; set main_branch in memo.toml".to_string())
    })?;
    let base_db = workspace::base_path(&env);
    let _held = workspace::lock(&base_db)?;
    let mut base = SqliteStore::open(&base_db)?;
    let notes = NoteDir::new(&env.memo_dir.join("notes"));
    let tree = GitRef::new(&env.root, &main)?;
    index_with_notes_scoped(&tree, &mut base, &env.config, &notes, "repo")?;
    base.set_meta("indexed_ref", &main)?;
    base.set_meta("indexed_commit", &tree.commit()?)?;
    let report = notes.promote(&base, task)?;
    Ok(render::promote_text(task, &main, &report))
}

/// Apaga o grafo da feature e as notas da task; só por comando explícito.
pub fn drop(cli: &Cli, task: &str) -> Result<String, Failure> {
    let env = workspace::locate(cli)?;
    let target = workspace::feature_path(&env, task);
    let had_feature = target.exists();
    if had_feature {
        let _held = workspace::lock(&target)?;
        std::fs::remove_file(&target).map_err(|e| Failure::Internal(e.to_string()))?;
    }
    let _ = std::fs::remove_file(workspace::lock_path(&target));
    let removed = NoteDir::new(&env.memo_dir.join("notes")).drop_task(task)?;
    if !had_feature && removed == 0 {
        return Err(Failure::NotFound(format!("task '{task}' not found\n")));
    }
    Ok(format!(
        "drop {task}: feature {}, {removed} notas\n",
        if had_feature { "apagada" } else { "ausente" }
    ))
}

/// Base e features, notas por estado, tasks promovíveis e cobertura da base.
pub fn status_text(cli: &Cli) -> Result<(String, serde_json::Value), Failure> {
    let env = workspace::locate(cli)?;
    let merged: Vec<String> = match workspace::resolve_main(&env) {
        Some(main) => workspace::git(
            &env.root,
            &["branch", "--merged", &main, "--format=%(refname:short)"],
        )
        .map(|out| out.lines().map(|l| l.trim().to_string()).collect())
        .unwrap_or_default(),
        None => Vec::new(),
    };
    let summary = status(&env.memo_dir, &merged)?;
    let cov = match summary.base {
        Some(_) => Some(coverage(&SqliteStore::open(&workspace::base_path(&env))?)?),
        None => None,
    };
    let data = serde_json::to_value(&summary).map_err(|e| Failure::Internal(e.to_string()))?;
    Ok((render::status_text(&summary, cov.as_ref()), data))
}
