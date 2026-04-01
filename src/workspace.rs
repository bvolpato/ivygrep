use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub root: PathBuf,
    pub index_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub id: String,
    pub root: PathBuf,
    pub created_at_unix: u64,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    pub id: String,
    pub root: PathBuf,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
    pub chunk_count: u64,
    pub file_count: u64,
    pub index_size_bytes: u64,
    pub has_neural_vectors: bool,
    pub neural_vector_count: u64,
    #[serde(default)]
    pub enhancing_in_progress: bool,
    #[serde(default)]
    pub enhancing_progress_count: Option<u64>,
    #[serde(default)]
    pub indexing_in_progress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkspaceScope {
    pub rel_path: PathBuf,
    pub is_file: bool,
}

impl WorkspaceScope {
    pub fn matches(&self, rel_path: &Path) -> bool {
        if self.is_file {
            rel_path == self.rel_path
        } else {
            rel_path.starts_with(&self.rel_path)
        }
    }
}

impl Workspace {
    pub fn resolve(path: &Path) -> Result<Self> {
        let root = detect_workspace_root(path)?;
        let id = workspace_id(&root);
        let index_dir = config::indexes_root()?.join(&id);

        Ok(Self {
            id,
            root,
            index_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.index_dir)?;
        Ok(())
    }

    pub fn metadata_path(&self) -> PathBuf {
        self.index_dir.join("workspace.json")
    }

    pub fn sqlite_path(&self) -> PathBuf {
        self.index_dir.join("metadata.sqlite3")
    }

    pub fn tantivy_dir(&self) -> PathBuf {
        self.index_dir.join("tantivy")
    }

    pub fn vector_path(&self) -> PathBuf {
        self.index_dir.join("vectors.usearch")
    }

    pub fn vector_neural_path(&self) -> PathBuf {
        self.index_dir.join("vectors_neural.usearch")
    }

    /// PID file written by the background `--enhance-internal` process.
    /// Contains the PID so `--status` can detect whether enhancement is in progress.
    pub fn enhancing_pid_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.pid")
    }

    pub fn enhancing_progress_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.progress")
    }

    pub fn indexing_pid_path(&self) -> PathBuf {
        self.index_dir.join(".indexing.pid")
    }

    /// Checks if an enhancement process is currently running for this workspace.
    pub fn is_enhancing_active(&self) -> bool {
        is_active_pid_alive(&self.enhancing_pid_path())
    }

    /// Triggers an atomic background spawn of the neural enhancement process.
    /// Uses O_EXCL file lock mechanics to mathematically prevent race conditions
    /// even if multiple threads or processes try to spawn this simultaneously.
    pub fn trigger_background_enhancement(&self) -> Result<()> {
        let exe = std::env::current_exe()?;
        let pid_path = self.enhancing_pid_path();

        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pid_path);

        if lock.is_ok() {
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("--enhance-internal").arg(&self.root);
            cmd.stdin(std::process::Stdio::null());
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            if let Ok(mut child) = cmd.spawn() {
                let _ = std::fs::write(&pid_path, child.id().to_string());

                // Spawn a detached thread solely to waitpid() the child.
                // Without this, the background process becomes a <defunct> zombie
                // in the daemon's process table forever when it exits, causing
                // `kill(pid, 0)` liveness checks to falsely return positive infinitely!
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            } else {
                let _ = std::fs::remove_file(&pid_path);
            }
        }

        Ok(())
    }

    pub fn merkle_snapshot_path(&self) -> PathBuf {
        self.index_dir.join("merkle_snapshot.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.index_dir.join("index.lock")
    }

    pub fn write_metadata(&self, metadata: &WorkspaceMetadata) -> Result<()> {
        let data = serde_json::to_vec_pretty(metadata)?;
        fs::write(self.metadata_path(), data)?;
        Ok(())
    }

    pub fn read_metadata(&self) -> Result<Option<WorkspaceMetadata>> {
        let path = self.metadata_path();
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(path)?;
        let parsed = serde_json::from_slice(&data)?;
        Ok(Some(parsed))
    }

    pub fn exists(&self) -> bool {
        self.index_dir.exists()
    }
}

pub fn detect_workspace_root(path: &Path) -> Result<PathBuf> {
    let mut current = config::canonicalize_lossy(path)?;

    if current.is_file() {
        current = current
            .parent()
            .map(Path::to_path_buf)
            .context("file has no parent directory")?;
    }

    let mut cursor = current.clone();
    loop {
        if cursor.join(".git").exists() {
            return Ok(cursor);
        }

        if !cursor.pop() {
            break;
        }
    }

    Ok(current)
}

