//! Fonte dos arquivos a indexar (spec §6.2).

use crate::error::CoreError;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
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
///
/// A ref é fixada no commit ao qual aponta quando o objeto é criado.
pub struct GitRef {
    root: PathBuf,
    commit: String,
    blobs: Mutex<HashMap<String, String>>,
    batch: Mutex<Option<CatFile>>,
}

struct CatFile {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>, CoreError> {
    let output = Command::new("git").args(args).current_dir(root).output()?;
    if !output.status.success() {
        return Err(CoreError::Source(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(output.stdout)
}

impl GitRef {
    pub fn new(root: &Path, git_ref: &str) -> Result<Self, CoreError> {
        let spec = format!("{git_ref}^{{commit}}");
        let out = git_output(root, &["rev-parse", "--verify", &spec])?;
        Ok(Self {
            root: root.to_path_buf(),
            commit: String::from_utf8_lossy(&out).trim().to_string(),
            blobs: Mutex::new(HashMap::new()),
            batch: Mutex::new(None),
        })
    }

    /// Hash do commit a que a ref aponta.
    pub fn commit(&self) -> Result<String, CoreError> {
        Ok(self.commit.clone())
    }
}

fn poisoned<T>(_: T) -> CoreError {
    CoreError::Source("git reader state is poisoned".to_string())
}

impl SourceTree for GitRef {
    /// `mtime_ns` vem dos 60 primeiros bits do blob: muda se, e só se, o conteúdo muda.
    fn list(&self) -> Result<Vec<SourceFile>, CoreError> {
        let out = git_output(&self.root, &["ls-tree", "-r", "-z", "--long", &self.commit])?;
        let mut blobs = self.blobs.lock().map_err(poisoned)?;
        blobs.clear();
        let mut files = Vec::new();
        for entry in String::from_utf8_lossy(&out)
            .split('\0')
            .filter(|e| !e.is_empty())
        {
            let Some((meta, path)) = entry.split_once('\t') else {
                continue;
            };
            let fields: Vec<&str> = meta.split_whitespace().collect();
            let [_mode, kind, sha, size] = fields.as_slice() else {
                continue;
            };
            if *kind != "blob" || !path.ends_with(".py") {
                continue;
            }
            blobs.insert(path.to_string(), (*sha).to_string());
            files.push(SourceFile {
                path: path.to_string(),
                mtime_ns: i64::from_str_radix(&sha[..15], 16).unwrap_or(0),
                size: size.parse().unwrap_or(0),
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    fn read(&self, path: &str) -> Result<String, CoreError> {
        let sha = self
            .blobs
            .lock()
            .map_err(poisoned)?
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::Source(format!("{path} is not in {}", self.commit)))?;
        let mut guard = self.batch.lock().map_err(poisoned)?;
        if guard.is_none() {
            let mut child = Command::new("git")
                .args(["cat-file", "--batch"])
                .current_dir(&self.root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| CoreError::Source("no stdin".into()))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| CoreError::Source("no stdout".into()))?;
            *guard = Some(CatFile {
                _child: child,
                stdin,
                stdout: BufReader::new(stdout),
            });
        }
        let cat = guard
            .as_mut()
            .ok_or_else(|| CoreError::Source("no cat-file".into()))?;
        writeln!(cat.stdin, "{sha}")?;
        cat.stdin.flush()?;
        let mut header = String::new();
        cat.stdout.read_line(&mut header)?;
        let size: usize = header
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CoreError::Source(format!("unexpected cat-file header: {header}")))?;
        let mut bytes = vec![0u8; size + 1];
        cat.stdout.read_exact(&mut bytes)?;
        bytes.truncate(size);
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}
