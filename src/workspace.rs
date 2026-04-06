use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub root: PathBuf,
    pub index_dir: PathBuf,
    /// Stable repo-level identifier shared by all worktrees of the same repository.
    /// `None` for non-git directories.
    #[serde(default)]
    pub repo_id: Option<String>,
    /// Path to the base (main) worktree's index directory.
    /// `Some(...)` only when this workspace is a git worktree (not the main checkout).
    #[serde(default)]
    pub base_index_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub id: String,
    pub root: PathBuf,
    pub created_at_unix: u64,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
    #[serde(default)]
    pub skip_gitignore: bool,
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
    pub enhancing_paused_reason: Option<String>,
    #[serde(default)]
    pub enhancing_error: Option<String>,
    #[serde(default)]
    pub indexing_in_progress: bool,
    #[serde(default)]
    pub indexing_progress: Option<String>,
    #[serde(default)]
    pub is_worktree: bool,
    #[serde(default)]
    pub base_repo_root: Option<PathBuf>,
    #[serde(default)]
    pub seeded_from_base: bool,
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

        let (repo_id, base_index_dir) = match git_common_dir(&root) {
            Some(common_dir) => {
                let rid = repo_id_from_common_dir(&common_dir);
                // If the common dir's parent is different from root, we are a worktree
                let main_root = git_main_worktree_root(&root);
                let base = if let Some(ref main) = main_root {
                    if *main != root {
                        let main_id = workspace_id(main);
                        Some(config::indexes_root()?.join(&main_id))
                    } else {
                        None
                    }
                } else {
                    None
                };
                (Some(rid), base)
            }
            None => (None, None),
        };

        Ok(Self {
            id,
            root,
            index_dir,
            repo_id,
            base_index_dir,
        })
    }

    /// Returns true if this workspace is a git worktree (not the main checkout).
    pub fn is_worktree(&self) -> bool {
        self.base_index_dir.is_some()
    }

    /// Returns the root path of the main worktree, if this is a worktree.
    pub fn main_worktree_root(&self) -> Option<PathBuf> {
        git_main_worktree_root(&self.root).filter(|main| *main != self.root)
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

    // ── Overlay paths (worktree-only, thin per-worktree stores) ──────────

    /// SQLite containing only divergent chunks + tombstones for this worktree.
    pub fn overlay_sqlite_path(&self) -> PathBuf {
        self.index_dir.join("overlay.sqlite3")
    }

    /// Tantivy index containing only divergent chunks for this worktree.
    pub fn overlay_tantivy_dir(&self) -> PathBuf {
        self.index_dir.join("overlay_tantivy")
    }

    /// Vector store containing only divergent vectors for this worktree.
    pub fn overlay_vector_path(&self) -> PathBuf {
        self.index_dir.join("overlay_vectors.usearch")
    }

    /// Returns true if this workspace has an active overlay (is_worktree + overlay exists).
    pub fn has_overlay(&self) -> bool {
        self.is_worktree() && self.overlay_sqlite_path().exists()
    }

    /// Path to the base reference JSON file recording which base we seeded from.
    pub fn base_ref_path(&self) -> PathBuf {
        self.index_dir.join("base_ref.json")
    }

    /// PID file written by the background `--enhance-internal` process.
    /// Contains the PID so `--status` can detect whether enhancement is in progress.
    pub fn enhancing_pid_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.pid")
    }

    pub fn enhancing_progress_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.progress")
    }

    pub fn enhancing_paused_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.paused")
    }

    pub fn indexing_pid_path(&self) -> PathBuf {
        self.index_dir.join(".indexing.pid")
    }

    pub fn indexing_progress_path(&self) -> PathBuf {
        self.index_dir.join(".indexing.progress")
    }

    /// PID file written by the daemon when it starts watching this workspace.
    /// Allows the CLI to skip expensive Merkle scans when a live watcher is confirmed.
    pub fn watcher_pid_path(&self) -> PathBuf {
        self.index_dir.join(".watcher.pid")
    }

    /// Trust-but-verify: check if a filesystem watcher daemon is alive for this workspace.
    /// Returns true only if the PID file exists AND the process is still running.
    pub fn is_watcher_alive(&self) -> bool {
        is_active_pid_alive(&self.watcher_pid_path())
    }

    /// Checks if an enhancement process is currently running for this workspace.
    pub fn is_enhancing_active(&self) -> bool {
        is_active_pid_alive(&self.enhancing_pid_path())
    }

    /// Checks if we need to trigger neural enhancement (e.g. if we have un-enhanced chunks).
    pub fn needs_neural_enhancement(&self) -> bool {
        if self.is_enhancing_active() {
            return false;
        }

        let (chunk_count, _) = read_sqlite_counts(&self.index_dir);
        if chunk_count == 0 {
            return false;
        }

        let neural_path = self.vector_neural_path();
        if !neural_path.exists() {
            return true;
        }

        // Fast metadata size estimate. Each quantized F32 vector / I8 element is 384 bytes
        // plus Usearch headers and hash metadata. If it's very small it's probably 0 chunks.
        // For exact size, we memory-map it (takes < 1ms).
        if let Ok(store) = crate::vector_store::VectorStore::open_readonly(
            &neural_path,
            384,
            crate::vector_store::ScalarKind::F32,
        ) {
            let enhanced = store.size();
            return (enhanced as u64) < chunk_count;
        }

        // If we can't open it but it exists and we have chunks, assume we need a rebuild/upgrade
        true
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

        // If this is a worktree overlay, its hybrid search strongly relies on the
        // base repository's vectors. We explicitly cascade the neural enhancement
        // trigger so the base index receives upgrades in the background too.
        if let Some(main_root) = self.main_worktree_root()
            && let Ok(base_ws) = Workspace::resolve(&main_root)
            && base_ws.needs_neural_enhancement()
        {
            let _ = base_ws.trigger_background_enhancement();
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
    hex::encode(xxhash_rust::xxh3::xxh3_128(root.to_string_lossy().as_bytes()).to_le_bytes())
}

/// Compute a stable repo-level ID from the git common directory path.
/// All worktrees of the same repo will return the same ID.
pub fn repo_id_from_common_dir(common_dir: &Path) -> String {
    let mut prefix = b"repo:".to_vec();
    prefix.extend_from_slice(common_dir.to_string_lossy().as_bytes());
    hex::encode(xxhash_rust::xxh3::xxh3_128(&prefix).to_le_bytes())
}

/// Get the git common directory for a repository root.
/// For regular repos this is `<root>/.git`, for worktrees this is the main repo's `.git`.
/// Returns `None` if not a git repository.
pub fn git_common_dir(root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(&raw);
    // git may return a relative path — resolve relative to root
    let resolved = if path.is_absolute() {
        path
    } else {
        root.join(&path)
    };
    // Canonicalize to resolve symlinks and ../ components
    resolved.canonicalize().ok().or(Some(resolved))
}

/// Get the root directory of the main worktree for a repository.
/// For a regular checkout, this returns the same root.
/// For a worktree, this returns the main checkout's root.
fn git_main_worktree_root(root: &Path) -> Option<PathBuf> {
    let git_entry = root.join(".git");
    if git_entry.is_file() {
        // This is a worktree — .git is a file containing "gitdir: ..."
        // The main worktree root is the parent of the common dir
        let common = git_common_dir(root)?;
        // common_dir is like /path/to/main/.git — its parent is the main root
        // But we need to be careful: common_dir might end with /.git
        let parent = common.parent()?;
        let parent_name = parent.file_name()?.to_str()?;
        if parent_name == ".git" {
            // common_dir is /path/to/main/.git → main root is /path/to/main
            // Wait, that means parent IS .git, so the main root is parent's parent
            // Actually no — git_common_dir returns /path/to/main/.git directly
            // So the main root is parent of the common_dir
            return parent.parent().map(|p| p.to_path_buf());
        }
        // common_dir might be /path/to/main/.git itself
        Some(parent.to_path_buf())
    } else if git_entry.is_dir() {
        // Regular checkout — this IS the main worktree
        Some(root.to_path_buf())
    } else {
        None
    }
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

        let enhancing_paused_reason = if enhancing_in_progress {
            let paused_path = index_dir.join(".enhancing.paused");
            std::fs::read_to_string(&paused_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        let enhancing_error =
            if !enhancing_in_progress && index_dir.join(".enhancing.error").exists() {
                std::fs::read_to_string(index_dir.join(".enhancing.error")).ok()
            } else {
                None
            };

        let indexing_progress = if indexing_in_progress {
            let progress_path = index_dir.join(".indexing.progress");
            std::fs::read_to_string(&progress_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        let ws_is_worktree = metadata.root.join(".git").is_file();
        let base_repo_root = if ws_is_worktree {
            git_main_worktree_root(&metadata.root).filter(|main| *main != metadata.root)
        } else {
            None
        };
        let seeded_from_base = index_dir.join("base_ref.json").exists();

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
                enhancing_paused_reason,
                enhancing_error,
                indexing_in_progress,
                indexing_progress,
                is_worktree: ws_is_worktree,
                base_repo_root,
                seeded_from_base,
            },
        );
    }

    Ok(by_id.into_values().collect())
}

fn read_sqlite_counts(index_dir: &Path) -> (u64, u64) {
    let overlay_path = index_dir.join("overlay.sqlite3");
    let sqlite_path = if overlay_path.exists() {
        overlay_path
    } else {
        index_dir.join("metadata.sqlite3")
    };

    if !sqlite_path.exists() {
        return (0, 0);
    }

    // Try read-only first for speed (no CREATE TABLE / PRAGMA overhead).
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        &sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return (0, 0);
    };

    // Fast path: read from cached _stats table (O(1) lookup).
    let cached_chunks = conn.query_row(
        "SELECT value FROM _stats WHERE key = 'chunk_count'",
        [],
        |row| row.get::<_, i64>(0),
    );
    let cached_files = conn.query_row(
        "SELECT value FROM _stats WHERE key = 'file_count'",
        [],
        |row| row.get::<_, i64>(0),
    );

    if let (Ok(c), Ok(f)) = (cached_chunks, cached_files) {
        return (c as u64, f as u64);
    }

    // Slow path: _stats table doesn't exist yet (pre-migration DB).
    // Try to open read-write and cache counts. If the DB is locked
    // (e.g., by the enhancer), fall back to a live read-only COUNT.
    drop(conn);

    // Try non-blocking write migration first
    if let Ok(conn) = rusqlite::Connection::open(&sqlite_path) {
        conn.busy_timeout(std::time::Duration::from_millis(100))
            .ok();
        if conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS _stats (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
            )
            .is_ok()
        {
            let chunks: i64 = conn
                .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
                .unwrap_or(0);
            let files: i64 = conn
                .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
                    row.get(0)
                })
                .unwrap_or(0);
            let _ = conn.execute(
                "INSERT OR REPLACE INTO _stats (key, value) VALUES ('chunk_count', ?1)",
                rusqlite::params![chunks],
            );
            let _ = conn.execute(
                "INSERT OR REPLACE INTO _stats (key, value) VALUES ('file_count', ?1)",
                rusqlite::params![files],
            );
            return (chunks as u64, files as u64);
        }
    }

    // DB is locked — do a read-only live COUNT (won't cache, but won't block)
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

