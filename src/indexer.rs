use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use tantivy::directory::error::{DeleteError, LockError, OpenReadError, OpenWriteError};
use tantivy::directory::{
    Directory, DirectoryLock, FileHandle, Lock, MmapDirectory, WatchCallback, WatchHandle, WritePtr,
};
use tantivy::schema::{
    Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::{Index as TantivyIndex, TantivyDocument, Term};

use crate::text::{CODE_TOKENIZER_NAME, build_code_analyzer};

use crate::chunking::{Chunk, chunk_source, is_indexable_file};
use crate::embedding::EmbeddingModel;
use crate::jobs::{self, JobKind, JobUpdate};
use crate::merkle::{MerkleDiff, MerkleSnapshot, normalized_indexable_content};
use crate::vector_store::{
    HASH_VECTOR_QUANTIZATION, NEURAL_VECTOR_QUANTIZATION, ScalarKind, VectorStore,
};
use crate::workspace::{Workspace, WorkspaceMetadata, index_path_string};

const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const TANTIVY_WRITE_RETRY_ATTEMPTS: u32 = 16;
const TANTIVY_WRITE_RETRY_MAX_DELAY_MS: u64 = 800;
const TANTIVY_INDEX_RETRY_ATTEMPTS: u32 = 3;
const TANTIVY_INDEX_RETRY_BASE_DELAY_MS: u64 = 250;

#[derive(Clone, Debug)]
struct RetryingDirectory<D> {
    inner: D,
}

impl<D> RetryingDirectory<D> {
    fn new(inner: D) -> Self {
        Self { inner }
    }
}

fn open_write_with_retry<F>(mut open: F) -> Result<WritePtr, OpenWriteError>
where
    F: FnMut() -> Result<WritePtr, OpenWriteError>,
{
    for attempt in 0..TANTIVY_WRITE_RETRY_ATTEMPTS {
        match open() {
            Ok(writer) => return Ok(writer),
            Err(OpenWriteError::IoError { io_error, .. })
                if io_error.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt + 1 < TANTIVY_WRITE_RETRY_ATTEMPTS =>
            {
                let delay_ms = (25_u64 << attempt).min(TANTIVY_WRITE_RETRY_MAX_DELAY_MS);
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            Err(err) => return Err(err),
        }
    }
    unreachable!("the retry loop always returns on its final attempt")
}

impl<D> Directory for RetryingDirectory<D>
where
    D: Directory + Clone + std::fmt::Debug,
{
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> Result<std::sync::Arc<dyn FileHandle>, OpenReadError> {
        self.inner.get_file_handle(path)
    }

    fn delete(&self, path: &Path) -> Result<(), DeleteError> {
        self.inner.delete(path)
    }

    fn exists(&self, path: &Path) -> Result<bool, OpenReadError> {
        self.inner.exists(path)
    }

    fn open_write(&self, path: &Path) -> Result<WritePtr, OpenWriteError> {
        open_write_with_retry(|| self.inner.open_write(path))
    }

    fn atomic_read(&self, path: &Path) -> Result<Vec<u8>, OpenReadError> {
        self.inner.atomic_read(path)
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        self.inner.atomic_write(path, data)
    }

    fn sync_directory(&self) -> std::io::Result<()> {
        self.inner.sync_directory()
    }

    fn acquire_lock(&self, lock: &Lock) -> Result<DirectoryLock, LockError> {
        self.inner.acquire_lock(lock)
    }

    fn watch(&self, watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        self.inner.watch(watch_callback)
    }
}

fn compress_text(text: &str) -> Vec<u8> {
    zstd::encode_all(text.as_bytes(), 1).unwrap_or_else(|_| text.as_bytes().to_vec())
}

pub fn decompress_text(raw: Vec<u8>) -> String {
    if raw.starts_with(ZSTD_MAGIC) {
        zstd::decode_all(&raw[..])
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_else(|| String::from_utf8_lossy(&raw).into_owned())
    } else {
        String::from_utf8(raw)
            .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingSummary {
    pub workspace_id: String,
    pub indexed_files: usize,
    pub deleted_files: usize,
    pub total_chunks: usize,
    #[serde(default)]
    pub phase_timings: IndexingPhaseTimings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexingPhaseTimings {
    pub discovery_ms: f64,
    pub persist_ms: f64,
    pub finalize_ms: f64,
}

#[derive(Debug, Clone)]
pub struct IndexedChunk {
    pub chunk_id: String,
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub kind: String,
    pub text: String,
    pub content_hash: String,
    pub vector_key: u64,
    pub is_ignored: bool,
}

#[derive(Debug, Clone)]
struct PreparedIndexedChunk {
    chunk: IndexedChunk,
    compressed_text: Vec<u8>,
}

fn prepare_indexed_chunk(chunk: IndexedChunk) -> PreparedIndexedChunk {
    let compressed_text = compress_text(&chunk.text);
    PreparedIndexedChunk {
        chunk,
        compressed_text,
    }
}

type IndexedFileBatch = Vec<(PathBuf, Vec<PreparedIndexedChunk>)>;

struct IndexBatchProducer {
    receiver: Option<std::sync::mpsc::Receiver<IndexedFileBatch>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl IndexBatchProducer {
    fn new(
        receiver: std::sync::mpsc::Receiver<IndexedFileBatch>,
        handle: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            handle: Some(handle),
        }
    }

    fn recv(&self) -> Option<IndexedFileBatch> {
        self.receiver.as_ref()?.recv().ok()
    }

    fn stop(&mut self) {
        drop(self.receiver.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    fn finish(mut self) -> Result<()> {
        drop(self.receiver.take());
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("index batch producer thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for IndexBatchProducer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
pub struct TantivyFields {
    pub vector_key: Field,
    pub file_path: Field,
    pub start_line: Field,
    pub end_line: Field,
    pub language: Field,
    pub kind: Field,
    pub text: Field,
    pub is_ignored: Option<Field>,
    pub file_path_text: Option<Field>,
    pub signature: Option<Field>,
}

#[derive(Debug, Clone)]
pub struct StorageHandles {
    pub sqlite_path: PathBuf,
    pub tantivy_dir: PathBuf,
    pub vector_path: PathBuf,
}

pub fn workspace_is_indexed(workspace: &Workspace) -> bool {
    workspace.quick_index_health().is_queryable()
}

pub fn remove_workspace_index(workspace: &Workspace) -> Result<()> {
    if workspace.index_dir.exists() {
        fs::remove_dir_all(&workspace.index_dir)?;
    }
    Ok(())
}

/// Remove all index contents EXCEPT `index.lock`. This is safe to call while
/// holding the flock because the lock file's inode is preserved, keeping the
/// advisory lock valid.
fn remove_workspace_index_contents(workspace: &Workspace) -> Result<()> {
    if !workspace.index_dir.exists() {
        return Ok(());
    }
    let lock_name = std::ffi::OsStr::new("index.lock");
    for entry in fs::read_dir(&workspace.index_dir)? {
        let entry = entry?;
        if entry.file_name() == lock_name {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn open_storage(workspace: &Workspace, embedding_dimensions: usize) -> Result<StorageHandles> {
    workspace.ensure_dirs()?;
    fs::create_dir_all(workspace.tantivy_dir())?;

    let sqlite_path = workspace.sqlite_path();
    let conn = Connection::open(&sqlite_path)?;
    create_tables(&conn)?;
    drop(conn);

    let tantivy_dir = workspace.tantivy_dir();
    let _ = open_tantivy_index(&tantivy_dir)?;

    let vector_path = workspace.vector_path();
    ensure_hash_vector_store(&vector_path, embedding_dimensions)?;

    Ok(StorageHandles {
        sqlite_path,
        tantivy_dir,
        vector_path,
    })
}

fn ensure_hash_vector_store(path: &Path, embedding_dimensions: usize) -> Result<()> {
    if path.exists() {
        let _ = VectorStore::open_readonly(path, embedding_dimensions, ScalarKind::F16)?;
    } else {
        VectorStore::open(path, embedding_dimensions, ScalarKind::F16)?.save()?;
    }
    Ok(())
}

pub fn index_workspace(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<IndexingSummary> {
    index_workspace_with_options(workspace, embedding_model, true, None, false)
}

pub fn index_workspace_for_watcher(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<IndexingSummary> {
    index_workspace_with_options(workspace, embedding_model, false, None, false)
}

pub fn index_workspace_paths_for_watcher(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
    changed_paths: &[PathBuf],
) -> Result<IndexingSummary> {
    index_workspace_with_options(
        workspace,
        embedding_model,
        false,
        Some(changed_paths),
        false,
    )
}

/// Rebuild a worktree overlay when its referenced base generation moved.
///
/// Searches call this before opening stores so stale tombstones and shadow
/// sets can never expose base-only files that are absent from the worktree.
pub fn reconcile_worktree_overlay(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<bool> {
    let reset_overlay = match workspace.worktree_overlay_is_stale() {
        Ok(false) => return Ok(false),
        Ok(true) => true,
        Err(err) if workspace.is_worktree() => {
            tracing::warn!(
                "invalid worktree overlay reference for {}: {err:#}; rebuilding",
                workspace.root.display()
            );
            true
        }
        Err(err) => return Err(err),
    };

    index_workspace_with_options(workspace, embedding_model, false, None, reset_overlay)?;
    if workspace.worktree_overlay_is_stale()? {
        anyhow::bail!(
            "worktree overlay remained stale after reconciliation: {}",
            workspace.root.display()
        );
    }
    Ok(true)
}

fn clear_worktree_overlay_storage(workspace: &Workspace) {
    let _ = fs::remove_file(workspace.overlay_sqlite_path());
    let _ = fs::remove_dir_all(workspace.overlay_tantivy_dir());
    let _ = fs::remove_file(workspace.overlay_vector_path());
    let _ = fs::remove_file(workspace.base_ref_path());
    let _ = fs::remove_file(workspace.merkle_snapshot_path());
}

fn index_workspace_with_options(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
    trust_live_watcher: bool,
    watcher_paths: Option<&[PathBuf]>,
    reset_worktree_overlay: bool,
) -> Result<IndexingSummary> {
    workspace.ensure_dirs()?;

    // On Linux, refuse to start indexing if available memory is critically low.
    // This prevents the OOM killer from randomly stopping the computer.
    #[cfg(target_os = "linux")]
    check_linux_memory_before_index()?;

    // Acquire an exclusive file lock to prevent concurrent writes to the
    // vector store (usearch) and other index files. The lock is advisory
    // and automatically released when `_lock_file` is dropped.
    //
    // IMPORTANT: The health check and rebuild MUST happen AFTER acquiring
    // this lock. Doing them before would destroy the lock file inode,
    // breaking flock mutual exclusion for any concurrent holder.
    let lock_path = workspace.lock_path();
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;
    fs2::FileExt::lock_exclusive(&lock_file)
        .with_context(|| format!("failed to acquire index lock {}", lock_path.display()))?;

    // Now that we truly own the workspace via flock, it's safe to inspect
    // health and rebuild if needed. rebuild_index_storage preserves the
    // lock file so our flock remains valid.
    let preserved_metadata = workspace.read_metadata().ok().flatten();
    if workspace.quick_index_health().needs_rebuild() {
        rebuild_index_storage(workspace, preserved_metadata.as_ref())?;
    }
    if reset_worktree_overlay {
        clear_worktree_overlay_storage(workspace);
    }

    let pid_path = workspace.indexing_pid_path();
    let _ = fs::write(&pid_path, std::process::id().to_string());

    struct IndexingGuard {
        pid_path: std::path::PathBuf,
        progress_path: std::path::PathBuf,
    }
    impl Drop for IndexingGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.pid_path);
            let _ = std::fs::remove_file(&self.progress_path);
        }
    }
    let _guard = IndexingGuard {
        pid_path: pid_path.clone(),
        progress_path: workspace.indexing_progress_path(),
    };

    let _ = jobs::start_job(workspace, JobKind::Indexing, "starting", 1);
    let stop_heartbeat = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let heartbeat_stop = stop_heartbeat.clone();
    let heartbeat_workspace = workspace.clone();
    std::thread::spawn(move || {
        while !heartbeat_stop.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if heartbeat_stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let progress = std::fs::read_to_string(heartbeat_workspace.indexing_progress_path())
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let mut update = JobUpdate {
                phase: Some(progress.clone().unwrap_or_else(|| "running".to_string())),
                ..Default::default()
            };
            if let Some(progress) = progress {
                update.details.insert("progress".to_string(), progress);
            }
            let _ = jobs::heartbeat_job(&heartbeat_workspace, JobKind::Indexing, update);
        }
    });

    let tracks_reusable_base_state =
        workspace.repo_id.is_some() && workspace.base_index_dir.is_none();
    let clean_git_state_before = tracks_reusable_base_state
        .then(|| clean_git_checkout_state(&workspace.root))
        .flatten();
    let result = retry_transient_tantivy_writes(|| {
        index_workspace_inner(
            workspace,
            embedding_model,
            trust_live_watcher,
            watcher_paths,
        )
    });
    if result.is_ok() && tracks_reusable_base_state {
        record_indexed_git_state(workspace, clean_git_state_before.as_deref());
    }

    // Run a checkpoint to reclaim WAL space after bulk writes, then
    // truncate the WAL file so it doesn't keep consuming disk.
    let sqlite_path = if workspace.has_overlay() || workspace.base_ref_path().exists() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    if sqlite_path.exists()
        && let Ok(conn) = Connection::open(sqlite_path)
    {
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    let _ = fs2::FileExt::unlock(&lock_file);
    stop_heartbeat.store(true, std::sync::atomic::Ordering::Relaxed);
    match &result {
        Ok(_) => {
            let _ = jobs::finish_job(workspace, JobKind::Indexing, "completed", None);
        }
        Err(err) => {
            let _ = jobs::finish_job(
                workspace,
                JobKind::Indexing,
                "failed",
                Some(format!("{err:#}")),
            );
        }
    }
    result
}

fn retry_transient_tantivy_writes<T>(mut operation: impl FnMut() -> Result<T>) -> Result<T> {
    for attempt in 0..TANTIVY_INDEX_RETRY_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error)
                if is_transient_tantivy_write_error(&error)
                    && attempt + 1 < TANTIVY_INDEX_RETRY_ATTEMPTS =>
            {
                let delay_ms = TANTIVY_INDEX_RETRY_BASE_DELAY_MS << attempt;
                tracing::warn!(
                    attempt = attempt + 1,
                    delay_ms,
                    "retrying index after transient Tantivy write denial"
                );
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the retry loop always returns on its final attempt")
}

fn is_transient_tantivy_write_error(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        matches!(
            source.downcast_ref::<tantivy::TantivyError>(),
            Some(
                tantivy::TantivyError::OpenWriteError(OpenWriteError::IoError {
                    io_error,
                    ..
                })
            ) | Some(tantivy::TantivyError::IoError(io_error))
                if io_error.kind() == std::io::ErrorKind::PermissionDenied
        )
    })
}

fn index_workspace_inner(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
    trust_live_watcher: bool,
    watcher_paths: Option<&[PathBuf]>,
) -> Result<IndexingSummary> {
    let index_started = std::time::Instant::now();

    // Write metadata early so the workspace appears in `ig --status` during indexing.
    // The final write after completion updates last_indexed_at_unix.
    if workspace.read_metadata()?.is_none() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        workspace.write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: now,
            last_indexed_at_unix: None,
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 0,
        })?;
    }

    // Trust-but-verify: if a live watcher daemon is confirmed, skip the
    // expensive Merkle rebuild entirely. The watcher already triggered
    // re-indexing for any changed files through filesystem events.
    if trust_live_watcher && workspace.is_watcher_alive() && workspace_is_indexed(workspace) {
        return Ok(IndexingSummary {
            workspace_id: workspace.id.clone(),
            indexed_files: 0,
            deleted_files: 0,
            total_chunks: count_chunks(&workspace.sqlite_path())?,
            phase_timings: IndexingPhaseTimings {
                discovery_ms: index_started.elapsed().as_secs_f64() * 1_000.0,
                ..Default::default()
            },
        });
    }

    let skip_gitignore = workspace.read_metadata()?.is_some_and(|m| m.skip_gitignore);

    // ── Worktree overlay ─────────────────────────────────────────────────
    // If this is a git worktree and the base has a fresh index, create a
    // thin overlay containing only divergent files instead of copying the
    // entire base. The base index is referenced by path, not copied.
    let overlay_mode = if let Some(ref base_dir) = workspace.base_index_dir {
        let base_sqlite = base_dir.join("metadata.sqlite3");
        let base_merkle = base_dir.join("merkle_snapshot.json");
        let mut base_refreshed = false;

        // Ignored files shared with a worktree belong in the base index. Keep
        // the base as a superset and let query options filter ignored chunks;
        // otherwise an unchanged ignored file cannot be inherited by a
        // worktree indexed with --skip-gitignore.
        if skip_gitignore && let Some(main_root) = workspace.main_worktree_root() {
            let base_ws = crate::workspace::Workspace::resolve(&main_root)?;
            let mut base_meta = base_ws
                .read_metadata()?
                .unwrap_or_else(|| WorkspaceMetadata {
                    id: base_ws.id.clone(),
                    root: base_ws.root.clone(),
                    created_at_unix: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    last_indexed_at_unix: None,
                    watch_enabled: false,
                    skip_gitignore: false,
                    index_generation: 0,
                });
            if !base_meta.skip_gitignore {
                base_meta.skip_gitignore = true;
                base_ws.ensure_dirs()?;
                base_ws.write_metadata(&base_meta)?;
                if base_sqlite.exists() {
                    eprintln!("  ⚡ enabling ignored files in base index before overlay...");
                    if let Err(err) = index_workspace_for_watcher(&base_ws, embedding_model) {
                        base_meta.skip_gitignore = false;
                        let _ = base_ws.write_metadata(&base_meta);
                        return Err(err);
                    }
                    base_refreshed = true;
                    if workspace.has_overlay() {
                        clear_worktree_overlay_storage(workspace);
                        return index_workspace_inner(
                            workspace,
                            embedding_model,
                            trust_live_watcher,
                            None,
                        );
                    }
                }
            }
        }

        // If the base index uses a different on-disk format, rebuild it before
        // referencing it. An overlay serves chunks and vectors from the base,
        // so querying any incompatible layout is unsafe. The base self-heals
        // via its own health check during index_workspace.
        let base_format = std::fs::read_to_string(base_dir.join("index_format_version"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        if base_sqlite.exists()
            && base_format != crate::workspace::INDEX_FORMAT_VERSION
            && let Some(main_root) = workspace.main_worktree_root()
            && let Ok(base_ws) = crate::workspace::Workspace::resolve(&main_root)
        {
            eprintln!("  ⚡ base index format incompatible — rebuilding base before overlay...");
            let _ = index_workspace(&base_ws, embedding_model)?;
            base_refreshed = true;
            if workspace.has_overlay() {
                // Existing overlay references the now-migrated base; rebuild it.
                clear_worktree_overlay_storage(workspace);
                return index_workspace_inner(workspace, embedding_model, trust_live_watcher, None);
            }
        }

        if (!base_sqlite.exists() || !base_merkle.exists())
            && !workspace.has_overlay()
            && let Some(main_root) = workspace.main_worktree_root()
        {
            eprintln!("  ⚡ base workspace is not indexed, running full base indexing first...");
            let base_workspace = crate::workspace::Workspace::resolve(&main_root)?;
            // We recursively call index_workspace on the base. It will acquire its
            // own safe lock and index natively.
            let _ = index_workspace(&base_workspace, embedding_model)?;
            base_refreshed = true;
            eprintln!("  ⚡ base indexing complete, proceeding with overlay...");
        }

        if base_sqlite.exists() && base_merkle.exists() && !workspace.has_overlay() {
            eprintln!("  ⚡ creating worktree overlay (no copy)...");
            let _ = fs::write(workspace.indexing_progress_path(), "building overlay");

            // Record base reference, including the base's current generation
            // so we can detect staleness on subsequent indexing runs.
            let main_root = workspace
                .main_worktree_root()
                .context("cannot find main worktree root")?;
            let base_ws = crate::workspace::Workspace::resolve(&main_root)?;
            // The overlay may inherit base paths only after the base index
            // reflects its current files. This is incremental and avoids
            // silently inheriting stale chunks from an unindexed base edit.
            if !base_refreshed && !refresh_clean_base_metadata(&base_ws)? {
                let _ = index_workspace_for_watcher(&base_ws, embedding_model)?;
            }
            let base_generation = base_ws
                .read_metadata()?
                .map(|m| m.index_generation)
                .unwrap_or(0);
            let base_ref = serde_json::json!({
                "base_index_dir": base_dir.to_string_lossy(),
                "base_workspace_root": main_root.to_string_lossy(),
                "base_generation": base_generation,
                "created_at_unix": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
            fs::write(
                workspace.base_ref_path(),
                serde_json::to_vec_pretty(&base_ref)?,
            )?;

            let _ = fs::write(
                workspace.indexing_progress_path(),
                "scanning (content-based)",
            );
            let old = MerkleSnapshot::build_content_based(&main_root, skip_gitignore)?;
            let new = MerkleSnapshot::build_content_based(&workspace.root, skip_gitignore)?;
            let diff = old.diff(&new);

            eprintln!(
                "  ⚡ overlay delta: {} added/modified, {} deleted",
                diff.added_or_modified.len(),
                diff.deleted.len()
            );

            // Save an mtime-based snapshot for this worktree so that future
            // incremental diffs (which use MerkleSnapshot::build / mtime mode)
            // produce correct deltas. The content-based snapshots above were
            // only needed for the initial cross-worktree diff; persisting them
            // would cause every file's hash to differ on the next watcher tick.
            let mtime_snapshot = MerkleSnapshot::build(&workspace.root, skip_gitignore)?;
            mtime_snapshot.save(&workspace.merkle_snapshot_path())?;

            Some(diff)
        } else if workspace.has_overlay() {
            // Overlay exists — check if the base index has been updated since
            // this overlay was created. If so, the tombstone/shadow sets are
            // stale and will produce wrong search results. Force a rebuild.
            let stale = (|| -> Option<bool> {
                let ref_data = fs::read(workspace.base_ref_path()).ok()?;
                let ref_json: serde_json::Value = serde_json::from_slice(&ref_data).ok()?;
                let overlay_gen = ref_json.get("base_generation")?.as_u64()?;
                let main_root = workspace.main_worktree_root()?;
                let base_ws = crate::workspace::Workspace::resolve(&main_root).ok()?;
                let current_gen = base_ws.read_metadata().ok()??.index_generation;
                Some(current_gen != overlay_gen)
            })();
            if stale == Some(true) {
                eprintln!(
                    "  ⚠ base index has changed since overlay was created — rebuilding overlay..."
                );
                clear_worktree_overlay_storage(workspace);
                // Re-enter this function to take the fresh overlay creation path
                return index_workspace_inner(workspace, embedding_model, trust_live_watcher, None);
            }
            None
        } else {
            // Base doesn't exist yet — fall through to full index
            None
        }
    } else {
        None
    };

    let overlay_base_snapshot = if (workspace.has_overlay() || workspace.base_ref_path().exists())
        && let Some(base_dir) = &workspace.base_index_dir
    {
        Some(MerkleSnapshot::load(
            &base_dir.join("merkle_snapshot.json"),
        )?)
    } else {
        None
    };
    let base_ignored_status = |rel_path: &Path| {
        overlay_base_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .files
                .get(&index_path_string(rel_path))
                .map(|hash| hash.ends_with("-1"))
        })
    };
    let path_exists_in_base = |rel_path: &Path| base_ignored_status(rel_path).is_some();
    // When not in overlay creation mode, use the standard Merkle diff path.
    // IMPORTANT: The snapshot is NOT saved here — it is deferred to after all
    // store commits complete. Saving it earlier creates a crash window where
    // the snapshot claims files are indexed but the actual stores are empty/partial.
    // See: snapshot must be a high-water mark of persisted state, not of intent.
    let (diff, pending_snapshot, clear_overlay_paths) = if let Some(overlay_diff) = overlay_mode {
        (overlay_diff, None, Vec::new())
    } else if workspace.has_overlay() {
        // Incremental update to existing overlay
        let old = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
        let _ = fs::write(workspace.indexing_progress_path(), "scanning");
        let new = MerkleSnapshot::build(&workspace.root, skip_gitignore)?;
        let mut d = old.diff(&new);
        let mut clear_overlay_paths = Vec::new();

        // Keep the overlay relative to the base after branch switches or local
        // restores. Reappearing base-identical paths should delegate to the
        // base index, while removed overlay-only paths need no tombstone.
        if let Some(main_root) = workspace.main_worktree_root() {
            let mut divergent = Vec::with_capacity(d.added_or_modified.len());
            for (rel_path, is_ignored) in d.added_or_modified {
                let base_snapshot_is_current = overlay_base_snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.files.get(&index_path_string(&rel_path)))
                    .is_some_and(|hash| {
                        MerkleSnapshot::path_matches_metadata_snapshot(
                            &main_root.join(&rel_path),
                            hash,
                        )
                    });
                let returns_to_base = base_snapshot_is_current
                    && base_ignored_status(&rel_path) == Some(is_ignored)
                    && files_have_same_contents(
                        &workspace.root.join(&rel_path),
                        &main_root.join(&rel_path),
                    );
                if returns_to_base {
                    clear_overlay_paths.push(rel_path);
                } else {
                    divergent.push((rel_path, is_ignored));
                }
            }
            d.added_or_modified = divergent;

            let mut base_deletions = Vec::with_capacity(d.deleted.len());
            for rel_path in d.deleted {
                if path_exists_in_base(&rel_path) {
                    base_deletions.push(rel_path);
                } else {
                    clear_overlay_paths.push(rel_path);
                }
            }
            d.deleted = base_deletions;
        }
        // True no-op: with no worktree changes, return without rewriting the
        // overlay stores. Rewriting them on every reindex/watcher tick also
        // busts the daemon's SearchContext/query caches via file-stamp changes.
        if d.added_or_modified.is_empty()
            && d.deleted.is_empty()
            && clear_overlay_paths.is_empty()
            && workspace_is_indexed(workspace)
        {
            return Ok(IndexingSummary {
                workspace_id: workspace.id.clone(),
                indexed_files: 0,
                deleted_files: 0,
                total_chunks: count_workspace_chunks(workspace).unwrap_or(0),
                phase_timings: IndexingPhaseTimings {
                    discovery_ms: index_started.elapsed().as_secs_f64() * 1_000.0,
                    ..Default::default()
                },
            });
        }
        (d, Some(new), clear_overlay_paths)
    } else {
        let old = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
        // Watcher events already identify changed files. Apply those directly
        // when safe, keeping full scans as reconciliation fallback for ignore
        // edits, directories, overlays, and uncertain state.
        let targeted = if let Some(paths) = watcher_paths.filter(|paths| !paths.is_empty())
            && workspace.merkle_snapshot_path().exists()
        {
            old.refresh_paths(&workspace.root, paths, skip_gitignore)?
        } else {
            None
        };
        let (new, d) = if let Some((new, diff)) = targeted {
            let _ = fs::write(workspace.indexing_progress_path(), "applying watcher delta");
            (new, diff)
        } else {
            let _ = fs::write(workspace.indexing_progress_path(), "scanning");
            let new = MerkleSnapshot::build(&workspace.root, skip_gitignore)?;
            let diff = old.diff(&new);
            (new, diff)
        };
        if d.added_or_modified.is_empty() && d.deleted.is_empty() && workspace_is_indexed(workspace)
        {
            return Ok(IndexingSummary {
                workspace_id: workspace.id.clone(),
                indexed_files: 0,
                deleted_files: 0,
                total_chunks: count_workspace_chunks(workspace).unwrap_or(0),
                phase_timings: IndexingPhaseTimings {
                    discovery_ms: index_started.elapsed().as_secs_f64() * 1_000.0,
                    ..Default::default()
                },
            });
        }
        (d, Some(new), Vec::new())
    };

    let discovery_ms = index_started.elapsed().as_secs_f64() * 1_000.0;
    let persist_started = std::time::Instant::now();

    // Determine which stores to write to: overlay or main
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    let (sqlite_path, tantivy_path, vector_path) = if use_overlay {
        (
            workspace.overlay_sqlite_path(),
            workspace.overlay_tantivy_dir(),
            workspace.overlay_vector_path(),
        )
    } else {
        (
            workspace.sqlite_path(),
            workspace.tantivy_dir(),
            workspace.vector_path(),
        )
    };

    if !use_overlay {
        let preserved_metadata = workspace.read_metadata().ok().flatten();
        if let Err(err) = open_storage(workspace, crate::EMBEDDING_DIMENSIONS) {
            tracing::warn!(
                "storage verification failed for {}: {err:#}; rebuilding index storage",
                workspace.root.display()
            );
            rebuild_index_storage(workspace, preserved_metadata.as_ref())?;
            let _ = open_storage(workspace, crate::EMBEDDING_DIMENSIONS).with_context(|| {
                format!(
                    "failed to reopen index storage after rebuild for {}",
                    workspace.root.display()
                )
            })?;
        }
    }

    let mut sqlite = Connection::open(&sqlite_path)?;
    // WAL mode + larger cache for bulk-write throughput on initial index.
    sqlite.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -16000;
         PRAGMA temp_store = MEMORY;",
    )?;
    create_tables(&sqlite)?;
    if use_overlay {
        create_overlay_tables(&sqlite)?;
    }

    fs::create_dir_all(&tantivy_path)?;
    // Clear stale Tantivy writer lock left by a crash — safe because we
    // already hold the fs2 advisory lock guaranteeing exclusive access.
    let tantivy_lock = tantivy_path.join(".tantivy-writer.lock");
    let _ = fs::remove_file(&tantivy_lock);
    let (tantivy, fields) = open_tantivy_index(&tantivy_path)?;
    // Retry with backoff — NFS/overlayfs may delay flock release.
    let mut writer = None;
    for attempt in 0..5u32 {
        match tantivy.writer(50_000_000) {
            Ok(w) => {
                writer = Some(w);
                break;
            }
            Err(err) => {
                if attempt < 4 {
                    let _ = fs::remove_file(&tantivy_lock);
                    std::thread::sleep(std::time::Duration::from_millis(
                        200 * (attempt as u64 + 1),
                    ));
                } else {
                    return Err(err.into());
                }
            }
        }
    }
    let mut writer = writer.context("writer must be acquired after retries")?;

    ensure_hash_vector_store(&vector_path, crate::EMBEDDING_DIMENSIONS)?;
    let hash_tombstones_path = workspace.hash_tombstones_path();
    let neural_tombstones_path = (!use_overlay).then(|| workspace.neural_tombstones_path());

    // Batch SQLite writes in a transaction for ~10-50x speedup.
    // Mutable so we can periodically commit and avert massive WAL files.
    let mut tx = sqlite.transaction()?;

    // Overlay state shadows only paths backed by the base index. Clear paths
    // that have returned to base content or were removed after being overlay-only.
    if use_overlay {
        for rel_path in &clear_overlay_paths {
            remove_file_chunks(
                &tx,
                &mut writer,
                &fields,
                &hash_tombstones_path,
                neural_tombstones_path.as_deref(),
                rel_path,
            )?;
            tx.execute(
                "DELETE FROM tombstones WHERE file_path = ?1",
                params![index_path_string(rel_path)],
            )?;
        }

        for rel_path in &diff.deleted {
            let rel_str = index_path_string(rel_path);
            remove_file_chunks(
                &tx,
                &mut writer,
                &fields,
                &hash_tombstones_path,
                neural_tombstones_path.as_deref(),
                rel_path,
            )?;
            if path_exists_in_base(rel_path) {
                tx.execute(
                    "INSERT OR IGNORE INTO tombstones (file_path) VALUES (?1)",
                    params![rel_str],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM tombstones WHERE file_path = ?1",
                    params![rel_str],
                )?;
            }
        }

        // Insert before chunking so a base file replaced by empty/binary
        // content is still hidden even though it produces no overlay chunks.
        for (rel_path, _) in &diff.added_or_modified {
            if path_exists_in_base(rel_path) {
                tx.execute(
                    "INSERT OR IGNORE INTO tombstones (file_path) VALUES (?1)",
                    params![index_path_string(rel_path)],
                )?;
            }
        }
    } else {
        apply_deletions(
            &tx,
            &mut writer,
            &fields,
            &hash_tombstones_path,
            neural_tombstones_path.as_deref(),
            &diff.deleted,
        )?;
    }

    let total = diff.added_or_modified.len();
    let show_progress = total > 0 && std::io::stderr().is_terminal();
    let progress_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let t0 = std::time::Instant::now();
    let mut total_chunks_processed = 0;
    let mut touched_files = HashSet::new();
    let mut chunks_since_commit = 0;

    // On a fresh (empty) index, skip per-file remove_file_chunks entirely —
    // there's nothing to delete, and the SELECT + DELETE per file is pure overhead
    // on large initial indexes (~93K files in linux kernel).
    let is_fresh_index = !workspace_is_indexed(workspace);

    // Stream through batches to rigidly bound memory footprints.
    // 4096 files is highly parallelizable while capping memory overhead effectively.
    let (tx_batch, rx_batch) = std::sync::mpsc::sync_channel::<IndexedFileBatch>(2);

    let progress_counter_clone = progress_counter.clone();
    let root_clone = workspace.root.clone();
    let progress_path_clone = workspace.indexing_progress_path();
    let diff_paths: Vec<_> = diff.added_or_modified.clone();

    let _ = fs::write(&progress_path_clone, format!("0/{total}"));

    let producer_handle = std::thread::spawn(move || {
        for batch_paths in diff_paths.chunks(128) {
            let file_chunks: Vec<_> = batch_paths
                .par_iter()
                .filter_map(|(rel_path, is_ignored)| {
                    // For a modified file that now yields no chunks (vanished,
                    // unreadable, empty, binary/non-text, or chunks to nothing)
                    // emit an empty entry on an incremental index so the
                    // consumer still runs remove_file_chunks and clears the
                    // stale chunks + orphaned vectors. On a fresh index there is
                    // nothing to remove, so skip the file entirely.
                    let nothing = |rel: &std::path::Path| {
                        if is_fresh_index {
                            None
                        } else {
                            Some((rel.to_path_buf(), Vec::new()))
                        }
                    };

                    let abs_path = root_clone.join(rel_path);
                    if !abs_path.exists() {
                        progress_counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return nothing(rel_path);
                    }

                    let content_bytes = match fs::read(&abs_path) {
                        Ok(b) => b,
                        Err(_) => return nothing(rel_path),
                    };
                    if !is_indexable_file(rel_path, &content_bytes) {
                        progress_counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return nothing(rel_path);
                    }

                    let content = match String::from_utf8(content_bytes) {
                        Ok(text) => text,
                        Err(err) => String::from_utf8_lossy(&err.into_bytes()).into_owned(),
                    };

                    let chunks = chunk_source(rel_path, &content);
                    let mut seen_vector_keys = HashSet::new();
                    let indexed: Vec<_> = chunks
                        .into_iter()
                        .map(|c| build_indexed_chunk(c, *is_ignored))
                        .filter(|chunk| seen_vector_keys.insert(chunk.vector_key))
                        .map(prepare_indexed_chunk)
                        .collect();

                    let n = progress_counter_clone
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        + 1;
                    if show_progress && n.is_multiple_of(500) {
                        eprint!("\r\x1b[K  ⠋ indexing {n}/{total} files...");
                    }
                    if n.is_multiple_of(2000) {
                        let _ = fs::write(&progress_path_clone, format!("{n}/{total}"));
                    }

                    if indexed.is_empty() {
                        return nothing(rel_path);
                    }
                    Some((rel_path.clone(), indexed))
                })
                .collect();

            if !file_chunks.is_empty() && tx_batch.send(file_chunks).is_err() {
                break;
            }
        }
    });
    let mut producer = IndexBatchProducer::new(rx_batch, producer_handle);

    macro_rules! persist_or_stop {
        ($result:expr) => {
            match $result {
                Ok(value) => value,
                Err(err) => {
                    // Dropping the receiver is the producer cancellation signal.
                    // Join before returning so a failed ingest cannot leave a
                    // scanner running in the background.
                    producer.stop();
                    return Err(err.into());
                }
            }
        };
    }

    while let Some(file_chunks) = producer.recv() {
        // Persist lexical metadata first. Hash ANN construction is intentionally
        // deferred to background enhancement: on multi-million chunk repos the
        // provisional graph dominated first-index latency and delayed usable
        // BM25/literal results by minutes.
        for (rel_path, indexed_chunks) in &file_chunks {
            let rel_path_string = index_path_string(rel_path);
            touched_files.insert(rel_path_string.clone());
            total_chunks_processed += indexed_chunks.len();
            chunks_since_commit += indexed_chunks.len();

            if !is_fresh_index {
                persist_or_stop!(remove_file_chunks(
                    &tx,
                    &mut writer,
                    &fields,
                    &hash_tombstones_path,
                    neural_tombstones_path.as_deref(),
                    rel_path,
                ));
            }

            // Batch the timestamp syscall per file, not per chunk.
            let now_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            for prepared in indexed_chunks {
                let indexed = &prepared.chunk;
                persist_or_stop!(insert_chunk(
                    &tx,
                    indexed,
                    &prepared.compressed_text,
                    &rel_path_string,
                    now_unix
                ));
                persist_or_stop!(add_chunk_doc(
                    &mut writer,
                    &fields,
                    indexed,
                    &rel_path_string
                ));
            }
        }

        // Bound SQLite WAL growth independently from Tantivy publication.
        // Fresh indexes publish Tantivy once at the end: committing every
        // SQLite batch forces repeated segment merges and multiplies disk I/O.
        if chunks_since_commit >= 25_000 {
            persist_or_stop!(tx.commit());
            if !is_fresh_index {
                persist_or_stop!(writer.commit());
            }
            tx = persist_or_stop!(sqlite.transaction());
            chunks_since_commit = 0;
        }
    }

    producer.finish()?;

    let t1 = std::time::Instant::now();
    let persist_ms = persist_started.elapsed().as_secs_f64() * 1_000.0;
    let finalize_started = std::time::Instant::now();
    if show_progress && total > 0 {
        eprint!(
            "\r\x1b[K  ✓ {} files, {} chunks — indexed completely in {:.1}s\n",
            touched_files.len(),
            total_chunks_processed,
            t1.duration_since(t0).as_secs_f64()
        );
    }

    finalize_graph_indexes(&tx)?;

    // Update cached stats before committing so status reads are O(1).
    let chunk_count: i64 = tx
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .unwrap_or(0);
    let file_count: i64 = tx
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    let vector_key_count: i64 = tx
        .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    tx.execute(
        "INSERT OR REPLACE INTO _stats (key, value) VALUES ('chunk_count', ?1)",
        params![chunk_count],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO _stats (key, value) VALUES ('file_count', ?1)",
        params![file_count],
    )?;
    tx.execute(
        "INSERT OR REPLACE INTO _stats (key, value) VALUES ('vector_key_count', ?1)",
        params![vector_key_count],
    )?;

    tx.commit()?;

    writer.commit()?;
    writer.wait_merging_threads()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let existing_meta = workspace
        .read_metadata()?
        .unwrap_or_else(|| WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: now,
            last_indexed_at_unix: None,
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 0,
        });
    let metadata = WorkspaceMetadata {
        id: workspace.id.clone(),
        root: workspace.root.clone(),
        created_at_unix: existing_meta.created_at_unix,
        last_indexed_at_unix: Some(now),
        watch_enabled: existing_meta.watch_enabled,
        skip_gitignore: existing_meta.skip_gitignore,
        // Tracks lexical commits so background vector enhancement can detect
        // concurrent edits and resume from the latest generation.
        index_generation: existing_meta.index_generation + 1,
    };
    workspace.write_metadata(&metadata)?;
    // Mark the index as written in the current on-disk format so an upgrade
    // that changes the layout forces a rebuild (see INDEX_FORMAT_VERSION).
    workspace.write_index_format_version()?;

    // Persist the Merkle snapshot AFTER all stores are committed and metadata
    // is written. This ensures the snapshot is a high-water mark: if we crash
    // before this point, the next run will see a non-empty diff and re-index
    // the affected files. `remove_file_chunks` cleans any partial state.
    if let Some(snapshot) = pending_snapshot {
        snapshot.save(&workspace.merkle_snapshot_path())?;
    }

    Ok(IndexingSummary {
        workspace_id: workspace.id.clone(),
        indexed_files: touched_files.len(),
        deleted_files: diff.deleted.len(),
        total_chunks: count_workspace_chunks(workspace).unwrap_or(0),
        phase_timings: IndexingPhaseTimings {
            discovery_ms,
            persist_ms,
            finalize_ms: finalize_started.elapsed().as_secs_f64() * 1_000.0,
        },
    })
}

fn files_have_same_contents(left: &Path, right: &Path) -> bool {
    match (fs::read(left), fs::read(right)) {
        (Ok(left_bytes), Ok(right_bytes)) => {
            left_bytes == right_bytes
                || normalized_indexable_content(left, &left_bytes)
                    == normalized_indexable_content(right, &right_bytes)
        }
        _ => false,
    }
}

fn git_head(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|head| !head.is_empty())
}

fn git_index_hash(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "index"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let bytes = fs::read(path).ok()?;
    Some(hex::encode(
        xxhash_rust::xxh3::xxh3_128(&bytes).to_le_bytes(),
    ))
}

fn git_worktree_is_clean(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.is_empty())
}

fn git_checkout_state(root: &Path) -> Option<String> {
    Some(format!(
        "{}\n{}\n{}",
        git_head(root)?,
        git_index_hash(root)?,
        git_sparse_checkout_state(root)
    ))
}

fn clean_git_checkout_state(root: &Path) -> Option<String> {
    git_worktree_is_clean(root).then(|| git_checkout_state(root))?
}

fn git_sparse_checkout_state(root: &Path) -> String {
    let list = std::process::Command::new("git")
        .args(["sparse-checkout", "list"])
        .current_dir(root)
        .output();
    let Ok(list) = list else {
        return "disabled".to_string();
    };
    if !list.status.success() {
        return "disabled".to_string();
    }

    let cone = std::process::Command::new("git")
        .args(["config", "--bool", "core.sparseCheckoutCone"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
        .unwrap_or_default();
    let mut state = list.stdout;
    state.extend_from_slice(&cone);
    format!(
        "enabled:{}",
        hex::encode(xxhash_rust::xxh3::xxh3_128(&state).to_le_bytes())
    )
}

fn indexed_git_state_path(workspace: &Workspace) -> PathBuf {
    workspace.index_dir.join("indexed_git_state")
}

fn record_indexed_git_state(workspace: &Workspace, expected_state: Option<&str>) -> bool {
    let current_state = clean_git_checkout_state(&workspace.root);
    if current_state.as_deref() == expected_state
        && let Some(state) = current_state
        && fs::write(indexed_git_state_path(workspace), state).is_ok()
    {
        return true;
    }
    let _ = fs::remove_file(indexed_git_state_path(workspace));
    false
}

enum BaseIndexCheckoutState {
    Current,
    MetadataChanged,
    Stale,
}

fn base_index_checkout_state(workspace: &Workspace) -> BaseIndexCheckoutState {
    let indexes_ignored_files = workspace
        .read_metadata()
        .ok()
        .flatten()
        .is_some_and(|metadata| metadata.skip_gitignore);
    if indexes_ignored_files
        || !workspace.quick_index_health().is_queryable()
        || !git_worktree_is_clean(&workspace.root)
    {
        return BaseIndexCheckoutState::Stale;
    }
    let Some(current_state) = git_checkout_state(&workspace.root) else {
        return BaseIndexCheckoutState::Stale;
    };
    let Some(indexed_state) = fs::read_to_string(indexed_git_state_path(workspace)).ok() else {
        return BaseIndexCheckoutState::Stale;
    };
    if indexed_state == current_state {
        return BaseIndexCheckoutState::Current;
    }

    let same_head = indexed_state.lines().next() == current_state.lines().next();
    let same_sparse_checkout = indexed_state.lines().nth(2) == current_state.lines().nth(2);
    if same_head && same_sparse_checkout {
        BaseIndexCheckoutState::MetadataChanged
    } else {
        BaseIndexCheckoutState::Stale
    }
}

fn refresh_clean_base_metadata(workspace: &Workspace) -> Result<bool> {
    match base_index_checkout_state(workspace) {
        BaseIndexCheckoutState::Current => return Ok(true),
        BaseIndexCheckoutState::Stale => return Ok(false),
        BaseIndexCheckoutState::MetadataChanged => {}
    }

    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.lock_path())?;
    fs2::FileExt::lock_exclusive(&lock_file)?;

    match base_index_checkout_state(workspace) {
        BaseIndexCheckoutState::Current => Ok(true),
        BaseIndexCheckoutState::Stale => Ok(false),
        BaseIndexCheckoutState::MetadataChanged => {
            let skip_gitignore = workspace
                .read_metadata()?
                .is_some_and(|metadata| metadata.skip_gitignore);
            let expected_state = clean_git_checkout_state(&workspace.root);
            MerkleSnapshot::build(&workspace.root, skip_gitignore)?
                .save(&workspace.merkle_snapshot_path())?;
            Ok(record_indexed_git_state(
                workspace,
                expected_state.as_deref(),
            ))
        }
    }
}

fn rebuild_index_storage(
    workspace: &Workspace,
    preserved_metadata: Option<&WorkspaceMetadata>,
) -> Result<()> {
    // Use lock-preserving removal so that any held flock remains valid.
    // The caller is expected to already hold the advisory lock.
    remove_workspace_index_contents(workspace)?;
    workspace.ensure_dirs()?;
    if let Some(mut metadata) = preserved_metadata.cloned() {
        metadata.last_indexed_at_unix = None;
        workspace.write_metadata(&metadata)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_pmset_batt(stdout: &str) -> Option<String> {
    if stdout.contains("Battery Power") {
        Some("Battery Power".to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn parse_pmset_therm(stdout: &str) -> Option<String> {
    if stdout.contains("warning level")
        && !stdout.contains("No thermal warning level")
        && !stdout.contains("No performance warning level")
    {
        Some("Thermal Throttling".to_string())
    } else {
        None
    }
}

/// Load-average multiple of the CPU count above which background neural
/// enhancement pauses. Configurable via `IVYGREP_ENHANCE_MAX_LOAD_RATIO`.
///
/// The enhancement subprocess is already `nice(10)` and capped at ~25% of
/// cores, so it yields to interactive work. The previous 0.75–0.8× threshold
/// paused it on routinely-busy machines (a dev box mid-build, a shared host),
/// so neural vectors were never built and search stayed on the lower-quality
/// hash path. Default 2.0× pauses only under genuine sustained oversubscription;
/// a value <= 0 disables the load check entirely.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn enhance_max_load_ratio() -> f64 {
    std::env::var("IVYGREP_ENHANCE_MAX_LOAD_RATIO")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(2.0)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_system_load(load1: f64, cpus: f64) -> Option<String> {
    let ratio = enhance_max_load_ratio();
    if ratio <= 0.0 {
        return None; // load check disabled
    }
    let max_load = cpus * ratio;
    if load1 > max_load {
        Some(format!("High System Load ({load1:.1} > {max_load:.1} max)"))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn check_system_constraints() -> Option<String> {
    // Never pause in test or CI environments to avoid breaking benchmarks randomly
    if cfg!(test) || std::env::var("CI").is_ok() {
        return None;
    }

    use std::process::Command;

    // 1. Check battery power
    if let Ok(output) = Command::new("pmset").arg("-g").arg("batt").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(reason) = parse_pmset_batt(&stdout) {
            return Some(reason);
        }
    }

    // 2. Check thermal limit
    if let Ok(output) = Command::new("pmset").arg("-g").arg("therm").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(reason) = parse_pmset_therm(&stdout) {
            return Some(reason);
        }
    }

    // 3. High load
    let mut loadavg = [0.0f64; 3];
    let has_load = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 3) };
    if has_load > 0 {
        let load1 = loadavg[0];
        let cpus = num_cpus::get() as f64;
        if let Some(reason) = parse_system_load(load1, cpus) {
            return Some(reason);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn check_system_constraints() -> Option<String> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return None;
    }

    let mut loadavg = [0.0f64; 3];
    let has_load = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 3) };
    if has_load > 0 {
        let load1 = loadavg[0];
        let cpus = num_cpus::get() as f64;
        if let Some(reason) = parse_system_load(load1, cpus) {
            return Some(reason);
        }
    }

    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo")
        && let Some(kb) = meminfo.lines().find_map(|line| {
            line.strip_prefix("MemAvailable:")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        })
        && kb < 1_048_576
    {
        return Some(format!("Low Available Memory ({} MiB)", kb / 1024));
    }

    None
}

/// Guard for the indexer: refuse to start indexing when available memory
/// is dangerously low. This prevents the OOM killer from firing during
/// heavy workloads on Linux machines with limited RAM.
#[cfg(target_os = "linux")]
fn check_linux_memory_before_index() -> Result<()> {
    if cfg!(test) || std::env::var("CI").is_ok() {
        return Ok(());
    }

    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo")
        && let Some(kb) = meminfo.lines().find_map(|line| {
            line.strip_prefix("MemAvailable:")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        })
        && kb < 524_288
    {
        anyhow::bail!(
            "refusing to index: only {} MiB of memory available (need at least 512 MiB). \
             Close other applications or free memory before re-indexing.",
            kb / 1024
        );
    }

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
fn check_system_constraints() -> Option<String> {
    None
}

/// Compute lightweight hash embeddings for all chunks and save as the first
/// background vector tier.
pub fn enhance_workspace_hash(
    workspace: &Workspace,
    hash_model: &dyn EmbeddingModel,
) -> Result<usize> {
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    let sqlite_path = if use_overlay {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    let vector_path = if use_overlay {
        workspace.overlay_vector_path()
    } else {
        workspace.vector_path()
    };
    if !sqlite_path.exists() {
        return Ok(0);
    }

    let index_generation = workspace
        .read_metadata()?
        .map(|metadata| metadata.index_generation)
        .unwrap_or(0);
    let sqlite = open_sqlite(&sqlite_path)?;
    let total_chunks = sqlite
        .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0) as usize;
    let mut vector_index = VectorStore::open(
        &vector_path,
        hash_model.dimensions(),
        HASH_VECTOR_QUANTIZATION,
    )?;
    let claimed_tombstones = claim_vector_tombstones(
        &workspace.hash_tombstones_path(),
        &workspace.hash_tombstones_processing_path(),
    )?;
    let removed_tombstones = claimed_tombstones.is_some();
    if let Some((_, keys)) = &claimed_tombstones {
        for key in keys {
            vector_index.remove(*key);
        }
    }
    vector_index.reserve_additional(total_chunks.saturating_sub(vector_index.size()))?;

    const BATCH_SIZE: usize = 2048;
    const CHECKPOINT_INTERVAL: usize = 262_144;
    let mut batch = Vec::<(u64, String)>::with_capacity(BATCH_SIZE);
    let mut batch_keys = HashSet::with_capacity(BATCH_SIZE);
    let mut newly_processed = 0usize;
    let mut progress_count = vector_index.size();
    let progress_path = workspace.enhancing_progress_path();
    let phase_path = workspace.enhancing_phase_path();
    let paused_path = workspace.enhancing_paused_path();
    let _ = fs::write(&phase_path, "hash");
    let _ = fs::write(&progress_path, progress_count.to_string());

    let process_batch = |batch: &mut Vec<(u64, String)>,
                         count: &mut usize,
                         store: &mut VectorStore|
     -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let texts = batch
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>();
        let embeddings = hash_model.embed_batch(&texts);
        for ((key, _), embedding) in batch.iter().zip(embeddings) {
            store.add_unchecked(*key, embedding)?;
        }
        *count += batch.len();
        batch.clear();
        Ok(())
    };

    let mut stmt = sqlite.prepare("SELECT vector_key, text FROM chunks")?;
    let rows = stmt.query_map([], |row| {
        let key = row.get::<_, i64>(0)? as u64;
        let raw: Vec<u8> = row.get(1)?;
        Ok((key, raw))
    })?;

    for row in rows {
        let (key, raw) = row?;
        if vector_index.contains(key) || !batch_keys.insert(key) {
            continue;
        }

        batch.push((key, decompress_text(raw)));
        if batch.len() >= BATCH_SIZE {
            while let Some(reason) = check_system_constraints() {
                let _ = fs::write(&paused_path, &reason);
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            let _ = fs::remove_file(&paused_path);

            process_batch(&mut batch, &mut newly_processed, &mut vector_index)?;
            batch_keys.clear();
            progress_count += BATCH_SIZE;
            let _ = fs::write(&progress_path, progress_count.to_string());
            if newly_processed.is_multiple_of(CHECKPOINT_INTERVAL) {
                vector_index.save()?;
            }
        }
    }

    let tail_len = batch.len();
    while !batch.is_empty()
        && let Some(reason) = check_system_constraints()
    {
        let _ = fs::write(&paused_path, &reason);
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
    let _ = fs::remove_file(&paused_path);
    process_batch(&mut batch, &mut newly_processed, &mut vector_index)?;
    progress_count += tail_len;
    let _ = fs::write(&progress_path, progress_count.to_string());
    if newly_processed > 0 || removed_tombstones {
        vector_index.save()?;
    }
    if let Some((path, _)) = claimed_tombstones {
        fs::remove_file(path)?;
    }
    if workspace
        .read_metadata()?
        .is_some_and(|metadata| metadata.index_generation == index_generation)
    {
        fs::write(
            workspace.hash_enhanced_generation_path(),
            index_generation.to_string(),
        )?;
    }
    Ok(newly_processed)
}

/// Compute neural Candle embeddings for all chunks and save as a separate
/// vector store. This is designed to run in a background process after the
/// lexical index already returns results to the user.
pub fn enhance_workspace_neural(
    workspace: &Workspace,
    neural_model: &dyn EmbeddingModel,
) -> Result<usize> {
    if workspace.has_overlay() || workspace.base_ref_path().exists() {
        return Ok(0);
    }

    let index_generation = workspace
        .read_metadata()?
        .map(|metadata| metadata.index_generation)
        .unwrap_or(0);
    let profile = neural_model.profile_info().unwrap_or("general");
    let model_identity = neural_model.model_identity();
    let identity_matches = match (workspace.neural_model_identity(), model_identity) {
        (Some(persisted), Some(active)) => persisted == *active,
        (None, None) => {
            workspace
                .neural_profile_name()
                .as_deref()
                .unwrap_or("general")
                == profile
        }
        _ => false,
    };
    if !identity_matches {
        let _ = fs::remove_file(workspace.vector_neural_path());
        let _ = fs::remove_file(workspace.neural_tombstones_path());
        let _ = fs::remove_file(workspace.neural_tombstones_processing_path());
        let _ = fs::remove_file(workspace.neural_enhanced_generation_path());
    }
    fs::write(workspace.neural_profile_path(), profile)?;
    if let Some(identity) = model_identity {
        fs::write(
            workspace.neural_model_path(),
            serde_json::to_vec_pretty(identity)?,
        )?;
    }

    let sqlite = open_sqlite(&workspace.sqlite_path())?;

    // Phase 1: Collect all vector_keys to determine which still need embedding.
    // This avoids decompressing text for the ~31% already done.
    let total_chunks: usize = sqlite
        .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0) as usize;

    let mut vector_index = VectorStore::open(
        &workspace.vector_neural_path(),
        neural_model.dimensions(),
        NEURAL_VECTOR_QUANTIZATION,
    )?;
    let claimed_tombstones = claim_vector_tombstones(
        &workspace.neural_tombstones_path(),
        &workspace.neural_tombstones_processing_path(),
    )?;
    if let Some((_, keys)) = &claimed_tombstones {
        for key in keys {
            vector_index.remove(*key);
        }
    }

    // Pre-reserve capacity so the index doesn't need to grow repeatedly
    let existing = vector_index.size();
    let remaining = total_chunks.saturating_sub(existing);
    vector_index.reserve_additional(remaining)?;

    let mut newly_processed = 0;
    let mut progress_count = existing;

    let progress_path = workspace.enhancing_progress_path();
    let phase_path = workspace.enhancing_phase_path();
    let paused_path = workspace.enhancing_paused_path();
    let _ = fs::write(&phase_path, "neural");
    let _ = fs::write(&progress_path, progress_count.to_string());

    // Phase 2: Stream rows and skip already-embedded keys without decompressing text.
    // Keep batches small so load and battery checks run frequently on laptops.
    const BATCH_SIZE: usize = 64;
    let mut batch: Vec<(u64, String)> = Vec::with_capacity(BATCH_SIZE);
    let mut batch_keys = HashSet::with_capacity(BATCH_SIZE);

    let mut stmt = sqlite.prepare("SELECT vector_key, text FROM chunks")?;
    let rows = stmt.query_map([], |row| {
        let key = row.get::<_, i64>(0)? as u64;
        let raw: Vec<u8> = row.get(1)?;
        Ok((key, raw))
    })?;

    let process_batch = |batch: &mut Vec<(u64, String)>,
                         count: &mut usize,
                         v_index: &mut VectorStore|
     -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let texts: Vec<&str> = batch
            .iter()
            .map(|(_, t)| {
                if t.len() > 1024 {
                    let mut end = 1024;
                    while !t.is_char_boundary(end) {
                        end -= 1;
                    }
                    &t[..end]
                } else {
                    t.as_str()
                }
            })
            .collect();

        let embeddings = neural_model.embed_batch(&texts);

        for ((key, _), embedding) in batch.iter().zip(embeddings) {
            if embedding.iter().all(|value| value.abs() <= f32::EPSILON) {
                anyhow::bail!("neural embedding produced a zero vector for key {key}");
            }
            v_index.add_unchecked(*key, embedding)?;
        }
        *count += batch.len();
        batch.clear();
        Ok(())
    };

    for row in rows {
        let (key, raw) = row?;

        // Skip without decompressing if already embedded
        if vector_index.contains(key) || !batch_keys.insert(key) {
            continue;
        }

        // Only decompress text for keys we actually need to embed
        let text = decompress_text(raw);
        batch.push((key, text));

        if batch.len() >= BATCH_SIZE {
            while neural_model.respects_system_constraints()
                && let Some(reason) = check_system_constraints()
            {
                let _ = std::fs::write(&paused_path, &reason);
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            let _ = std::fs::remove_file(&paused_path);

            process_batch(&mut batch, &mut newly_processed, &mut vector_index)?;
            batch_keys.clear();
            progress_count += BATCH_SIZE;
            let _ = std::fs::write(&progress_path, progress_count.to_string());

            if newly_processed.is_multiple_of(16_384) {
                vector_index.save()?;
            }
        }
    }

    // Process any remaining tail
    let tail_len = batch.len();
    while neural_model.respects_system_constraints()
        && !batch.is_empty()
        && let Some(reason) = check_system_constraints()
    {
        let _ = std::fs::write(&paused_path, &reason);
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
    let _ = std::fs::remove_file(&paused_path);
    process_batch(&mut batch, &mut newly_processed, &mut vector_index)?;
    progress_count += tail_len;

    let _ = std::fs::write(&progress_path, progress_count.to_string());
    vector_index.save()?;
    if let Some((path, _)) = claimed_tombstones {
        fs::remove_file(path)?;
    }
    if newly_processed > 0
        && let Some(backend) = neural_model.backend_info()
    {
        fs::write(workspace.neural_backend_path(), backend)?;
    }
    if workspace
        .read_metadata()?
        .is_some_and(|metadata| metadata.index_generation == index_generation)
    {
        fs::write(
            workspace.neural_enhanced_generation_path(),
            index_generation.to_string(),
        )?;
    }
    Ok(newly_processed)
}

fn build_indexed_chunk(chunk: Chunk, is_ignored: bool) -> IndexedChunk {
    // Stable logical keys let background vector enrichment resume across an
    // unchanged reindex. Path and bounds keep identical boilerplate chunks in
    // different files distinct.
    let vector_key = vector_key_for_chunk(
        &chunk.file_path,
        chunk.start_line,
        chunk.end_line,
        &chunk.content_hash,
    );
    let kind = format!("{:?}", chunk.kind);

    IndexedChunk {
        // Search deduplicates by the persisted vector key. Computing a second
        // formatted identity here would be pure indexing overhead.
        chunk_id: String::new(),
        file_path: chunk.file_path,
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        language: chunk.language,
        kind,
        text: chunk.text,
        content_hash: chunk.content_hash,
        vector_key,
        is_ignored,
    }
}

fn vector_key_for_chunk(
    file_path: &Path,
    start_line: usize,
    end_line: usize,
    content_hash: &str,
) -> u64 {
    let mut key_data = Vec::with_capacity(content_hash.len() + 64);
    key_data.extend_from_slice(index_path_string(file_path).as_bytes());
    key_data.extend_from_slice(&start_line.to_le_bytes());
    key_data.extend_from_slice(&end_line.to_le_bytes());
    key_data.extend_from_slice(content_hash.as_bytes());
    let digest = xxhash_rust::xxh3::xxh3_128(&key_data).to_le_bytes();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_le_bytes(bytes);
    value & i64::MAX as u64
}

fn create_overlay_tables(conn: &Connection) -> Result<()> {
    // The overlay chunks table has the exact same schema.
    // It only stores chunks for files that are different from the base.
    create_tables(conn)?;

    // Tract deleted files that exist in the base index
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tombstones (
            file_path TEXT PRIMARY KEY
        );
        "#,
    )?;

    Ok(())
}

fn apply_deletions(
    sqlite: &Connection,
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    hash_tombstones_path: &Path,
    neural_tombstones_path: Option<&Path>,
    deleted: &[PathBuf],
) -> Result<()> {
    for rel_path in deleted {
        remove_file_chunks(
            sqlite,
            writer,
            fields,
            hash_tombstones_path,
            neural_tombstones_path,
            rel_path,
        )?;
    }
    Ok(())
}

fn remove_file_chunks(
    sqlite: &Connection,
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    hash_tombstones_path: &Path,
    neural_tombstones_path: Option<&Path>,
    rel_path: &Path,
) -> Result<()> {
    let rel_str = index_path_string(rel_path);
    let keys = chunk_vector_keys_for_file(sqlite, &rel_str)?;

    writer.delete_term(Term::from_field_text(fields.file_path, &rel_str));

    append_vector_tombstones(hash_tombstones_path, &keys)?;
    if let Some(path) = neural_tombstones_path {
        append_vector_tombstones(path, &keys)?;
    }

    crate::symbols::remove_file_graph(sqlite, &rel_str)?;
    sqlite.execute("DELETE FROM chunks WHERE file_path = ?1", params![rel_str])?;
    Ok(())
}

fn append_vector_tombstones(path: &Path, keys: &[u64]) -> Result<()> {
    if keys.is_empty() {
        return Ok(());
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for key in keys {
        writeln!(file, "{key}")?;
    }
    file.sync_data()?;
    Ok(())
}

fn claim_vector_tombstones(
    pending: &Path,
    processing: &Path,
) -> Result<Option<(PathBuf, Vec<u64>)>> {
    if !processing.exists() {
        match fs::rename(pending, processing) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        }
    }

    let keys = fs::read_to_string(processing)?
        .lines()
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Some((processing.to_path_buf(), keys)))
}

fn extract_signature(chunk: &IndexedChunk) -> String {
    let is_definition = matches!(
        chunk.kind.as_str(),
        "Function"
            | "function"
            | "Class"
            | "class"
            | "Struct"
            | "struct"
            | "Trait"
            | "trait"
            | "Interface"
            | "interface"
            | "Impl"
            | "impl"
            | "Enum"
            | "enum"
    );
    if !is_definition {
        return String::new();
    }
    chunk
        .text
        .lines()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with('#')
        })
        .unwrap_or_default()
        .to_string()
}

fn add_chunk_doc(
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    chunk: &IndexedChunk,
    file_path: &str,
) -> Result<()> {
    let mut doc = TantivyDocument::default();
    doc.add_u64(fields.vector_key, chunk.vector_key);
    doc.add_text(fields.file_path, file_path);
    doc.add_u64(fields.start_line, chunk.start_line as u64);
    doc.add_u64(fields.end_line, chunk.end_line as u64);
    doc.add_text(fields.language, &chunk.language);
    doc.add_text(fields.kind, &chunk.kind);
    doc.add_text(fields.text, &chunk.text);
    if let Some(f) = fields.is_ignored {
        doc.add_u64(f, if chunk.is_ignored { 1u64 } else { 0u64 });
    }
    if let Some(f) = fields.file_path_text {
        doc.add_text(f, file_path);
    }
    if let Some(f) = fields.signature {
        let sig = extract_signature(chunk);
        if !sig.is_empty() {
            doc.add_text(f, sig);
        }
    }
    writer.add_document(doc)?;
    Ok(())
}

fn insert_chunk(
    conn: &Connection,
    chunk: &IndexedChunk,
    compressed_text: &[u8],
    file_path: &str,
    now_unix: i64,
) -> Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO chunks (
            file_path,
            start_line,
            end_line,
            language,
            kind,
            text,
            vector_key,
            modified_unix,
            is_ignored
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let is_ignored_int = if chunk.is_ignored { 1i64 } else { 0i64 };
    stmt.execute(params![
        file_path,
        chunk.start_line as i64,
        chunk.end_line as i64,
        chunk.language,
        chunk.kind,
        compressed_text,
        chunk.vector_key as i64,
        now_unix,
        is_ignored_int,
    ])?;
    crate::symbols::index_chunk_definition(conn, chunk, conn.last_insert_rowid())?;
    Ok(())
}

fn chunk_vector_keys_for_file(conn: &Connection, rel_path: &str) -> Result<Vec<u64>> {
    let mut stmt = conn.prepare("SELECT vector_key FROM chunks WHERE file_path = ?1")?;
    let rows = stmt.query_map(params![rel_path], |row| row.get::<_, i64>(0))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row? as u64);
    }

    Ok(out)
}

fn count_chunks(sqlite_path: &Path) -> Result<usize> {
    if !sqlite_path.exists() {
        return Ok(0);
    }
    let conn = Connection::open(sqlite_path)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    Ok(count as usize)
}

fn count_workspace_chunks(workspace: &Workspace) -> Result<usize> {
    let mut count = count_chunks(&workspace.sqlite_path()).unwrap_or(0);
    if workspace.has_overlay() {
        count += count_chunks(&workspace.overlay_sqlite_path()).unwrap_or(0);
        // We don't subtract tombstones here because this is just an approximate
        // indicator of index size for the CLI output / summary.
    }
    Ok(count)
}

pub fn open_sqlite(sqlite_path: &Path) -> Result<Connection> {
    let conn = Connection::open(sqlite_path)?;
    create_tables(&conn)?;
    Ok(conn)
}

/// Open SQLite in read-only mode for search and status queries.
/// Skips CREATE TABLE / PRAGMA writes for maximum speed.
pub fn open_sqlite_readonly(sqlite_path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    // Performance PRAGMAs for read-heavy workloads on large databases.
    // On multi-GB databases (large repos with millions of chunks), the default
    // 2 MB page cache causes constant disk re-reads.
    conn.execute_batch(
        "PRAGMA mmap_size = 2147483648;
         PRAGMA cache_size = -65536;
         PRAGMA temp_store = MEMORY;",
    )?;

    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS chunks (
            chunk_key INTEGER PRIMARY KEY,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            language TEXT NOT NULL,
            kind TEXT NOT NULL,
            text TEXT NOT NULL,
            vector_key INTEGER NOT NULL,
            modified_unix INTEGER NOT NULL,
            is_ignored INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS _stats (
            key TEXT PRIMARY KEY,
            value INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS symbols (
            normalized_name TEXT NOT NULL,
            chunk_key INTEGER NOT NULL,
            PRIMARY KEY (normalized_name, chunk_key)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS idx_chunks_file_path ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_vector_key ON chunks(vector_key);
        CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
        "#,
    )?;

    // Migration: Add is_ignored column to older tables
    let _ = conn.execute(
        "ALTER TABLE chunks ADD COLUMN is_ignored INTEGER NOT NULL DEFAULT 0;",
        [],
    );

    let mut table_info = conn.prepare("PRAGMA table_info(symbols)")?;
    let columns = table_info
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let legacy_single_symbol_schema = columns
        .iter()
        .any(|(name, primary_key)| name == "chunk_key" && *primary_key == 1)
        && columns
            .iter()
            .any(|(name, primary_key)| name == "normalized_name" && *primary_key == 0);
    drop(table_info);

    if legacy_single_symbol_schema {
        conn.execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP TABLE IF EXISTS symbols_legacy;
            ALTER TABLE symbols RENAME TO symbols_legacy;
            CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
            ) WITHOUT ROWID;
            INSERT OR IGNORE INTO symbols (normalized_name, chunk_key)
                SELECT normalized_name, chunk_key FROM symbols_legacy;
            DROP TABLE symbols_legacy;
            COMMIT;
            "#,
        )?;
    }

    Ok(())
}

fn finalize_graph_indexes(conn: &Connection) -> Result<()> {
    // `symbols` is WITHOUT ROWID with PRIMARY KEY (normalized_name, chunk_key),
    // so its table B-tree already serves normalized-name prefix lookups.
    // Remove the legacy duplicate index to avoid extra finalization I/O and
    // nearly doubling symbol graph storage.
    conn.execute_batch("DROP INDEX IF EXISTS idx_symbols_name;")?;
    Ok(())
}

fn build_schema() -> Schema {
    let code_indexing = TextFieldIndexing::default()
        .set_tokenizer(CODE_TOKENIZER_NAME)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let code_text_opts = TextOptions::default().set_indexing_options(code_indexing.clone());

    let mut schema = Schema::builder();
    schema.add_u64_field("vector_key", STORED);
    schema.add_text_field("file_path", STRING | STORED);
    schema.add_u64_field("start_line", STORED);
    schema.add_u64_field("end_line", STORED);
    schema.add_text_field("language", STRING | STORED);
    schema.add_text_field("kind", STRING | STORED);
    // Full text indexed with code-aware tokenizer (not STORED — lives in SQLite)
    schema.add_text_field("text", code_text_opts.clone());
    schema.add_u64_field("is_ignored", STORED);
    // BM25F fields: tokenized path + definition signature with code tokenizer
    schema.add_text_field("file_path_text", code_text_opts.clone());
    schema.add_text_field("signature", code_text_opts);
    schema.build()
}

pub fn open_tantivy_index(path: &Path) -> Result<(TantivyIndex, TantivyFields)> {
    fs::create_dir_all(path)?;

    let schema = build_schema();
    let directory = RetryingDirectory::new(MmapDirectory::open(path)?);
    let index = if path.join("meta.json").exists() {
        TantivyIndex::open(directory)?
    } else {
        TantivyIndex::open_or_create(directory, schema)?
    };

    // Register the code-aware tokenizer so both indexing and querying use it.
    index
        .tokenizers()
        .register(CODE_TOKENIZER_NAME, build_code_analyzer());

    let schema = index.schema();
    let fields = TantivyFields {
        vector_key: schema.get_field("vector_key")?,
        file_path: schema.get_field("file_path")?,
        start_line: schema.get_field("start_line")?,
        end_line: schema.get_field("end_line")?,
        language: schema.get_field("language")?,
        kind: schema.get_field("kind")?,
        text: schema.get_field("text")?,
        is_ignored: schema.get_field("is_ignored").ok(),
        file_path_text: schema.get_field("file_path_text").ok(),
        signature: schema.get_field("signature").ok(),
    };

    Ok((index, fields))
}

pub fn fetch_chunk_by_vector_key(
    conn: &Connection,
    vector_key: u64,
) -> Result<Option<IndexedChunk>> {
    let mut stmt = conn.prepare_cached(
        "SELECT file_path, start_line, end_line, language, kind, text,
                vector_key, is_ignored
         FROM chunks
         WHERE vector_key = ?1
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![vector_key as i64])?;
    if let Some(row) = rows.next()? {
        let raw_text: Vec<u8> = row.get(5)?;
        let file_path = PathBuf::from(row.get::<_, String>(0)?);
        let start_line = row.get::<_, i64>(1)? as usize;
        let end_line = row.get::<_, i64>(2)? as usize;
        let language = row.get::<_, String>(3)?;
        let kind = row.get::<_, String>(4)?;
        let vector_key = row.get::<_, i64>(6)? as u64;
        let chunk = IndexedChunk {
            chunk_id: String::new(),
            file_path,
            start_line,
            end_line,
            language,
            kind,
            text: decompress_text(raw_text),
            content_hash: String::new(),
            vector_key,
            is_ignored: row.get::<_, bool>(7)?,
        };

        return Ok(Some(chunk));
    }

    Ok(None)
}

/// Batch-fetch chunks by vector key in a single SQL round-trip.
/// On large indexes (3.8M chunks), this reduces hundreds of individual
/// B-tree traversals to 1-2 batched queries.
pub fn fetch_chunks_by_vector_keys_batch(
    conn: &Connection,
    keys: &[u64],
) -> Result<HashMap<u64, IndexedChunk>> {
    let mut result = HashMap::with_capacity(keys.len());
    if keys.is_empty() {
        return Ok(result);
    }

    // SQLite supports up to 999 bind parameters; batch in groups of 500.
    for batch in keys.chunks(500) {
        let placeholders = batch.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT file_path, start_line, end_line, language, kind, text, \
             vector_key, is_ignored \
             FROM chunks WHERE vector_key IN ({})",
            placeholders
        );
        let mut stmt = conn.prepare(&query)?;

        let params: Vec<rusqlite::types::Value> = batch
            .iter()
            .map(|k| rusqlite::types::Value::Integer(*k as i64))
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();

        let mut rows = stmt.query(param_refs.as_slice())?;
        while let Some(row) = rows.next()? {
            let raw_text: Vec<u8> = row.get(5)?;
            let file_path = PathBuf::from(row.get::<_, String>(0)?);
            let start_line = row.get::<_, i64>(1)? as usize;
            let end_line = row.get::<_, i64>(2)? as usize;
            let language = row.get::<_, String>(3)?;
            let kind = row.get::<_, String>(4)?;
            let vector_key = row.get::<_, i64>(6)? as u64;
            let chunk = IndexedChunk {
                chunk_id: String::new(),
                file_path,
                start_line,
                end_line,
                language,
                kind,
                text: decompress_text(raw_text),
                content_hash: String::new(),
                vector_key,
                is_ignored: row.get::<_, bool>(7)?,
            };
            result.insert(vector_key, chunk);
        }
    }

    Ok(result)
}

pub fn read_preview_line(content: &str) -> String {
    content
        .lines()
        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("//"))
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn fetch_chunk_by_id(
    search_doc: TantivyDocument,
    fields: &TantivyFields,
) -> Option<IndexedChunk> {
    let vector_key = search_doc
        .get_first(fields.vector_key)
        .and_then(|v| v.as_u64())?;

    let file_path = PathBuf::from(
        search_doc
            .get_first(fields.file_path)
            .and_then(|v| v.as_str())?
            .to_string(),
    );

    let start_line = search_doc
        .get_first(fields.start_line)
        .and_then(|v| v.as_u64())? as usize;

    let end_line = search_doc
        .get_first(fields.end_line)
        .and_then(|v| v.as_u64())? as usize;

    let language = search_doc
        .get_first(fields.language)
        .and_then(|v| v.as_str())?
        .to_string();

    let kind = search_doc
        .get_first(fields.kind)
        .and_then(|v| v.as_str())?
        .to_string();

    // Text may be absent (STORED removed); callers populate from SQLite.
    let text = search_doc
        .get_first(fields.text)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let is_ignored = fields
        .is_ignored
        .and_then(|f| search_doc.get_first(f))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        > 0;
    Some(IndexedChunk {
        chunk_id: String::new(),
        file_path,
        start_line,
        end_line,
        language,
        kind,
        text,
        content_hash: String::new(),
        vector_key,
        is_ignored,
    })
}

pub fn diff_for_workspace(workspace: &Workspace) -> Result<MerkleDiff> {
    let old_snapshot = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
    let skip_gitignore = match workspace.read_metadata()? {
        Some(m) => m.skip_gitignore,
        None => false,
    };
    let new_snapshot = MerkleSnapshot::build(&workspace.root, skip_gitignore)?;
    Ok(old_snapshot.diff(&new_snapshot))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::chunking::{Chunk, ChunkKind};
    use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
    use crate::workspace::Workspace;

    use super::*;

    struct RecordedTestEmbedding {
        model: HashEmbeddingModel,
        backend: &'static str,
        identity: crate::embedding::NeuralModelIdentity,
    }

    impl EmbeddingModel for RecordedTestEmbedding {
        fn dimensions(&self) -> usize {
            self.model.dimensions()
        }

        fn embed(&self, text: &str) -> Vec<f32> {
            self.model.embed(text)
        }

        fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
            self.model.embed_batch(texts)
        }

        fn backend_info(&self) -> Option<&'static str> {
            Some(self.backend)
        }

        fn profile_info(&self) -> Option<&'static str> {
            Some("static")
        }

        fn model_identity(&self) -> Option<&crate::embedding::NeuralModelIdentity> {
            Some(&self.identity)
        }
    }

    #[test]
    fn create_tables_migrates_symbols_to_many_names_per_chunk() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER PRIMARY KEY
             ) WITHOUT ROWID;
             INSERT INTO symbols (normalized_name, chunk_key) VALUES ('router', 7);",
        )
        .unwrap();

        create_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO symbols (normalized_name, chunk_key) VALUES (?1, ?2)",
            params!["routekind", 7],
        )
        .unwrap();

        let count = conn
            .query_row(
                "SELECT COUNT(*) FROM symbols WHERE chunk_key = 7",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn graph_finalization_removes_redundant_symbol_name_index() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();
        conn.execute_batch("CREATE INDEX idx_symbols_name ON symbols(normalized_name);")
            .unwrap();

        finalize_graph_indexes(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_symbols_name'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    #[serial]
    fn enhance_load_throttle_is_lenient_and_configurable() {
        // #62: the load throttle was too aggressive (paused enhancement at
        // ~0.75-0.8x CPUs despite nice(10)+25%-core capping), so neural never
        // built on busy machines. Default is now 2.0x and env-configurable.
        // SAFETY: test is #[serial]; no other thread mutates this env var.
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO") };
        // A fully-loaded machine (load == cpus) does NOT pause at the default.
        assert!(parse_system_load(8.0, 8.0).is_none());
        // Genuine sustained oversubscription (load > 2x cpus) does pause.
        assert!(parse_system_load(20.0, 8.0).is_some());
        // Configurable: a stricter ratio pauses earlier.
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0.5") };
        assert!(parse_system_load(8.0, 8.0).is_some());
        // A non-positive ratio disables the load check entirely.
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0") };
        assert!(parse_system_load(100.0, 8.0).is_none());
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO") };
    }

    #[test]
    fn identical_content_chunks_get_distinct_vector_keys() {
        // Path and bounds are part of stable vector identity, so identical
        // boilerplate in different files cannot share one usearch key.
        let make = |path: &str| Chunk {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from(path),
            start_line: 1,
            end_line: 1,
            text: "// SPDX-License-Identifier: MIT\n".to_string(),
            language: "rust".to_string(),
            kind: ChunkKind::Module,
            content_hash: "identical-hash".to_string(),
        };
        let a = build_indexed_chunk(make("a.rs"), false);
        let b = build_indexed_chunk(make("b.rs"), false);

        assert_eq!(a.content_hash, b.content_hash);
        assert_ne!(
            a.vector_key, b.vector_key,
            "identical-content chunks collided on one vector key"
        );
    }

    #[test]
    fn unchanged_chunk_gets_stable_vector_key() {
        let make = || Chunk {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("src/lib.rs"),
            start_line: 10,
            end_line: 14,
            text: "pub fn stable() {}\n".to_string(),
            language: "rust".to_string(),
            kind: ChunkKind::Function,
            content_hash: "stable-content-hash".to_string(),
        };

        assert_eq!(
            build_indexed_chunk(make(), false).vector_key,
            build_indexed_chunk(make(), false).vector_key,
            "unchanged chunks must keep background enrichment keys across reindex"
        );
    }

    #[test]
    fn dropping_index_batch_producer_cancels_blocked_sender() {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<IndexedFileBatch>(0);
        let handle = std::thread::spawn(move || {
            assert!(
                sender.send(Vec::new()).is_err(),
                "receiver drop must cancel blocked producer send"
            );
        });

        drop(IndexBatchProducer::new(receiver, handle));
    }

    #[test]
    fn index_batch_producer_propagates_worker_panic() {
        let (_sender, receiver) = std::sync::mpsc::sync_channel::<IndexedFileBatch>(0);
        let handle = std::thread::spawn(|| panic!("test producer panic"));

        let err = IndexBatchProducer::new(receiver, handle)
            .finish()
            .unwrap_err();
        assert!(
            err.to_string().contains("producer thread panicked"),
            "unexpected producer error: {err:#}"
        );
    }

    #[test]
    #[serial]
    fn indexes_simple_repo() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("lib.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.deleted_files, 0);
        assert!(summary.total_chunks >= 1);
        assert!(workspace_is_indexed(&workspace));
        assert!(workspace.vector_path().metadata().unwrap().len() > 0);
    }

    #[test]
    #[serial]
    fn foreground_index_is_queryable_before_hash_enrichment() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("lib.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let summary = index_workspace(&workspace, &model).unwrap();

        assert!(workspace_is_indexed(&workspace));
        let foreground = VectorStore::open_readonly(
            &workspace.vector_path(),
            EMBEDDING_DIMENSIONS,
            ScalarKind::F16,
        )
        .unwrap();
        assert_eq!(
            foreground.size(),
            0,
            "foreground index must not block on hash HNSW construction"
        );

        assert_eq!(
            enhance_workspace_hash(&workspace, &model).unwrap(),
            summary.total_chunks
        );
        assert_eq!(
            fs::read_to_string(workspace.enhancing_phase_path()).unwrap(),
            "hash"
        );
        assert_eq!(
            fs::read_to_string(workspace.hash_enhanced_generation_path()).unwrap(),
            workspace
                .read_metadata()
                .unwrap()
                .unwrap()
                .index_generation
                .to_string()
        );
        let enriched = VectorStore::open_readonly(
            &workspace.vector_path(),
            EMBEDDING_DIMENSIONS,
            ScalarKind::F16,
        )
        .unwrap();
        assert_eq!(enriched.size(), summary.total_chunks);
        assert_eq!(enhance_workspace_hash(&workspace, &model).unwrap(), 0);
        assert_eq!(
            fs::read_to_string(workspace.enhancing_progress_path()).unwrap(),
            summary.total_chunks.to_string(),
            "resumed enhancement progress must not count persisted vectors twice"
        );
    }

    #[test]
    #[serial]
    fn enhancement_handles_duplicate_stable_vector_keys() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir(root.path().join(".git")).unwrap();
        fs::write(
            root.path().join("lib.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let summary = index_workspace(&workspace, &model).unwrap();

        let sqlite = open_sqlite(&workspace.sqlite_path()).unwrap();
        sqlite
            .execute(
                "INSERT INTO chunks (
                    file_path, start_line, end_line, language, kind, text,
                    vector_key, modified_unix, is_ignored
                 )
                 SELECT
                    file_path, start_line, end_line, language, kind, text,
                    vector_key, modified_unix, is_ignored
                 FROM chunks
                 LIMIT 1",
                [],
            )
            .unwrap();

        let row_count = sqlite
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap() as usize;
        let vector_key_count = sqlite
            .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap() as usize;
        assert_eq!(row_count, summary.total_chunks + 1);
        assert_eq!(vector_key_count, summary.total_chunks);

        assert_eq!(
            enhance_workspace_hash(&workspace, &model).unwrap(),
            vector_key_count
        );
        let hash_store = VectorStore::open_readonly(
            &workspace.vector_path(),
            EMBEDDING_DIMENSIONS,
            ScalarKind::F16,
        )
        .unwrap();
        assert_eq!(hash_store.size(), vector_key_count);

        assert_eq!(
            enhance_workspace_neural(&workspace, &model).unwrap(),
            vector_key_count
        );
        let neural_store = VectorStore::open_readonly(
            &workspace.vector_neural_path(),
            EMBEDDING_DIMENSIONS,
            crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
        )
        .unwrap();
        assert_eq!(neural_store.size(), vector_key_count);
    }

    #[test]
    #[serial]
    fn workspace_is_indexed_handles_interruption() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();

        // Initially false
        assert!(!workspace_is_indexed(&workspace));

        let md = crate::workspace::WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: None,
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 0,
        };
        std::fs::create_dir_all(&workspace.index_dir).unwrap();
        std::fs::write(workspace.sqlite_path(), "").unwrap();
        std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        std::fs::write(workspace.vector_path(), "").unwrap();

        std::fs::write(
            workspace.index_dir.join("workspace.json"),
            serde_json::to_string(&md).unwrap(),
        )
        .unwrap();

        // last_indexed_at_unix is None → treat as not indexed
        assert!(!workspace_is_indexed(&workspace));

        let md_fixed = crate::workspace::WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(123),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 0,
        };
        std::fs::write(
            workspace.index_dir.join("workspace.json"),
            serde_json::to_string(&md_fixed).unwrap(),
        )
        .unwrap();
        // Completed metadata cannot make corrupt/incomplete stores queryable.
        assert!(!workspace_is_indexed(&workspace));
        assert!(
            workspace
                .quick_index_health()
                .issues
                .iter()
                .any(|issue| issue.contains("cached index statistics are missing"))
        );
    }

    #[test]
    #[serial]
    fn respects_gitignore_by_default() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();

        fs::write(root.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(
            root.path().join("kept.rs"),
            "pub fn included_symbol() -> i32 { 42 }\n",
        )
        .unwrap();
        fs::write(
            root.path().join("ignored.rs"),
            "pub fn excluded_symbol() -> i32 { 0 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let _ = index_workspace(&workspace, &model).unwrap();

        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT file_path FROM chunks ORDER BY file_path")
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(rows.iter().any(|path| path == "kept.rs"));
        assert!(!rows.iter().any(|path| path == "ignored.rs"));
    }

    #[test]
    #[serial]
    fn enhance_workspace_neural_creates_vector_store() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("lib.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();
        fs::write(
            root.path().join("util.rs"),
            "pub fn format_currency(val: f64) -> String { format!(\"${:.2}\", val) }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

        // Phase 1: index with hash
        let summary = index_workspace(&workspace, &hash_model).unwrap();
        assert!(summary.total_chunks >= 2);
        assert!(!workspace.vector_neural_path().exists());

        // Phase 2: enhance with a deterministic stand-in carrying backend attribution.
        let neural_model = RecordedTestEmbedding {
            model: HashEmbeddingModel::new(EMBEDDING_DIMENSIONS),
            backend: "test local neural backend",
            identity: crate::embedding::configured_neural_model_identity(),
        };
        let enhanced = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert_eq!(enhanced, summary.total_chunks);
        assert_eq!(
            fs::read_to_string(workspace.enhancing_phase_path()).unwrap(),
            "neural"
        );

        // Verify neural vector store was created
        assert!(workspace.vector_neural_path().exists());

        // Verify the neural store has correct number of vectors
        let store = crate::vector_store::VectorStore::open(
            &workspace.vector_neural_path(),
            EMBEDDING_DIMENSIONS,
            crate::vector_store::NEURAL_VECTOR_QUANTIZATION,
        )
        .unwrap();
        assert_eq!(store.size(), enhanced);
        assert_eq!(
            fs::read_to_string(workspace.neural_backend_path()).unwrap(),
            "test local neural backend"
        );
        assert_eq!(
            fs::read_to_string(workspace.neural_profile_path()).unwrap(),
            "static"
        );
        assert_eq!(
            fs::read_to_string(workspace.neural_enhanced_generation_path()).unwrap(),
            workspace
                .read_metadata()
                .unwrap()
                .unwrap()
                .index_generation
                .to_string()
        );
        let status = crate::workspace::list_workspaces()
            .unwrap()
            .into_iter()
            .find(|status| status.id == workspace.id)
            .unwrap();
        assert_eq!(
            status.neural_backend.as_deref(),
            Some("test local neural backend")
        );
    }

    #[test]
    #[serial]
    fn enhance_workspace_neural_is_idempotent() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("app.rs"),
            "pub fn process(data: &str) -> String { data.to_uppercase() }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash_model).unwrap();

        let first_model = RecordedTestEmbedding {
            model: HashEmbeddingModel::new(EMBEDDING_DIMENSIONS),
            backend: "first backend",
            identity: crate::embedding::configured_neural_model_identity(),
        };
        let second_model = RecordedTestEmbedding {
            model: HashEmbeddingModel::new(EMBEDDING_DIMENSIONS),
            backend: "unused backend",
            identity: crate::embedding::configured_neural_model_identity(),
        };

        let n1 = enhance_workspace_neural(&workspace, &first_model).unwrap();
        assert!(n1 > 0, "first enhance should process chunks");
        let n2 = enhance_workspace_neural(&workspace, &second_model).unwrap();
        assert_eq!(n2, 0, "second enhance should skip already-processed chunks");
        assert_eq!(
            fs::read_to_string(workspace.neural_backend_path()).unwrap(),
            "first backend",
            "no-op enhancement must not rewrite recorded backend"
        );
    }

    #[test]
    #[serial]
    fn enhance_neural_reflects_index_changes() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("mod.rs"),
            "pub fn original() -> i32 { 1 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash_model).unwrap();

        let neural_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let n1 = enhance_workspace_neural(&workspace, &neural_model).unwrap();

        // Add more files and re-index
        for i in 0..5 {
            fs::write(
                root.path().join(format!("extra_{i}.rs")),
                format!("pub fn extra_{i}() -> i32 {{ {i} }}\n"),
            )
            .unwrap();
        }
        index_workspace(&workspace, &hash_model).unwrap();

        // Re-enhance — should now cover more chunks
        let n2 = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert!(
            n2 > n1,
            "neural enhancement should cover new chunks: before={n1} after={n2}"
        );
    }

    #[test]
    #[serial]
    fn foreground_reindex_journals_stale_neural_vectors() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(
            root.path().join("mod.rs"),
            "pub fn original() -> i32 { 1 }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let neural_model = RecordedTestEmbedding {
            model: HashEmbeddingModel::new(EMBEDDING_DIMENSIONS),
            backend: "test local neural backend",
            identity: crate::embedding::configured_neural_model_identity(),
        };
        index_workspace(&workspace, &hash_model).unwrap();
        enhance_workspace_hash(&workspace, &hash_model).unwrap();
        enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert!(!workspace.needs_neural_enhancement());

        fs::write(
            root.path().join("mod.rs"),
            "pub fn replacement() -> i32 { 2 }\n",
        )
        .unwrap();
        index_workspace(&workspace, &hash_model).unwrap();
        assert!(
            workspace.hash_tombstones_path().exists(),
            "foreground edit must journal stale hash keys without loading hash graph"
        );
        assert!(
            workspace.neural_tombstones_path().exists(),
            "foreground edit must journal stale neural keys without loading neural graph"
        );

        enhance_workspace_hash(&workspace, &hash_model).unwrap();
        assert!(!workspace.hash_tombstones_path().exists());
        assert!(!workspace.hash_tombstones_processing_path().exists());
        assert_eq!(
            fs::read_to_string(workspace.hash_enhanced_generation_path()).unwrap(),
            workspace
                .read_metadata()
                .unwrap()
                .unwrap()
                .index_generation
                .to_string()
        );
        assert!(workspace.needs_neural_enhancement());
        assert_eq!(
            enhance_workspace_neural(&workspace, &neural_model).unwrap(),
            1
        );
        assert!(!workspace.neural_tombstones_path().exists());
        assert!(!workspace.neural_tombstones_processing_path().exists());
        assert_eq!(
            fs::read_to_string(workspace.neural_enhanced_generation_path()).unwrap(),
            workspace
                .read_metadata()
                .unwrap()
                .unwrap()
                .index_generation
                .to_string()
        );
        assert!(!workspace.needs_neural_enhancement());
    }

    #[test]
    #[serial]
    fn enhance_neural_returns_zero_for_empty_index() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash_model).unwrap();

        let neural_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let n = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert_eq!(n, 0, "empty index should produce zero enhanced chunks");
    }

    #[test]
    fn decompress_text_roundtrips_zstd() {
        let original = "pub fn hello() -> &str { \"world\" }\n";
        let compressed = super::compress_text(original);
        let decompressed = super::decompress_text(compressed);
        assert_eq!(decompressed, original);
    }

    #[test]
    fn tantivy_segment_writes_retry_transient_permission_denials() {
        use tantivy::directory::{Directory, RamDirectory, TerminatingWrite};

        let directory = RamDirectory::create();
        let attempts = std::cell::Cell::new(0);
        let path = PathBuf::from("segment.term");

        let writer = open_write_with_retry(|| {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt < 2 {
                return Err(OpenWriteError::wrap_io_error(
                    std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                    path.clone(),
                ));
            }
            directory.open_write(&path)
        })
        .unwrap();

        writer.terminate().unwrap();
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn tantivy_segment_writes_do_not_retry_non_permission_errors() {
        let attempts = std::cell::Cell::new(0);
        let path = PathBuf::from("segment.term");

        let result = open_write_with_retry(|| {
            attempts.set(attempts.get() + 1);
            Err(OpenWriteError::FileAlreadyExists(path.clone()))
        });

        assert!(matches!(result, Err(OpenWriteError::FileAlreadyExists(_))));
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn transient_tantivy_write_denial_is_retryable() {
        let denied = anyhow::Error::from(tantivy::TantivyError::OpenWriteError(
            OpenWriteError::wrap_io_error(
                std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                PathBuf::from("segment.fast"),
            ),
        ));
        let missing = anyhow::Error::from(tantivy::TantivyError::OpenWriteError(
            OpenWriteError::wrap_io_error(
                std::io::Error::from(std::io::ErrorKind::NotFound),
                PathBuf::from("segment.fast"),
            ),
        ));

        assert!(is_transient_tantivy_write_error(&denied));
        assert!(!is_transient_tantivy_write_error(&missing));
    }

    #[test]
    fn whole_index_retry_retries_transient_tantivy_write_denial() {
        let attempts = std::cell::Cell::new(0);

        let result: Result<()> = retry_transient_tantivy_writes(|| {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 0 {
                return Err(anyhow::Error::from(tantivy::TantivyError::OpenWriteError(
                    OpenWriteError::wrap_io_error(
                        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                        PathBuf::from("segment.fast"),
                    ),
                )));
            }
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn prepared_chunk_compresses_text_before_persistence() {
        let chunk = build_indexed_chunk(
            Chunk {
                id: uuid::Uuid::new_v4(),
                file_path: PathBuf::from("src/lib.rs"),
                start_line: 1,
                end_line: 1,
                text: "pub fn prepared() {}\n".to_string(),
                language: "rust".to_string(),
                kind: ChunkKind::Function,
                content_hash: "prepared-content-hash".to_string(),
            },
            false,
        );
        let prepared = super::prepare_indexed_chunk(chunk.clone());

        assert_eq!(prepared.chunk.text, chunk.text);
        assert_eq!(super::decompress_text(prepared.compressed_text), chunk.text);
    }

    #[test]
    fn decompress_text_handles_plain_utf8() {
        let plain = b"plain text, not zstd";
        let decompressed = super::decompress_text(plain.to_vec());
        assert_eq!(decompressed, "plain text, not zstd");
    }

    #[test]
    fn read_preview_line_skips_blanks_and_comments() {
        let content = "\n\n  // This is a comment\n  pub fn main() {}\n";
        assert_eq!(super::read_preview_line(content), "pub fn main() {}");
    }

    #[test]
    fn read_preview_line_returns_empty_for_all_comments() {
        let content = "// only comment\n// another\n";
        assert_eq!(super::read_preview_line(content), "");
    }

    #[test]
    fn read_preview_line_handles_empty_input() {
        assert_eq!(super::read_preview_line(""), "");
    }

    #[test]
    #[serial]
    fn remove_workspace_index_cleans_up() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        fs::write(root.path().join("lib.rs"), "pub fn to_remove() {}\n").unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert!(workspace.index_dir.exists());

        remove_workspace_index(&workspace).unwrap();

        assert!(!workspace.index_dir.exists());
    }

    #[test]
    fn workspace_id_is_deterministic() {
        use crate::workspace::workspace_id;
        use std::path::Path;

        let id1 = workspace_id(Path::new("/some/project"));
        let id2 = workspace_id(Path::new("/some/project"));
        let id3 = workspace_id(Path::new("/different/project"));

        assert_eq!(id1, id2, "same path should produce same id");
        assert_ne!(id1, id3, "different paths should produce different ids");
        assert!(!id1.is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_parse_pmset_batt() {
        let ac_output = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=22741091)\t96%; AC attached; not charging present: true";
        let batt_output = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=22741091)\t96%; discharging; (no estimate) present: true";

        assert_eq!(super::parse_pmset_batt(ac_output), None);
        assert_eq!(
            super::parse_pmset_batt(batt_output),
            Some("Battery Power".to_string())
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_parse_pmset_therm() {
        let normal = "Note: No thermal warning level has been recorded\nNote: No performance warning level has been recorded";
        let throttled = "Note: Thermal warning level CPU_Speed_Limit = 50";

        assert_eq!(super::parse_pmset_therm(normal), None);
        assert_eq!(
            super::parse_pmset_therm(throttled),
            Some("Thermal Throttling".to_string())
        );
    }
}
