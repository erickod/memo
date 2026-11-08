// Helpers compartilhados: cada binário de teste usa só parte deles.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Cópia temporária e isolada da fixture, inicializada como repositório git.
pub struct Repo {
    root: PathBuf,
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

impl Repo {
    /// A fixture commitada na branch `main`: a base do `memo` vem da ref principal.
    pub fn fixture() -> Self {
        let repo = Self::fixture_uncommitted();
        repo.commit_all();
        repo
    }

    /// A fixture como repositório git sem nenhum commit.
    pub fn fixture_uncommitted() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("memo-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures"),
            &root,
        );
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        Self { root }
    }

    /// Repositório git vazio, sem a fixture e sem commit, para testes com poucos arquivos.
    pub fn empty() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("memo-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        Self { root }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Commita tudo numa branch `main`, com identidade fixa.
    pub fn commit_all(&self) {
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .env("GIT_AUTHOR_DATE", "2026-11-07T14:05:00Z")
                .env("GIT_COMMITTER_DATE", "2026-11-07T14:05:00Z")
                .current_dir(&self.root)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["add", "-A"]);
        let staged = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&self.root)
            .status()
            .unwrap();
        if !staged.success() {
            git(&["commit", "-q", "-m", "init"]);
        }
        git(&["branch", "-M", "main"]);
    }

    pub fn write(&self, rel: &str, content: &str) {
        let file = self.root.join(rel);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
