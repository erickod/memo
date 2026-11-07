//! Fonte dos arquivos a indexar (spec §6.2).

use crate::error::CoreError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub mtime_ns: i64,
    pub size: i64,
}

pub trait SourceTree {
    fn list(&self) -> Result<Vec<SourceFile>, CoreError>;
    fn read(&self, path: &str) -> Result<String, CoreError>;
}

/// Árvore de trabalho: `git ls-files -co --exclude-standard`, só `*.py`.
pub struct WorkTree {
    root: PathBuf,
}

impl WorkTree {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl SourceTree for WorkTree {
    fn list(&self) -> Result<Vec<SourceFile>, CoreError> {
        let output = Command::new("git")
            .args(["ls-files", "-co", "--exclude-standard", "-z"])
            .current_dir(&self.root)
            .output()?;
        if !output.status.success() {
            return Err(CoreError::Source(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        let mut paths: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|p| p.ends_with(".py"))
            .map(str::to_string)
            .collect();
        paths.sort();
        paths.dedup();
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            // Arquivo listado mas removido do disco não entra no índice.
            let Ok(meta) = std::fs::metadata(self.root.join(&path)) else {
                continue;
            };
            let mtime_ns = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
            files.push(SourceFile {
                path,
                mtime_ns,
                size: i64::try_from(meta.len()).unwrap_or(i64::MAX),
            });
        }
        Ok(files)
    }

    fn read(&self, path: &str) -> Result<String, CoreError> {
        let bytes = std::fs::read(self.root.join(path))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Uma ref git lida sem checkout: `git ls-tree` e `git cat-file --batch` (spec §6.2).
pub struct GitRef {
    root: PathBuf,
    git_ref: String,
}

impl GitRef {
    pub fn new(root: &Path, git_ref: &str) -> Result<Self, CoreError> {
        let _ = (root, git_ref);
        todo!("spec §6.2 ref git")
    }

    /// Hash do commit a que a ref aponta.
    pub fn commit(&self) -> Result<String, CoreError> {
        let _ = (&self.root, &self.git_ref);
        todo!("spec §6.2 ref git")
    }
}

impl SourceTree for GitRef {
    fn list(&self) -> Result<Vec<SourceFile>, CoreError> {
        todo!("spec §6.2 ref git")
    }

    fn read(&self, _path: &str) -> Result<String, CoreError> {
        todo!("spec §6.2 ref git")
    }
}
