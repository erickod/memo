//! Fonte dos arquivos a indexar (spec §6.2).

use crate::error::CoreError;
use std::path::{Path, PathBuf};

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
        let _ = &self.root;
        todo!("spec §6.2")
    }

    fn read(&self, _path: &str) -> Result<String, CoreError> {
        todo!("spec §6.2")
    }
}