/// Fast index size estimate by stat-ing known index files instead of
/// recursively walking potentially 17+ GB of index directories.
fn dir_size_bytes(dir: &Path) -> u64 {
    let known_files = [
        "metadata.sqlite3",
        "metadata.sqlite3-wal",
        "metadata.sqlite3-shm",
        "vectors.usearch",
        "vectors_neural.usearch",
        "overlay.sqlite3",
        "overlay.sqlite3-wal",
        "overlay.sqlite3-shm",
        "overlay_vectors.usearch",
        "merkle_snapshot.json",
        "workspace.json",
    ];

    let mut total = 0u64;
    for name in &known_files {
        if let Ok(meta) = fs::metadata(dir.join(name)) {
            total += meta.len();
        }
    }

    // Add Tantivy directories
    for t_dir in ["tantivy", "overlay_tantivy"] {
        let path = dir.join(t_dir);
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata()
                    && meta.is_file()
                {
                    total += meta.len();
                }
            }
        }
    }

    total
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

    #[test]
    fn test_needs_neural_enhancement() {
        let tmp = tempfile::tempdir().unwrap();
        let index_dir = tmp.path().join("index");
        std::fs::create_dir_all(&index_dir).unwrap();

        let ws = Workspace {
            id: "test".to_string(),
            root: tmp.path().to_path_buf(),
            index_dir: index_dir.clone(),
            repo_id: None,
            base_index_dir: None,
        };

        // No DB file exists yet → chunk_count is 0 → false
        assert!(!ws.needs_neural_enhancement());

        // Insert 2 chunks into the database
        let conn = crate::indexer::open_sqlite(&index_dir.join("metadata.sqlite3")).unwrap();
        conn.execute("INSERT INTO chunks (chunk_id, file_path, start_line, end_line, language, kind, text, content_hash, vector_key, modified_unix) VALUES ('1', '', 0, 0, '', '', x'', '0', 1, 0)", []).unwrap();
        conn.execute("INSERT INTO chunks (chunk_id, file_path, start_line, end_line, language, kind, text, content_hash, vector_key, modified_unix) VALUES ('2', '', 0, 0, '', '', x'', '0', 2, 0)", []).unwrap();

        // 2 chunks, no neural vectors → true
        assert!(ws.needs_neural_enhancement());

        {
            let mut store = crate::vector_store::VectorStore::open(
                &ws.vector_neural_path(),
                384,
                crate::vector_store::ScalarKind::F32,
            )
            .unwrap();
            store.upsert(1, vec![0.0; 384]);
            store.save().unwrap();
        }

        // 1 vector < 2 chunks → true
        assert!(ws.needs_neural_enhancement());

        {
            let mut store = crate::vector_store::VectorStore::open(
                &ws.vector_neural_path(),
                384,
                crate::vector_store::ScalarKind::F32,
            )
            .unwrap();
            store.upsert(2, vec![0.0; 384]);
            store.save().unwrap();
        }

        // 2 vectors == 2 chunks → false
        assert!(!ws.needs_neural_enhancement());
    }
}