pub fn resolve_workspace_and_scope(path: &Path) -> Result<(Workspace, Option<WorkspaceScope>)> {
    let canonical = config::canonicalize_lossy(path)?;
    let metadata = fs::metadata(&canonical)
        .with_context(|| format!("failed to inspect path {}", canonical.display()))?;
    let scope_is_file = metadata.is_file();

    let workspace = Workspace::resolve(&canonical)?;
    let rel_path = canonical
        .strip_prefix(&workspace.root)
        .with_context(|| {
            format!(
                "path {} is not inside workspace root {}",
                canonical.display(),
                workspace.root.display()
            )
        })?
        .to_path_buf();

    let scope = if rel_path.as_os_str().is_empty() {
        None
    } else {
        Some(WorkspaceScope {
            rel_path,
            is_file: scope_is_file,
        })
    };

    Ok((workspace, scope))
}

pub fn workspace_id(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

pub fn list_workspaces() -> Result<Vec<WorkspaceStatus>> {
    let root = config::indexes_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }

    let mut by_id = BTreeMap::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let metadata_path = entry.path().join("workspace.json");
        if !metadata_path.exists() {
            continue;
        }

        let raw = fs::read(&metadata_path).with_context(|| {
            format!(
                "failed reading workspace metadata at {}",
                metadata_path.display()
            )
        })?;
        let metadata: WorkspaceMetadata = serde_json::from_slice(&raw)?;

        let index_dir = entry.path();
        let (chunk_count, file_count) = read_sqlite_counts(&index_dir);
        let index_size_bytes = dir_size_bytes(&index_dir);
        let neural_path = index_dir.join("vectors_neural.usearch");
        let has_neural_vectors = neural_path.exists();
        let neural_vector_count = if has_neural_vectors {
            neural_path.metadata().map(|m| m.len()).unwrap_or(0) / 4 // rough estimate
        } else {
            0
        };

        // Check if enhancement is actively running
        let pid_path = index_dir.join(".enhancing.pid");
        let enhancing_in_progress = is_active_pid_alive(&pid_path);

        // Check if indexing is actively running
        let indexing_pid_path = index_dir.join(".indexing.pid");
        let indexing_in_progress = is_active_pid_alive(&indexing_pid_path);
        
        let enhancing_progress_count = if enhancing_in_progress {
            let progress_path = index_dir.join(".enhancing.progress");
            std::fs::read_to_string(&progress_path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
        } else {
            None
        };

        by_id.insert(
            metadata.id.clone(),
            WorkspaceStatus {
                id: metadata.id,
                root: metadata.root,
                last_indexed_at_unix: metadata.last_indexed_at_unix,
                watch_enabled: metadata.watch_enabled,
                chunk_count,
                file_count,
                index_size_bytes,
                has_neural_vectors,
                neural_vector_count,
                enhancing_in_progress,
                enhancing_progress_count,
                indexing_in_progress,
            },
        );
    }

    Ok(by_id.into_values().collect())
}

fn read_sqlite_counts(index_dir: &Path) -> (u64, u64) {
    let sqlite_path = index_dir.join("metadata.sqlite3");
    if !sqlite_path.exists() {
        return (0, 0);
    }
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        &sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return (0, 0);
    };
    let chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .unwrap_or(0);
    let files: i64 = conn
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    (chunks as u64, files as u64)
}

fn dir_size_bytes(dir: &Path) -> u64 {
    fn walk(path: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let ft = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                if ft.is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else if ft.is_dir() {
                    total += walk(&entry.path());
                }
            }
        }
        total
    }
    walk(dir)
}

/// Check if a background process is alive by reading the PID file.
/// Returns false (and cleans up the file) if the PID is stale.
fn is_active_pid_alive(pid_path: &Path) -> bool {
    let content = match fs::read_to_string(pid_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let content = content.trim();
    if content.is_empty() || content == "PENDING" {
        // Temporarily locked by a concurrent spawning thread, treat as alive
        return true;
    }

    let pid: i32 = match content.parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = fs::remove_file(pid_path);
            return false;
        }
    };

    // kill(pid, 0) checks if process exists without sending a signal
    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            let _ = fs::remove_file(pid_path);
        }
        alive
    }
    #[cfg(not(unix))]
    {
        // On non-unix, just check if the file exists (best effort)
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn resolve_workspace_and_scope_tracks_subpaths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), "pub fn sample() {}\n").unwrap();
        let canonical_root = config::canonicalize_lossy(tmp.path()).unwrap();

        let (workspace_dir, scope_dir) =
            resolve_workspace_and_scope(&tmp.path().join("src")).unwrap();
        assert_eq!(workspace_dir.root, canonical_root);
        assert_eq!(
            scope_dir,
            Some(WorkspaceScope {
                rel_path: PathBuf::from("src"),
                is_file: false,
            })
        );

        let (workspace_file, scope_file) =
            resolve_workspace_and_scope(&tmp.path().join("src/lib.rs")).unwrap();
        assert_eq!(workspace_file.root, canonical_root);
        assert_eq!(
            scope_file,
            Some(WorkspaceScope {
                rel_path: PathBuf::from("src/lib.rs"),
                is_file: true,
            })
        );
    }
}
