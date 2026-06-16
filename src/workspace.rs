use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::chunking::is_indexable_file_reader;
use crate::config;
use crate::jobs::{
    self, ENHANCEMENT_HEARTBEAT_TTL_SECS, INDEXING_HEARTBEAT_TTL_SECS, JobKind,
    WATCHER_HEARTBEAT_TTL_SECS,
};
use crate::walker::source_walker;

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

/// On-disk index format version. Bump when a stored-layout or chunking-semantic
/// change makes an existing index incompatible with current code.
///
/// History:
///   1 — vector keys derived from content hash (implicit; pre-versioning)
///   2 — vector keys derived from the unique chunk id (#27)
///   3 — leading doc-comments folded into the following definition chunk (#59)
///   4 — Merkle metadata fingerprints include Unix ctime (#21)
///   5 — Starlark metadata/macro AST chunks and TSX grammar selection
///   6 — Very large BUILD-like sources split target-call AST chunks
///   7 — Lexical index commits before background hash ANN enrichment
///   8 — Stable vector keys and tombstone journals preserve resumable background enrichment
///   9 — Neural vector storage uses F16 quantization
///  10 — Symbol graph persistence, F16 vectors, and portable relative paths
///  11 — Deduplicated chunk metadata, compact symbols, and on-demand call-site lookup
pub const INDEX_FORMAT_VERSION: u32 = 11;
const COMPACTION_FREE_BYTES_THRESHOLD: u64 = 16 * 1024 * 1024;
const COMPACTION_FREE_PERCENT_THRESHOLD: f64 = 20.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub id: String,
    pub root: PathBuf,
    pub created_at_unix: u64,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
    #[serde(default)]
    pub skip_gitignore: bool,
    /// Monotonically increasing counter bumped on every successful index commit.
    /// Worktree overlays record this at creation to detect base-index drift.
    #[serde(default)]
    pub index_generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    pub id: String,
    pub root: PathBuf,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
    #[serde(default)]
    pub watcher_alive: bool,
    pub chunk_count: u64,
    pub file_count: u64,
    pub index_size_bytes: u64,
    pub index_components: IndexComponentSizes,
    pub compaction: IndexCompactionHealth,
    pub vector_key_count: u64,
    pub has_neural_vectors: bool,
    pub neural_vector_count: u64,
    pub neural_coverage_percent: f64,
    pub neural_dimensions: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_model: Option<crate::embedding::NeuralModelIdentity>,
    pub reranker_candidate_limit: usize,
    #[serde(default = "default_reranker_mode")]
    pub reranker_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reranker_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reranker_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_backend: Option<String>,
    #[serde(default)]
    pub enhancing_in_progress: bool,
    #[serde(default)]
    pub enhancing_progress_count: Option<u64>,
    #[serde(default)]
    pub enhancing_phase: Option<String>,
    #[serde(default)]
    pub enhancing_paused_reason: Option<String>,
    #[serde(default)]
    pub enhancing_error: Option<String>,
    #[serde(default)]
    pub enhancing_stalled: bool,
    #[serde(default)]
    pub indexing_in_progress: bool,
    #[serde(default)]
    pub indexing_progress: Option<String>,
    #[serde(default)]
    pub indexing_stalled: bool,
    #[serde(default)]
    pub watcher_coalesced_events: Option<u64>,
    #[serde(default)]
    pub is_worktree: bool,
    #[serde(default)]
    pub base_repo_root: Option<PathBuf>,
    #[serde(default)]
    pub seeded_from_base: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexComponentSizes {
    pub metadata_bytes: u64,
    #[serde(default)]
    pub stored_chunks_bytes: u64,
    #[serde(default)]
    pub graph_bytes: u64,
    #[serde(default)]
    pub sqlite_auxiliary_bytes: u64,
    pub lexical_bytes: u64,
    pub hash_vectors_bytes: u64,
    pub neural_vectors_bytes: u64,
    pub other_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexCompactionHealth {
    pub format_version: u32,
    pub current_format_version: u32,
    pub sqlite_page_bytes: u64,
    pub sqlite_free_bytes: u64,
    pub sqlite_free_percent: f64,
    pub legacy_graph_bytes: u64,
    pub compaction_recommended: bool,
    pub healthy: bool,
}

fn default_reranker_mode() -> String {
    "deterministic".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct WorkspaceScope {
    pub rel_path: PathBuf,
    pub is_file: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIndexState {
    NotIndexed,
    Healthy,
    HealthyEmpty,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIndexHealth {
    pub state: WorkspaceIndexState,
    pub chunk_count: u64,
    pub file_count: u64,
    pub has_indexable_files: bool,
    pub issues: Vec<String>,
}

impl WorkspaceIndexHealth {
    pub fn is_queryable(&self) -> bool {
        matches!(
            self.state,
            WorkspaceIndexState::Healthy | WorkspaceIndexState::HealthyEmpty
        )
    }

    pub fn needs_rebuild(&self) -> bool {
        self.state == WorkspaceIndexState::Unhealthy
    }
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

/// Canonical representation for relative paths persisted in an index.
///
/// Rust accepts `/` as a separator on Windows, so using it everywhere keeps
/// SQLite, Tantivy, Merkle snapshots, and serialized results platform-neutral.
pub fn index_path_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
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

    pub fn job_ledger_path(&self) -> PathBuf {
        self.index_dir.join("job.json")
    }

    pub fn job_lock_path(&self) -> PathBuf {
        self.index_dir.join("job.lock")
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

    pub fn neural_profile_path(&self) -> PathBuf {
        self.index_dir.join("neural_profile")
    }

    pub fn neural_profile_name(&self) -> Option<String> {
        fs::read_to_string(self.neural_profile_path())
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    pub fn neural_model_path(&self) -> PathBuf {
        self.index_dir.join("neural_model.json")
    }

    pub fn neural_model_identity(&self) -> Option<crate::embedding::NeuralModelIdentity> {
        let contents = fs::read_to_string(self.neural_model_path()).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Returns whether a neural vector store is available for query-time use.
    /// Worktree searches can use their base workspace's neural store.
    pub fn has_neural_vectors(&self) -> bool {
        neural_store_has_vectors(&self.index_dir)
            || self
                .base_index_dir
                .as_ref()
                .is_some_and(|base| neural_store_has_vectors(base))
    }

    pub fn neural_vector_count(&self) -> u64 {
        let path = self.vector_neural_path();
        if !path.exists() {
            return 0;
        }
        let Some(identity) = self.neural_model_identity() else {
            return 0;
        };
        vector_store_size(
            &path,
            identity.dimensions,
            crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
        )
        .unwrap_or(0)
    }

    pub fn neural_coverage_percent(&self) -> f64 {
        let total = self.vector_key_count();
        if total == 0 {
            100.0
        } else {
            (self.neural_vector_count() as f64 / total as f64 * 100.0).min(100.0)
        }
    }

    pub fn neural_backend_path(&self) -> PathBuf {
        self.index_dir.join("neural_backend")
    }

    /// Sentinel recording the on-disk index format version. Bumped when the
    /// stored layout changes incompatibly so that an upgraded-but-not-rebuilt
    /// index is detected as stale and rebuilt before being served.
    pub fn index_format_version_path(&self) -> PathBuf {
        self.index_dir.join("index_format_version")
    }

    /// Returns the index format version recorded on disk, or 0 if the sentinel
    /// is missing/unreadable (i.e. an index written before versioning existed).
    pub fn read_index_format_version(&self) -> u32 {
        std::fs::read_to_string(self.index_format_version_path())
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Records the current index format version. Call after a successful index
    /// commit so the index is marked as written with the current layout.
    pub fn write_index_format_version(&self) -> std::io::Result<()> {
        std::fs::write(
            self.index_format_version_path(),
            INDEX_FORMAT_VERSION.to_string(),
        )
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

    pub fn enhancing_phase_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.phase")
    }

    pub fn enhancing_paused_path(&self) -> PathBuf {
        self.index_dir.join(".enhancing.paused")
    }

    pub fn neural_tombstones_path(&self) -> PathBuf {
        self.index_dir.join(".neural_tombstones")
    }

    pub fn neural_tombstones_processing_path(&self) -> PathBuf {
        self.index_dir.join(".neural_tombstones.processing")
    }

    pub fn hash_tombstones_path(&self) -> PathBuf {
        self.index_dir.join(".hash_tombstones")
    }

    pub fn hash_tombstones_processing_path(&self) -> PathBuf {
        self.index_dir.join(".hash_tombstones.processing")
    }

    pub fn hash_enhanced_generation_path(&self) -> PathBuf {
        self.index_dir.join(".hash_enhanced_generation")
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
        let status = jobs::job_status(self, JobKind::Watcher, WATCHER_HEARTBEAT_TTL_SECS);
        if status.record.is_some() {
            status.active()
        } else {
            is_active_pid_alive(&self.watcher_pid_path())
        }
    }

    /// Checks if an enhancement process is currently running for this workspace.
    pub fn is_enhancing_active(&self) -> bool {
        let status = jobs::job_status(self, JobKind::Enhancement, ENHANCEMENT_HEARTBEAT_TTL_SECS);
        if status.record.is_some() {
            status.active()
        } else {
            is_active_pid_alive(&self.enhancing_pid_path())
        }
    }

    /// Checks if background hash or neural enhancement still has work to do.
    pub fn needs_neural_enhancement(&self) -> bool {
        let enhancement_status =
            jobs::job_status(self, JobKind::Enhancement, ENHANCEMENT_HEARTBEAT_TTL_SECS);
        if enhancement_status.active() {
            return false;
        }

        let use_overlay = self.has_overlay() || self.base_ref_path().exists();
        let (chunk_count, _) = read_sqlite_counts(&self.index_dir);
        if chunk_count == 0 {
            return false;
        }
        let vector_key_count = read_sqlite_vector_key_count(&self.index_dir);

        let hash_path = if use_overlay {
            self.overlay_vector_path()
        } else {
            self.vector_path()
        };
        let index_generation = self
            .read_metadata()
            .ok()
            .flatten()
            .map(|metadata| metadata.index_generation)
            .unwrap_or(0);
        let hash_enhanced_generation =
            std::fs::read_to_string(self.hash_enhanced_generation_path())
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok());
        if hash_enhanced_generation != Some(index_generation)
            || self.hash_tombstones_path().exists()
            || self.hash_tombstones_processing_path().exists()
        {
            return true;
        }
        if vector_store_size(
            &hash_path,
            crate::EMBEDDING_DIMENSIONS,
            crate::vector_store::HASH_VECTOR_QUANTIZATION,
        )
        .is_none_or(|enhanced| enhanced < vector_key_count)
        {
            return true;
        }

        // Worktree-specific chunks use their overlay hash store. Neural search
        // falls through to the base workspace's shared neural store.
        if use_overlay {
            return false;
        }

        if self.neural_tombstones_path().exists()
            || self.neural_tombstones_processing_path().exists()
        {
            return true;
        }

        if enhancement_status
            .record
            .as_ref()
            .and_then(|record| record.last_error.as_deref())
            .is_some_and(|err| err.contains("neural feature not compiled"))
        {
            return false;
        }

        if self.neural_model_identity().as_ref()
            != Some(&crate::embedding::configured_neural_model_identity())
        {
            return true;
        }

        let neural_path = self.vector_neural_path();
        if !neural_path.exists() {
            return true;
        }

        // Open the persisted store for an exact count. The optimized backend
        // memory-maps this path; the portable backend validates and loads it.
        if let Ok(store) = crate::vector_store::VectorStore::open_readonly(
            &neural_path,
            crate::embedding::configured_neural_model_identity().dimensions,
            crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
        ) {
            let enhanced = store.size();
            return (enhanced as u64) < vector_key_count;
        }

        // If we can't open it but it exists and we have chunks, assume we need a rebuild/upgrade
        true
    }

    /// Triggers an atomic background spawn of the hash and neural enhancement process.
    /// Uses O_EXCL file lock mechanics to mathematically prevent race conditions
    /// even if multiple threads or processes try to spawn this simultaneously.
    pub fn trigger_background_enhancement(&self) -> Result<()> {
        let exe = std::env::current_exe()?;
        let pid_path = self.enhancing_pid_path();

        let status = jobs::job_status(self, JobKind::Enhancement, ENHANCEMENT_HEARTBEAT_TTL_SECS);
        if status.active() {
            return Ok(());
        }
        if status.stalled {
            let _ = fs::remove_file(&pid_path);
            let _ = fs::remove_file(self.enhancing_paused_path());
            let _ = fs::remove_file(self.enhancing_progress_path());
            let _ = fs::remove_file(self.enhancing_phase_path());
            let _ = jobs::finish_job(
                self,
                JobKind::Enhancement,
                "recovering-stale-worker",
                status.record.and_then(|record| record.last_error),
            );
        }
        let _ = is_active_pid_alive(&pid_path);

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

            // Lower the scheduling priority of the background process so
            // interactive work (editor, shell, search) is never starved.
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                // SAFETY: nice(2) is async-signal-safe and has no side effects
                // beyond adjusting the process niceness.
                unsafe {
                    cmd.pre_exec(|| {
                        libc::nice(10);
                        Ok(())
                    });
                }
            }

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
        // base repository's vectors. We explicitly cascade the background enhancement
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
        // Atomic write (tmp + rename) so a crash/disk-full mid-write can't
        // truncate workspace.json — a partial file would make the workspace
        // look un-indexed and silently drop its watcher on restore. Use a
        // unique temp name per write so concurrent writers (the daemon handles
        // requests in parallel) don't race on a shared temp file.
        let path = self.metadata_path();
        let tmp = path.with_file_name(format!("workspace.json.tmp.{}", uuid::Uuid::new_v4()));
        fs::write(&tmp, data)?;
        if let Err(err) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(err.into());
        }
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

    pub fn vector_key_count(&self) -> u64 {
        read_sqlite_vector_key_count(&self.index_dir)
    }

    pub fn index_component_sizes(&self) -> IndexComponentSizes {
        index_component_sizes(&self.index_dir)
    }

    pub fn index_compaction_health(&self) -> IndexCompactionHealth {
        index_compaction_health(&self.index_dir)
    }

    pub fn compact_sqlite_if_needed(&self) -> Result<bool> {
        let health = self.index_compaction_health();
        if !health.compaction_recommended || health.format_version != health.current_format_version
        {
            return Ok(false);
        }

        let mut compacted = false;
        for name in ["metadata.sqlite3", "overlay.sqlite3"] {
            let path = self.index_dir.join(name);
            if !path.exists() {
                continue;
            }
            let conn = rusqlite::Connection::open(&path)?;
            let (page_bytes, free_bytes) = sqlite_page_usage(&conn);
            if compaction_is_recommended(page_bytes, free_bytes) {
                conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
                compacted = true;
            }
        }
        Ok(compacted)
    }

    pub fn quick_index_health(&self) -> WorkspaceIndexHealth {
        self.index_health_with_options(false)
    }

    pub fn index_health(&self) -> WorkspaceIndexHealth {
        self.index_health_with_options(true)
    }

    fn index_health_with_options(&self, verify_stores: bool) -> WorkspaceIndexHealth {
        let mut issues = Vec::new();
        let metadata = self.read_metadata().ok().flatten();

        // Worktree overlays keep their own stores under overlay.* names and
        // reference the base index by path. Health must inspect the overlay
        // stores, not the base-named paths (which don't exist for an overlay),
        // otherwise a healthy overlay is wrongly flagged and rebuilt on every
        // reindex.
        let is_overlay = self.has_overlay() || self.base_ref_path().exists();
        let sqlite_p = if is_overlay {
            self.overlay_sqlite_path()
        } else {
            self.sqlite_path()
        };
        let tantivy_p = if is_overlay {
            self.overlay_tantivy_dir()
        } else {
            self.tantivy_dir()
        };
        let vector_p = if is_overlay {
            self.overlay_vector_path()
        } else {
            self.vector_path()
        };

        let has_any_index_artifacts = self.metadata_path().exists()
            || sqlite_p.exists()
            || tantivy_p.exists()
            || vector_p.exists();

        if !has_any_index_artifacts {
            return WorkspaceIndexHealth {
                state: WorkspaceIndexState::NotIndexed,
                chunk_count: 0,
                file_count: 0,
                has_indexable_files: if verify_stores {
                    workspace_has_indexable_files(&self.root, false)
                } else {
                    false
                },
                issues,
            };
        }

        if metadata.is_none() {
            issues.push("missing workspace metadata".to_string());
        }

        // Detect crashed indexing: if .indexing.pid exists but the process is
        // dead, the IndexingGuard's Drop never ran (SIGKILL / OOM / power loss).
        // The index is in an unknown partial state — force a rebuild.
        if matches!(
            legacy_pid_status(&self.indexing_pid_path(), true),
            LegacyPidStatus::Stale
        ) {
            issues.push("previous indexing process crashed (stale .indexing.pid)".to_string());
        }

        let skip_gitignore = metadata.as_ref().is_some_and(|m| m.skip_gitignore);

        if !sqlite_p.exists() {
            issues.push("missing metadata.sqlite3".to_string());
        }
        if !tantivy_p.exists() {
            issues.push("missing Tantivy index".to_string());
        }
        if !vector_p.exists() {
            issues.push("missing hash vector store".to_string());
        }
        if metadata
            .as_ref()
            .is_some_and(|m| m.last_indexed_at_unix.is_none())
        {
            issues.push("index metadata never recorded a completed run".to_string());
        }

        // A corrupt Merkle snapshot can't drive an incremental diff (diffing
        // against an empty set would re-add files but never remove chunks for
        // files deleted before the corruption). Force a full rebuild, which
        // clears the stores and reindexes from scratch.
        if crate::merkle::MerkleSnapshot::file_is_corrupt(&self.merkle_snapshot_path()) {
            issues.push("merkle snapshot is corrupt; rebuild required".to_string());
        }

        // For worktree overlays, queries also read chunks/vectors from the
        // base index. If the base predates the current format, the overlay
        // serves incompatible base data even when its own sentinel is current,
        // so force a rebuild (the overlay-index path migrates the base too).
        if let Some(base_dir) = &self.base_index_dir
            && base_dir.join("metadata.sqlite3").exists()
        {
            let base_format = std::fs::read_to_string(base_dir.join("index_format_version"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);
            if base_format != INDEX_FORMAT_VERSION {
                issues.push(format!(
                    "base index format incompatible (v{base_format} != v{INDEX_FORMAT_VERSION}); rebuild required"
                ));
            }
        }

        let cached_counts = read_cached_sqlite_counts(&self.index_dir);
        let missing_cached_stats = !verify_stores && sqlite_p.exists() && cached_counts.is_none();
        if missing_cached_stats {
            issues.push(
                "cached index statistics are missing; run `ig --doctor --deep` or rebuild"
                    .to_string(),
            );
        }
        let (chunk_count, file_count) = if verify_stores && sqlite_p.exists() {
            match read_sqlite_counts_live(&sqlite_p) {
                Ok(counts) => counts,
                Err(err) => {
                    issues.push(format!("failed to read SQLite index: {err:#}"));
                    cached_counts.unwrap_or((0, 0))
                }
            }
        } else {
            cached_counts.unwrap_or((0, 0))
        };
        let vector_key_count = if verify_stores && sqlite_p.exists() {
            read_sqlite_vector_key_count_live(&sqlite_p).unwrap_or(chunk_count)
        } else {
            chunk_count
        };

        if chunk_count > 0 && !self.merkle_snapshot_path().exists() {
            issues.push("missing merkle snapshot; rebuild required".to_string());
        }

        // Empty standalone indexes carry no queryable data to migrate. Empty
        // overlays still persist a Merkle snapshot and serve the base index,
        // so their format must be validated before incremental comparison.
        if chunk_count > 0 || is_overlay {
            let format_version = self.read_index_format_version();
            if format_version != INDEX_FORMAT_VERSION {
                issues.push(format!(
                    "index format incompatible (v{format_version} != v{INDEX_FORMAT_VERSION}); rebuild required"
                ));
            }
        }

        if chunk_count > 0 {
            if !dir_has_entries(&tantivy_p) {
                issues.push("Tantivy index directory is empty despite indexed chunks".to_string());
            }

            if verify_stores {
                match crate::indexer::open_tantivy_index(&tantivy_p) {
                    Ok((index, _)) => match index.reader() {
                        Ok(reader) => {
                            let tantivy_count = reader.searcher().num_docs();
                            if tantivy_count != chunk_count {
                                issues.push(format!(
                                    "Tantivy/SQLite chunk count mismatch ({tantivy_count} != {chunk_count})"
                                ));
                            }
                        }
                        Err(err) => issues.push(format!("failed to read Tantivy index: {err:#}")),
                    },
                    Err(err) => issues.push(format!("failed to open Tantivy index: {err:#}")),
                }

                match crate::vector_store::VectorStore::open_readonly(
                    &vector_p,
                    256,
                    crate::vector_store::HASH_VECTOR_QUANTIZATION,
                ) {
                    Ok(store) => {
                        let generation = metadata.as_ref().map(|meta| meta.index_generation);
                        let enhanced_generation =
                            std::fs::read_to_string(self.hash_enhanced_generation_path())
                                .ok()
                                .and_then(|value| value.trim().parse::<u64>().ok());
                        let has_pending_tombstones = self.hash_tombstones_path().exists()
                            || self.hash_tombstones_processing_path().exists();
                        if enhanced_generation == generation
                            && !has_pending_tombstones
                            && store.size() as u64 != vector_key_count
                        {
                            issues.push(format!(
                                "hash vector/SQLite vector-key count mismatch ({} != {vector_key_count})",
                                store.size()
                            ));
                        }
                    }
                    Err(err) => issues.push(format!("failed to open hash vector store: {err:#}")),
                }

                if !is_overlay
                    && self.vector_neural_path().exists()
                    && let Err(err) = crate::vector_store::VectorStore::open_readonly(
                        &self.vector_neural_path(),
                        self.neural_model_identity()
                            .map(|identity| identity.dimensions)
                            .unwrap_or(384),
                        crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
                    )
                {
                    issues.push(format!("failed to open neural vector store: {err:#}"));
                }

                if let Ok(snapshot) =
                    crate::merkle::MerkleSnapshot::load(&self.merkle_snapshot_path())
                {
                    match read_sqlite_file_paths(&sqlite_p) {
                        Ok(paths) => {
                            let snapshot_paths =
                                snapshot.files.keys().cloned().collect::<BTreeSet<_>>();
                            let orphaned = paths
                                .difference(&snapshot_paths)
                                .take(5)
                                .cloned()
                                .collect::<Vec<_>>();
                            if !orphaned.is_empty() {
                                issues.push(format!(
                                    "SQLite contains paths absent from merkle snapshot: {}",
                                    orphaned.join(", ")
                                ));
                            }
                        }
                        Err(err) => {
                            issues.push(format!("failed to read SQLite file paths: {err:#}"))
                        }
                    }
                }
            }
        }

        let has_indexable_files = if verify_stores && chunk_count == 0 && !missing_cached_stats {
            workspace_has_indexable_files(&self.root, skip_gitignore)
        } else {
            false
        };

        // For an overlay, zero chunks just means the worktree has no files
        // diverging from the base — the base index still serves content, so
        // this is not an unhealthy state.
        if chunk_count == 0 && has_indexable_files && !is_overlay {
            issues.push(
                "index contains zero chunks but the workspace has indexable files".to_string(),
            );
        }

        let state = if issues.is_empty() {
            if chunk_count == 0 {
                WorkspaceIndexState::HealthyEmpty
            } else {
                WorkspaceIndexState::Healthy
            }
        } else {
            WorkspaceIndexState::Unhealthy
        };

        WorkspaceIndexHealth {
            state,
            chunk_count,
            file_count,
            has_indexable_files,
            issues,
        }
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
    let reranker = crate::reranker::runtime_status();
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
        let index_components = index_component_sizes(&index_dir);
        let compaction = index_compaction_health(&index_dir);
        let vector_key_count = read_sqlite_vector_key_count(&index_dir);
        let neural_model = fs::read_to_string(index_dir.join("neural_model.json"))
            .ok()
            .and_then(|value| {
                serde_json::from_str::<crate::embedding::NeuralModelIdentity>(&value).ok()
            });
        let neural_dimensions = neural_model
            .as_ref()
            .map(|identity| identity.dimensions)
            .unwrap_or(384);
        let neural_path = index_dir.join("vectors_neural.usearch");
        let neural_vector_count = if neural_path.exists() {
            crate::vector_store::VectorStore::open_readonly(
                &neural_path,
                neural_dimensions,
                crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
            )
            .map(|store| store.size() as u64)
            .unwrap_or(0)
        } else {
            0
        };
        let has_neural_vectors = neural_vector_count > 0;
        let neural_coverage_percent = if vector_key_count > 0 {
            (neural_vector_count as f64 / vector_key_count as f64 * 100.0).min(100.0)
        } else {
            100.0
        };
        let neural_backend = fs::read_to_string(index_dir.join("neural_backend"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let neural_profile = fs::read_to_string(index_dir.join("neural_profile"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| Some(crate::embedding::configured_neural_profile_name().to_string()));

        let workspace = Workspace {
            id: metadata.id.clone(),
            root: metadata.root.clone(),
            index_dir: index_dir.clone(),
            repo_id: None,
            base_index_dir: None,
        };

        let ledger = jobs::read_job_ledger(&workspace);
        let observed_at_unix = jobs::now_unix();
        let watcher_status = jobs::job_status_at(
            &ledger,
            JobKind::Watcher,
            WATCHER_HEARTBEAT_TTL_SECS,
            observed_at_unix,
        );
        let watcher_alive = if watcher_status.record.is_some() {
            watcher_status.active()
        } else {
            is_active_pid_alive(&index_dir.join(".watcher.pid"))
        };

        let enhancement_status = jobs::job_status_at(
            &ledger,
            JobKind::Enhancement,
            ENHANCEMENT_HEARTBEAT_TTL_SECS,
            observed_at_unix,
        );
        let enhancing_in_progress = if enhancement_status.record.is_some() {
            enhancement_status.active()
        } else {
            is_active_pid_alive(&index_dir.join(".enhancing.pid"))
        };

        let indexing_status = jobs::job_status_at(
            &ledger,
            JobKind::Indexing,
            INDEXING_HEARTBEAT_TTL_SECS,
            observed_at_unix,
        );
        let indexing_in_progress = if indexing_status.record.is_some() {
            indexing_status.active()
        } else {
            is_active_pid_alive(&index_dir.join(".indexing.pid"))
        };

        let enhancing_progress_count = if enhancing_in_progress {
            let progress_path = index_dir.join(".enhancing.progress");
            std::fs::read_to_string(&progress_path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
        } else {
            None
        };

        let enhancing_phase = if enhancing_in_progress {
            let phase_path = index_dir.join(".enhancing.phase");
            std::fs::read_to_string(&phase_path)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
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

        let enhancing_error = enhancement_status
            .record
            .as_ref()
            .and_then(|record| record.last_error.clone())
            .or_else(|| {
                if !enhancing_in_progress && index_dir.join(".enhancing.error").exists() {
                    std::fs::read_to_string(index_dir.join(".enhancing.error")).ok()
                } else {
                    None
                }
            });

        let indexing_progress = if indexing_in_progress {
            let progress_path = index_dir.join(".indexing.progress");
            std::fs::read_to_string(&progress_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        let enhancing_stalled = enhancement_status.stalled;
        let indexing_stalled = indexing_status.stalled;
        let watcher_coalesced_events = watcher_status
            .record
            .as_ref()
            .and_then(|record| record.details.get("coalesced_events"))
            .and_then(|value| value.parse::<u64>().ok());

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
                watcher_alive,
                chunk_count,
                file_count,
                index_size_bytes,
                index_components,
                compaction,
                vector_key_count,
                has_neural_vectors,
                neural_vector_count,
                neural_coverage_percent,
                neural_dimensions,
                neural_profile,
                neural_model,
                reranker_candidate_limit: crate::search::rerank_candidate_limit(),
                reranker_mode: reranker.mode.clone(),
                reranker_model: reranker.model_id.clone(),
                reranker_error: reranker.error.clone(),
                neural_backend,
                enhancing_in_progress,
                enhancing_progress_count,
                enhancing_phase,
                enhancing_paused_reason,
                enhancing_error,
                enhancing_stalled,
                indexing_in_progress,
                indexing_progress,
                indexing_stalled,
                watcher_coalesced_events,
                is_worktree: ws_is_worktree,
                base_repo_root,
                seeded_from_base,
            },
        );
    }

    Ok(by_id.into_values().collect())
}

fn read_sqlite_counts(index_dir: &Path) -> (u64, u64) {
    read_cached_sqlite_counts(index_dir).unwrap_or((0, 0))
}

fn read_cached_sqlite_counts(index_dir: &Path) -> Option<(u64, u64)> {
    let overlay_path = index_dir.join("overlay.sqlite3");
    let sqlite_path = if overlay_path.exists() {
        overlay_path
    } else {
        index_dir.join("metadata.sqlite3")
    };

    if !sqlite_path.exists() {
        return Some((0, 0));
    }

    let conn = rusqlite::Connection::open_with_flags(
        &sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let chunks = conn
        .query_row(
            "SELECT value FROM _stats WHERE key = 'chunk_count'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()?;
    let files = conn
        .query_row(
            "SELECT value FROM _stats WHERE key = 'file_count'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()?;
    Some((chunks as u64, files as u64))
}

fn index_component_sizes(index_dir: &Path) -> IndexComponentSizes {
    let metadata_bytes: u64 = ["metadata.sqlite3", "overlay.sqlite3"]
        .iter()
        .map(|name| file_size(&index_dir.join(name)))
        .sum();
    let lexical_bytes: u64 = ["tantivy", "overlay_tantivy"]
        .iter()
        .map(|name| shallow_dir_size_bytes(&index_dir.join(name)))
        .sum();
    let hash_vectors_bytes: u64 = ["vectors.usearch", "overlay_vectors.usearch"]
        .iter()
        .map(|name| file_size(&index_dir.join(name)))
        .sum();
    let neural_vectors_bytes = file_size(&index_dir.join("vectors_neural.usearch"));
    let (stored_chunks_bytes, graph_bytes) = ["metadata.sqlite3", "overlay.sqlite3"]
        .iter()
        .map(|name| sqlite_tier_bytes(&index_dir.join(name)))
        .fold((0, 0), |(chunks, graph), (next_chunks, next_graph)| {
            (chunks + next_chunks, graph + next_graph)
        });
    let sqlite_auxiliary_bytes = metadata_bytes.saturating_sub(stored_chunks_bytes + graph_bytes);
    let classified = metadata_bytes + lexical_bytes + hash_vectors_bytes + neural_vectors_bytes;
    IndexComponentSizes {
        metadata_bytes,
        stored_chunks_bytes,
        graph_bytes,
        sqlite_auxiliary_bytes,
        lexical_bytes,
        hash_vectors_bytes,
        neural_vectors_bytes,
        other_bytes: dir_size_bytes(index_dir).saturating_sub(classified),
    }
}

fn index_compaction_health(index_dir: &Path) -> IndexCompactionHealth {
    let format_version = fs::read_to_string(index_dir.join("index_format_version"))
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    let mut sqlite_page_bytes = 0;
    let mut sqlite_free_bytes = 0;
    let mut legacy_graph_bytes = 0;

    for name in ["metadata.sqlite3", "overlay.sqlite3"] {
        let path = index_dir.join(name);
        let Ok(conn) = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            continue;
        };
        let (page_bytes, free_bytes) = sqlite_page_usage(&conn);
        sqlite_page_bytes += page_bytes;
        sqlite_free_bytes += free_bytes;
        legacy_graph_bytes += sqlite_named_bytes(
            &conn,
            &[
                "symbol_edges",
                "sqlite_autoindex_symbol_edges_1",
                "idx_symbol_edges_source_chunk",
            ],
        );
    }

    let sqlite_free_percent = if sqlite_page_bytes == 0 {
        0.0
    } else {
        sqlite_free_bytes as f64 / sqlite_page_bytes as f64 * 100.0
    };
    let compaction_recommended = compaction_is_recommended(sqlite_page_bytes, sqlite_free_bytes);
    let healthy = sqlite_page_bytes == 0
        || (format_version == INDEX_FORMAT_VERSION
            && legacy_graph_bytes == 0
            && !compaction_recommended);

    IndexCompactionHealth {
        format_version,
        current_format_version: INDEX_FORMAT_VERSION,
        sqlite_page_bytes,
        sqlite_free_bytes,
        sqlite_free_percent,
        legacy_graph_bytes,
        compaction_recommended,
        healthy,
    }
}

fn sqlite_tier_bytes(path: &Path) -> (u64, u64) {
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return (0, 0);
    };
    let stored_chunks_bytes = sqlite_named_bytes(&conn, &["chunks"]);
    let graph_bytes = sqlite_named_bytes(
        &conn,
        &[
            "symbols",
            "idx_symbols_name",
            "symbol_edges",
            "sqlite_autoindex_symbol_edges_1",
            "idx_symbol_edges_source_chunk",
        ],
    );
    (stored_chunks_bytes, graph_bytes)
}

fn sqlite_named_bytes(conn: &rusqlite::Connection, names: &[&str]) -> u64 {
    let Ok(mut stmt) = conn.prepare("SELECT name, SUM(pgsize) FROM dbstat GROUP BY name") else {
        return 0;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) else {
        return 0;
    };
    rows.filter_map(Result::ok)
        .filter(|(name, _)| names.contains(&name.as_str()))
        .map(|(_, bytes)| bytes.max(0) as u64)
        .sum()
}

fn sqlite_page_usage(conn: &rusqlite::Connection) -> (u64, u64) {
    let page_count = conn
        .query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
        .unwrap_or(0)
        .max(0) as u64;
    let page_size = conn
        .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
        .unwrap_or(0)
        .max(0) as u64;
    let freelist_count = conn
        .query_row("PRAGMA freelist_count", [], |row| row.get::<_, i64>(0))
        .unwrap_or(0)
        .max(0) as u64;
    (
        page_count.saturating_mul(page_size),
        freelist_count.saturating_mul(page_size),
    )
}

fn compaction_is_recommended(page_bytes: u64, free_bytes: u64) -> bool {
    free_bytes >= COMPACTION_FREE_BYTES_THRESHOLD
        && page_bytes > 0
        && free_bytes as f64 / page_bytes as f64 * 100.0 >= COMPACTION_FREE_PERCENT_THRESHOLD
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn read_sqlite_vector_key_count(index_dir: &Path) -> u64 {
    let overlay_path = index_dir.join("overlay.sqlite3");
    let sqlite_path = if overlay_path.exists() {
        overlay_path
    } else {
        index_dir.join("metadata.sqlite3")
    };
    if !sqlite_path.exists() {
        return 0;
    }

    rusqlite::Connection::open_with_flags(
        &sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
    .and_then(|conn| {
        conn.query_row(
            "SELECT value FROM _stats WHERE key = 'vector_key_count'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
    })
    .unwrap_or(0) as u64
}

fn read_sqlite_counts_live(sqlite_path: &Path) -> Result<(u64, u64)> {
    let conn = rusqlite::Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    let files: i64 = conn.query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
        row.get(0)
    })?;
    Ok((chunks as u64, files as u64))
}

fn read_sqlite_vector_key_count_live(sqlite_path: &Path) -> Result<u64> {
    let conn = rusqlite::Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let count: i64 =
        conn.query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
            row.get(0)
        })?;
    Ok(count as u64)
}

fn read_sqlite_file_paths(sqlite_path: &Path) -> Result<BTreeSet<String>> {
    let conn = rusqlite::Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut stmt = conn.prepare("SELECT DISTINCT file_path FROM chunks")?;
    let paths = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    Ok(paths)
}

fn workspace_has_indexable_files(root: &Path, skip_gitignore: bool) -> bool {
    for entry in source_walker(root, skip_gitignore).build().flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let Ok(mut file) = fs::File::open(entry.path()) else {
            continue;
        };
        // Stream the full file so minified-blob detection matches indexing
        // without allocating whole bundles during health checks.
        let Ok(indexable) = is_indexable_file_reader(entry.path(), &mut file) else {
            continue;
        };
        if indexable {
            return true;
        }
    }

    false
}

fn dir_has_entries(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
}

fn neural_store_has_vectors(index_dir: &Path) -> bool {
    let Some(dimensions) = fs::read_to_string(index_dir.join("neural_model.json"))
        .ok()
        .and_then(|value| {
            serde_json::from_str::<crate::embedding::NeuralModelIdentity>(&value).ok()
        })
        .map(|identity| identity.dimensions)
    else {
        return false;
    };
    crate::vector_store::VectorStore::open_readonly(
        &index_dir.join("vectors_neural.usearch"),
        dimensions,
        crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
    )
    .is_ok_and(|store| store.size() > 0)
}

fn vector_store_size(
    path: &Path,
    dimensions: usize,
    scalar_kind: crate::vector_store::ScalarKind,
) -> Option<u64> {
    crate::vector_store::VectorStore::open_readonly(path, dimensions, scalar_kind)
        .ok()
        .map(|store| store.size() as u64)
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

fn shallow_dir_size_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

/// Check if a background process is alive by reading the PID file.
/// Returns false (and cleans up the file) if the PID is stale.
fn is_active_pid_alive(pid_path: &Path) -> bool {
    matches!(legacy_pid_status(pid_path, true), LegacyPidStatus::Alive)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum LegacyPidStatus {
    Missing,
    Alive,
    Stale,
}

fn legacy_pid_status(pid_path: &Path, cleanup_stale: bool) -> LegacyPidStatus {
    let content = match fs::read_to_string(pid_path) {
        Ok(c) => c,
        Err(_) => return LegacyPidStatus::Missing,
    };

    let content = content.trim();
    if content.is_empty() || content == "PENDING" {
        // Temporarily locked by a concurrent spawning thread, treat as alive
        return LegacyPidStatus::Alive;
    }

    let pid: i32 = match content.parse() {
        Ok(p) => p,
        Err(_) => {
            if cleanup_stale {
                let _ = fs::remove_file(pid_path);
            }
            return LegacyPidStatus::Stale;
        }
    };

    // kill(pid, 0) checks if process exists without sending a signal.
    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive && cleanup_stale {
            let _ = fs::remove_file(pid_path);
        }
        if alive {
            LegacyPidStatus::Alive
        } else {
            LegacyPidStatus::Stale
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32) };
        let alive = if handle.is_null() {
            (unsafe { GetLastError() }) == ERROR_ACCESS_DENIED
        } else {
            let _ = unsafe { CloseHandle(handle) };
            true
        };
        if !alive && cleanup_stale {
            let _ = fs::remove_file(pid_path);
        }
        if alive {
            LegacyPidStatus::Alive
        } else {
            LegacyPidStatus::Stale
        }
    }
}

impl Workspace {
    pub fn stale_legacy_runtime_findings(&self) -> Vec<String> {
        let mut findings = Vec::new();
        let ledger = jobs::read_job_ledger(self);

        if !ledger.contains(JobKind::Watcher)
            && matches!(
                legacy_pid_status(&self.watcher_pid_path(), false),
                LegacyPidStatus::Stale
            )
        {
            findings.push("legacy watcher pid file is stale".to_string());
        }

        if !ledger.contains(JobKind::Indexing)
            && matches!(
                legacy_pid_status(&self.indexing_pid_path(), false),
                LegacyPidStatus::Stale
            )
        {
            findings.push("legacy indexing pid file is stale".to_string());
        }

        if !ledger.contains(JobKind::Enhancement)
            && matches!(
                legacy_pid_status(&self.enhancing_pid_path(), false),
                LegacyPidStatus::Stale
            )
        {
            findings.push("legacy enhancement pid file is stale".to_string());
        }

        findings
    }

    pub fn cleanup_stale_legacy_runtime_files(&self) -> Vec<String> {
        let mut cleaned = Vec::new();
        let ledger = jobs::read_job_ledger(self);

        if !ledger.contains(JobKind::Watcher)
            && matches!(
                legacy_pid_status(&self.watcher_pid_path(), true),
                LegacyPidStatus::Stale
            )
        {
            cleaned.push("removed stale legacy watcher pid file".to_string());
        }

        if !ledger.contains(JobKind::Indexing)
            && matches!(
                legacy_pid_status(&self.indexing_pid_path(), true),
                LegacyPidStatus::Stale
            )
        {
            cleaned.push("removed stale legacy indexing pid file".to_string());
        }

        if !ledger.contains(JobKind::Enhancement)
            && matches!(
                legacy_pid_status(&self.enhancing_pid_path(), true),
                LegacyPidStatus::Stale
            )
        {
            cleaned.push("removed stale legacy enhancement pid file".to_string());
        }

        cleaned
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use serial_test::serial;

    #[test]
    fn index_paths_use_forward_slashes() {
        assert_eq!(
            index_path_string(&PathBuf::from("src").join("search.rs")),
            "src/search.rs"
        );
    }

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
    #[serial]
    fn test_needs_neural_enhancement() {
        unsafe { std::env::remove_var("IVYGREP_MODEL_PROFILE") };
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
        conn.execute("INSERT INTO chunks (file_path, start_line, end_line, language, kind, text, vector_key, modified_unix) VALUES ('', 0, 0, '', '', x'', 1, 0)", []).unwrap();
        conn.execute("INSERT INTO chunks (file_path, start_line, end_line, language, kind, text, vector_key, modified_unix) VALUES ('', 0, 0, '', '', x'', 2, 0)", []).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('chunk_count', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('file_count', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('vector_key_count', 2)",
            [],
        )
        .unwrap();

        // 2 chunks, no hash vectors -> true
        assert!(ws.needs_neural_enhancement());
        assert!(!ws.has_neural_vectors());

        {
            let mut store = crate::vector_store::VectorStore::open(
                &ws.vector_path(),
                crate::EMBEDDING_DIMENSIONS,
                crate::vector_store::ScalarKind::F16,
            )
            .unwrap();
            store
                .upsert(1, vec![0.0; crate::EMBEDDING_DIMENSIONS])
                .unwrap();
            store
                .upsert(2, vec![0.0; crate::EMBEDDING_DIMENSIONS])
                .unwrap();
            store.save().unwrap();
        }
        std::fs::write(ws.hash_enhanced_generation_path(), "0").unwrap();

        // Hash vectors are complete, but neural vectors are missing -> true
        assert!(ws.needs_neural_enhancement());

        let neural_dimensions = crate::embedding::configured_neural_model_identity().dimensions;
        {
            let mut store = crate::vector_store::VectorStore::open(
                &ws.vector_neural_path(),
                neural_dimensions,
                crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
            )
            .unwrap();
            store.upsert(1, vec![0.0; neural_dimensions]).unwrap();
            store.save().unwrap();
        }

        // 1 vector < 2 chunks → true
        assert!(ws.needs_neural_enhancement());
        assert!(!ws.has_neural_vectors());

        {
            let mut store = crate::vector_store::VectorStore::open(
                &ws.vector_neural_path(),
                neural_dimensions,
                crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
            )
            .unwrap();
            store.upsert(2, vec![0.0; neural_dimensions]).unwrap();
            store.save().unwrap();
        }

        // Identity-less vectors predate complete model metadata and rebuild once.
        assert!(ws.needs_neural_enhancement());
        assert!(!ws.has_neural_vectors());
        std::fs::write(
            ws.neural_model_path(),
            serde_json::to_vec_pretty(&crate::embedding::configured_neural_model_identity())
                .unwrap(),
        )
        .unwrap();

        // 2 vectors == 2 chunks with matching identity → false
        assert!(ws.has_neural_vectors());
        assert!(!ws.needs_neural_enhancement());

        unsafe { std::env::set_var("IVYGREP_MODEL_PROFILE", "code") };
        assert!(
            ws.needs_neural_enhancement(),
            "a mismatched persisted model identity must force re-embedding"
        );
        unsafe { std::env::remove_var("IVYGREP_MODEL_PROFILE") };
    }

    #[test]
    fn worktree_overlay_only_requires_hash_enrichment() {
        let tmp = tempfile::tempdir().unwrap();
        let index_dir = tmp.path().join("index");
        std::fs::create_dir_all(&index_dir).unwrap();

        let ws = Workspace {
            id: "overlay".to_string(),
            root: tmp.path().to_path_buf(),
            index_dir: index_dir.clone(),
            repo_id: Some("repo".to_string()),
            base_index_dir: Some(tmp.path().join("base-index")),
        };

        let conn = crate::indexer::open_sqlite(&ws.overlay_sqlite_path()).unwrap();
        conn.execute("INSERT INTO chunks (file_path, start_line, end_line, language, kind, text, vector_key, modified_unix) VALUES ('', 0, 0, '', '', x'', 1, 0)", []).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('chunk_count', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('file_count', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('vector_key_count', 1)",
            [],
        )
        .unwrap();

        assert!(ws.needs_neural_enhancement());

        let mut store = crate::vector_store::VectorStore::open(
            &ws.overlay_vector_path(),
            crate::EMBEDDING_DIMENSIONS,
            crate::vector_store::ScalarKind::F16,
        )
        .unwrap();
        store
            .upsert(1, vec![0.0; crate::EMBEDDING_DIMENSIONS])
            .unwrap();
        store.save().unwrap();
        std::fs::write(ws.hash_enhanced_generation_path(), "0").unwrap();

        assert!(!ws.needs_neural_enhancement());
    }

    #[test]
    #[serial]
    fn index_health_flags_zero_chunk_index_when_workspace_has_source_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        ws.ensure_dirs().unwrap();
        ws.write_metadata(&WorkspaceMetadata {
            id: ws.id.clone(),
            root: ws.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 0,
        })
        .unwrap();
        std::fs::write(ws.sqlite_path(), "").unwrap();
        std::fs::create_dir_all(ws.tantivy_dir()).unwrap();
        std::fs::write(ws.vector_path(), "").unwrap();

        let health = ws.index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(health.has_indexable_files);
    }

    #[test]
    #[serial]
    fn quick_index_health_flags_missing_merkle_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();
        let ws = Workspace::resolve(tmp.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();
        std::fs::remove_file(ws.merkle_snapshot_path()).unwrap();

        let health = ws.quick_index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("missing merkle snapshot"))
        );
    }

    #[test]
    #[serial]
    fn quick_index_health_never_scans_to_backfill_missing_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        ws.ensure_dirs().unwrap();
        ws.write_metadata(&WorkspaceMetadata {
            id: ws.id.clone(),
            root: ws.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 1,
        })
        .unwrap();
        let sqlite = rusqlite::Connection::open(ws.sqlite_path()).unwrap();
        sqlite
            .execute_batch(
                "CREATE TABLE chunks (
                    chunk_id TEXT PRIMARY KEY,
                    file_path TEXT NOT NULL,
                    vector_key INTEGER NOT NULL
                 );
                 INSERT INTO chunks VALUES ('one', 'lib.rs', 1);",
            )
            .unwrap();
        std::fs::create_dir_all(ws.tantivy_dir()).unwrap();
        std::fs::write(ws.vector_path(), "").unwrap();
        ws.write_index_format_version().unwrap();

        let health = ws.quick_index_health();
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("cached index statistics are missing"))
        );
        let stats_table_count = sqlite
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = '_stats'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(
            stats_table_count, 0,
            "quick health must not mutate or scan the database to backfill stats"
        );
    }

    #[test]
    #[serial]
    fn quick_index_health_does_not_scan_cached_empty_indexes() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn should_only_be_seen_by_deep_health() {}\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        ws.ensure_dirs().unwrap();
        ws.write_metadata(&WorkspaceMetadata {
            id: ws.id.clone(),
            root: ws.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 1,
        })
        .unwrap();
        let sqlite = rusqlite::Connection::open(ws.sqlite_path()).unwrap();
        sqlite
            .execute_batch(
                "CREATE TABLE _stats (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
                 INSERT INTO _stats VALUES ('chunk_count', 0);
                 INSERT INTO _stats VALUES ('file_count', 0);
                 INSERT INTO _stats VALUES ('vector_key_count', 0);",
            )
            .unwrap();
        std::fs::create_dir_all(ws.tantivy_dir()).unwrap();
        std::fs::write(ws.vector_path(), "").unwrap();

        let quick = ws.quick_index_health();
        assert_eq!(quick.state, WorkspaceIndexState::HealthyEmpty);
        assert!(!quick.has_indexable_files);

        let deep = ws.index_health();
        assert_eq!(deep.state, WorkspaceIndexState::Unhealthy);
        assert!(deep.has_indexable_files);
    }

    #[test]
    #[serial]
    fn index_health_flags_tantivy_sqlite_cardinality_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("other.rs"),
            "pub fn other() -> bool { false }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();
        let sqlite = rusqlite::Connection::open(ws.sqlite_path()).unwrap();
        sqlite
            .execute(
                "DELETE FROM chunks WHERE rowid = (SELECT rowid FROM chunks LIMIT 1)",
                [],
            )
            .unwrap();
        drop(sqlite);

        let health = ws.index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("Tantivy/SQLite chunk count mismatch")),
            "{:#?}",
            health.issues
        );
    }

    #[test]
    #[serial]
    fn index_health_flags_hash_sqlite_cardinality_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();
        crate::indexer::enhance_workspace_hash(&ws, &model).unwrap();
        let sqlite = rusqlite::Connection::open(ws.sqlite_path()).unwrap();
        let vector_key = sqlite
            .query_row("SELECT vector_key FROM chunks LIMIT 1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap() as u64;
        let mut vectors = crate::vector_store::VectorStore::open(
            &ws.vector_path(),
            crate::EMBEDDING_DIMENSIONS,
            crate::vector_store::ScalarKind::F16,
        )
        .unwrap();
        vectors.remove(vector_key);
        vectors.save().unwrap();

        let health = ws.index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("hash vector/SQLite vector-key count mismatch")),
            "{:#?}",
            health.issues
        );
    }

    #[test]
    #[serial]
    fn index_health_flags_sqlite_path_missing_from_merkle_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();
        let mut snapshot = crate::merkle::MerkleSnapshot::load(&ws.merkle_snapshot_path()).unwrap();
        snapshot.files.clear();
        snapshot.save(&ws.merkle_snapshot_path()).unwrap();

        let health = ws.index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("absent from merkle snapshot")),
            "{:#?}",
            health.issues
        );
    }

    #[test]
    #[serial]
    fn index_health_flags_corrupt_optional_neural_store() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn sample() -> bool { true }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(tmp.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();
        std::fs::write(ws.vector_neural_path(), vec![0; 80]).unwrap();

        let health = ws.index_health();
        assert_eq!(health.state, WorkspaceIndexState::Unhealthy);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("failed to open neural vector store")),
            "{:#?}",
            health.issues
        );
    }

    #[test]
    fn workspace_has_indexable_files_skips_minified_only_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut banner_then_blob = b"// Copyright 2026 Example Inc.\n".to_vec();
        banner_then_blob.extend(std::iter::repeat_n(b'a', 60_000));

        std::fs::write(tmp.path().join("bundle.min.js"), banner_then_blob).unwrap();
        assert!(!workspace_has_indexable_files(tmp.path(), false));

        std::fs::write(tmp.path().join("app.js"), "const answer = 42;\n").unwrap();
        assert!(workspace_has_indexable_files(tmp.path(), false));
    }

    #[test]
    fn worktree_detects_base_neural_vector_store() {
        let tmp = tempfile::tempdir().unwrap();
        let base_index_dir = tmp.path().join("base");
        let overlay_index_dir = tmp.path().join("overlay");
        std::fs::create_dir_all(&base_index_dir).unwrap();
        std::fs::create_dir_all(&overlay_index_dir).unwrap();

        let ws = Workspace {
            id: "overlay".to_string(),
            root: tmp.path().to_path_buf(),
            index_dir: overlay_index_dir,
            repo_id: None,
            base_index_dir: Some(base_index_dir.clone()),
        };
        assert!(!ws.has_neural_vectors());

        let identity = crate::embedding::configured_neural_model_identity();
        let mut store = crate::vector_store::VectorStore::open(
            &base_index_dir.join("vectors_neural.usearch"),
            identity.dimensions,
            crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
        )
        .unwrap();
        std::fs::write(
            base_index_dir.join("neural_model.json"),
            serde_json::to_vec_pretty(&identity).unwrap(),
        )
        .unwrap();
        store.save().unwrap();
        assert!(!ws.has_neural_vectors());

        store.upsert(1, vec![0.0; identity.dimensions]).unwrap();
        store.save().unwrap();
        assert!(ws.has_neural_vectors());
    }

    #[test]
    #[serial]
    fn component_sizes_and_compaction_health_report_v11_tiers() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let ws = Workspace::resolve(root.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&ws, &model).unwrap();

        let sizes = ws.index_component_sizes();
        assert!(sizes.stored_chunks_bytes > 0);
        assert!(sizes.graph_bytes > 0);
        assert!(sizes.sqlite_auxiliary_bytes > 0);
        assert!(
            sizes.stored_chunks_bytes + sizes.graph_bytes + sizes.sqlite_auxiliary_bytes
                <= sizes.metadata_bytes
        );

        let health = ws.index_compaction_health();
        assert_eq!(health.format_version, INDEX_FORMAT_VERSION);
        assert_eq!(health.legacy_graph_bytes, 0);
        assert!(!health.compaction_recommended);
        assert!(health.healthy);
    }

    #[test]
    #[serial]
    fn compact_sqlite_reclaims_large_freelist() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let ws = Workspace::resolve(root.path()).unwrap();
        ws.ensure_dirs().unwrap();
        let conn = crate::indexer::open_sqlite(&ws.sqlite_path()).unwrap();
        let payload = vec![b'x'; 1024 * 1024];
        for index in 0..20 {
            conn.execute(
                "INSERT INTO chunks (
                    file_path, start_line, end_line, language, kind, text,
                    vector_key, modified_unix, is_ignored
                 ) VALUES ('generated.rs', 1, 1, 'rust', 'Function', ?1, ?2, 0, 0)",
                rusqlite::params![payload, index],
            )
            .unwrap();
        }
        conn.execute("DELETE FROM chunks", []).unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(conn);
        ws.write_index_format_version().unwrap();

        let before = ws.index_compaction_health();
        assert!(before.compaction_recommended, "{before:#?}");
        assert!(ws.compact_sqlite_if_needed().unwrap());
        let after = ws.index_compaction_health();
        assert!(!after.compaction_recommended, "{after:#?}");
        assert!(after.sqlite_free_bytes < before.sqlite_free_bytes);
        assert!(after.healthy);
    }
}
