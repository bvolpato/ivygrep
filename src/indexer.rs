use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::{Connection, OptionalExtension, Statement, ToSql, params, params_from_iter};
use serde::{Deserialize, Serialize};

use tantivy::directory::error::OpenWriteError;
use tantivy::schema::Value;
use tantivy::{TantivyDocument, Term};

use crate::text::first_code_line_range;

use crate::chunking::{
    Chunk, RustDocInclude, chunk_rust_doc_include, chunk_source_with_metadata, is_indexable_file,
};
use crate::embedding::EmbeddingModel;
use crate::jobs::{self, JobKind, JobUpdate};
use crate::merkle::{MerkleDiff, MerkleSnapshot};
use crate::vector_store::{HASH_VECTOR_QUANTIZATION, NEURAL_VECTOR_QUANTIZATION, VectorStore};
use crate::workspace::{Workspace, WorkspaceMetadata, index_path_string};

mod compression;
mod git_state;
mod resources;
mod staging;
mod storage;

use compression::compress_text;
pub use compression::{decompress_text, try_decompress_text};
use git_state::{
    clean_git_checkout_state, files_have_same_contents, indexed_git_state_path,
    record_indexed_git_state, refresh_clean_base_metadata,
};
use resources::{
    NEURAL_BATCH_SIZE_REFRESH_INTERVAL, check_memory_before_index, check_system_constraints,
    indexing_pool, neural_enhance_batch_size, tantivy_writer_settings,
};
use staging::FreshIndexStaging;
pub use storage::{
    StorageHandles, TantivyFields, open_sqlite, open_sqlite_readonly, open_storage,
    open_tantivy_index,
};
use storage::{
    apply_bulk_write_pragmas, apply_default_write_pragmas, apply_fresh_staging_pragmas,
    create_secondary_indexes, create_tables, create_tables_schema, ensure_hash_vector_store,
    finalize_graph_indexes, open_storage_with_options,
};
const TANTIVY_INDEX_RETRY_ATTEMPTS: u32 = 3;
const TANTIVY_INDEX_RETRY_BASE_DELAY_MS: u64 = 250;
const MIB: u64 = 1024 * 1024;
const MAX_RUST_DOC_INCLUDES_PER_SOURCE: usize = 16;
const MAX_RUST_DOC_INCLUDE_BYTES: u64 = MIB;
const MAX_RUST_DOC_INCLUDE_TOTAL_BYTES: u64 = 2 * MIB;
const MAX_RUST_DOC_INCLUDE_CHUNKS_PER_SOURCE: usize = 128;
const SYMBOL_INSERT_BATCH_ROWS: usize = 256;
const INDEX_FILE_BATCH_SIZE: usize = 64;
// Soft per-transaction target, checked after each file. One file's bounded
// chunk-key batch may exceed it before the journal and SQLite commit checkpoint.
const MAX_VECTOR_TOMBSTONE_TRANSACTION_BYTES: usize = 1024 * 1024;
const INDEXED_SKIP_GITIGNORE_FILE: &str = ".indexed_skip_gitignore";

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
#[serde(default)]
pub struct IndexingPhaseTimings {
    pub discovery_ms: f64,
    pub persist_ms: f64,
    pub finalize_ms: f64,
    pub secondary_indexes_ms: f64,
    pub vector_key_count_ms: f64,
    pub sqlite_commit_ms: f64,
    pub tantivy_commit_ms: f64,
    pub tantivy_merge_ms: f64,
    pub staging_publish_ms: f64,
    pub metadata_ms: f64,
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
    tantivy_doc: TantivyDocument,
}

fn prepare_indexed_chunk(chunk: IndexedChunk, fields: &TantivyFields) -> PreparedIndexedChunk {
    let compressed_text = compress_text(&chunk.text);
    let file_path = index_path_string(&chunk.file_path);
    let tantivy_doc = build_chunk_doc(fields, &chunk, &file_path);
    PreparedIndexedChunk {
        chunk,
        compressed_text,
        tantivy_doc,
    }
}

struct IndexedFile {
    rel_path: PathBuf,
    chunks: Vec<PreparedIndexedChunk>,
    included_paths: Vec<PathBuf>,
    file_edges: Vec<crate::context_graph::FileEdge>,
    unresolved_dependencies: Vec<crate::context_graph::UnresolvedDependency>,
    manifest_resolution_signature: Option<String>,
}

type IndexedFileBatch = Vec<IndexedFile>;

struct IndexBatchProducer {
    receiver: Option<std::sync::mpsc::Receiver<Result<IndexedFileBatch>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl IndexBatchProducer {
    fn new(
        receiver: std::sync::mpsc::Receiver<Result<IndexedFileBatch>>,
        handle: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            handle: Some(handle),
        }
    }

    fn recv(&self) -> Option<Result<IndexedFileBatch>> {
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

fn spawn_index_batch_producer(
    workspace: &Workspace,
    diff: &MerkleDiff,
    current_snapshot: Option<Arc<MerkleSnapshot>>,
    fields: &TantivyFields,
    is_fresh_index: bool,
    show_progress: bool,
) -> IndexBatchProducer {
    let total = diff.added_or_modified.len();
    let (sender, receiver) = std::sync::mpsc::sync_channel::<Result<IndexedFileBatch>>(2);
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let root = workspace.root.clone();
    let progress_path = workspace.indexing_progress_path();
    let diff_paths = diff.added_or_modified.clone();
    let fields = fields.clone();

    let _ = fs::write(&progress_path, format!("0/{total}"));
    let handle = std::thread::spawn(move || {
        for batch_paths in diff_paths.chunks(INDEX_FILE_BATCH_SIZE) {
            let file_chunks: Result<Vec<_>> = indexing_pool().install(|| {
                batch_paths
                    .par_iter()
                    .map(|(rel_path, is_ignored)| {
                        let empty_incremental_file = |rel: &Path| {
                            (!is_fresh_index).then(|| IndexedFile {
                                rel_path: rel.to_path_buf(),
                                chunks: Vec::new(),
                                included_paths: Vec::new(),
                                file_edges: Vec::new(),
                                unresolved_dependencies: Vec::new(),
                                manifest_resolution_signature: None,
                            })
                        };

                        let abs_path = root.join(rel_path);
                        let content_bytes = fs::read(&abs_path).with_context(|| {
                            format!("failed reading source file {}", abs_path.display())
                        })?;
                        if !is_indexable_file(rel_path, &content_bytes) {
                            progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            return Ok(empty_incremental_file(rel_path));
                        }

                        let content = String::from_utf8(content_bytes).unwrap_or_else(|error| {
                            String::from_utf8_lossy(&error.into_bytes()).into_owned()
                        });
                        let mut chunked = chunk_source_with_metadata(rel_path, &content);
                        let file_graph = crate::context_graph::extract_file_graph(
                            &root,
                            current_snapshot.as_deref(),
                            rel_path,
                            &content,
                        );
                        let (included_chunks, included_paths) = load_rust_doc_includes(
                            &root,
                            rel_path,
                            &chunked.rust_doc_includes,
                            current_snapshot.as_deref(),
                        );
                        chunked.chunks.extend(included_chunks);
                        let mut seen_vector_keys = HashSet::new();
                        let mut content_occurrences = HashMap::new();
                        let mut indexed: Vec<_> = chunked
                            .chunks
                            .into_iter()
                            .map(|chunk| {
                                let content_identity =
                                    xxhash_rust::xxh3::xxh3_128(chunk.text.as_bytes());
                                let occurrence =
                                    content_occurrences.entry(content_identity).or_insert(0);
                                let indexed = if *occurrence == 0 {
                                    build_indexed_chunk(chunk, *is_ignored)
                                } else {
                                    build_indexed_chunk_with_occurrence(
                                        chunk,
                                        *is_ignored,
                                        *occurrence,
                                    )
                                };
                                *occurrence += 1;
                                indexed
                            })
                            .filter(|chunk| seen_vector_keys.insert(chunk.vector_key))
                            .map(|chunk| prepare_indexed_chunk(chunk, &fields))
                            .collect();
                        if let (Some(field), Some(first)) =
                            (fields.text_trigrams, indexed.first_mut())
                        {
                            first.tantivy_doc.add_text(field, &content);
                        }

                        let completed =
                            progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if show_progress && completed.is_multiple_of(500) {
                            eprint!("\r\x1b[K  ⠋ indexing {completed}/{total} files...");
                        }
                        if completed.is_multiple_of(2000) {
                            let _ = fs::write(&progress_path, format!("{completed}/{total}"));
                        }

                        if indexed.is_empty()
                            && included_paths.is_empty()
                            && file_graph.edges.is_empty()
                            && file_graph.unresolved_dependencies.is_empty()
                        {
                            return Ok(empty_incremental_file(rel_path));
                        }
                        Ok(Some(IndexedFile {
                            rel_path: rel_path.clone(),
                            chunks: indexed,
                            included_paths,
                            file_edges: file_graph.edges,
                            unresolved_dependencies: file_graph.unresolved_dependencies,
                            manifest_resolution_signature:
                                crate::context_graph::manifest_resolution_signature(
                                    rel_path, &content,
                                ),
                        }))
                    })
                    .collect::<Result<Vec<_>>>()
                    .map(|files| files.into_iter().flatten().collect())
            });

            match file_chunks {
                Ok(file_chunks) => {
                    if !file_chunks.is_empty() && sender.send(Ok(file_chunks)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = sender.send(Err(err));
                    break;
                }
            }
        }
    });

    IndexBatchProducer::new(receiver, handle)
}

pub fn workspace_is_indexed(workspace: &Workspace) -> bool {
    workspace.quick_index_health().is_queryable()
}

pub(crate) fn indexed_skip_gitignore(workspace: &Workspace) -> Option<bool> {
    fs::read_to_string(workspace.index_dir.join(INDEXED_SKIP_GITIGNORE_FILE))
        .ok()
        .and_then(|value| value.trim().parse::<bool>().ok())
}

pub(crate) fn workspace_index_matches_skip_gitignore(
    workspace: &Workspace,
    skip_gitignore: bool,
) -> bool {
    indexed_skip_gitignore(workspace) == Some(skip_gitignore)
}

fn record_indexed_skip_gitignore(workspace: &Workspace, skip_gitignore: bool) -> Result<()> {
    fs::write(
        workspace.index_dir.join(INDEXED_SKIP_GITIGNORE_FILE),
        skip_gitignore.to_string(),
    )?;
    Ok(())
}

pub fn remove_workspace_index(workspace: &Workspace) -> Result<()> {
    if !workspace.index_dir.exists() {
        return Ok(());
    }
    workspace.ensure_dirs()?;
    let lock_path = workspace.lock_path();
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open index lock {}", lock_path.display()))?;
    fs2::FileExt::lock_exclusive(&lock_file)
        .with_context(|| format!("failed to acquire index lock {}", lock_path.display()))?;

    let remove_result = remove_workspace_index_contents(workspace);
    let unlock_result = fs2::FileExt::unlock(&lock_file)
        .with_context(|| format!("failed to release index lock {}", lock_path.display()));
    remove_result?;
    unlock_result
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
    crate::vector_store::remove_store_files(&workspace.vector_neural_path());
    let _ = fs::remove_file(workspace.neural_model_path());
    let _ = fs::remove_file(workspace.neural_profile_path());
    let _ = fs::remove_file(workspace.neural_backend_path());
    let _ = fs::remove_file(workspace.neural_tombstones_path());
    let _ = fs::remove_file(workspace.neural_tombstones_processing_path());
    let _ = fs::remove_file(workspace.neural_enhanced_generation_path());
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
    let indexing_started = Instant::now();
    workspace.ensure_dirs()?;

    check_memory_before_index()?;

    // Rebuild only after locking. Replacing the lock inode first would break
    // mutual exclusion with a process that still holds the old inode.
    let lock_path = workspace.lock_path();
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;
    fs2::FileExt::lock_exclusive(&lock_file)
        .with_context(|| format!("failed to acquire index lock {}", lock_path.display()))?;

    let preserved_metadata = workspace.read_metadata().ok().flatten();
    let skip_gitignore = preserved_metadata
        .as_ref()
        .is_some_and(|metadata| metadata.skip_gitignore);
    let indexed_filter_is_current =
        workspace_index_matches_skip_gitignore(workspace, skip_gitignore);
    let index_health = workspace.quick_index_health();
    if index_health.needs_rebuild() {
        rebuild_index_storage(workspace, preserved_metadata.as_ref())?;
    }
    if reset_worktree_overlay {
        clear_worktree_overlay_storage(workspace);
    }

    let tracks_reusable_base_state =
        workspace.repo_id.is_some() && workspace.base_index_dir.is_none();
    let clean_git_state_before = tracks_reusable_base_state
        .then(|| clean_git_checkout_state(&workspace.root))
        .flatten();
    let reusable_index_is_current = clean_git_state_before.as_deref().is_some_and(|state| {
        index_health.is_queryable()
            && !skip_gitignore
            && indexed_filter_is_current
            && fs::read_to_string(indexed_git_state_path(workspace))
                .ok()
                .as_deref()
                == Some(state)
    });
    if reusable_index_is_current
        && clean_git_checkout_state(&workspace.root) == clean_git_state_before
    {
        return Ok(IndexingSummary {
            workspace_id: workspace.id.clone(),
            indexed_files: 0,
            deleted_files: 0,
            total_chunks: count_chunks(&workspace.sqlite_path())?,
            phase_timings: IndexingPhaseTimings {
                discovery_ms: indexing_started.elapsed().as_secs_f64() * 1_000.0,
                ..Default::default()
            },
        });
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
    let (heartbeat_stop_tx, heartbeat_stop_rx) = std::sync::mpsc::channel::<()>();
    let heartbeat_workspace = workspace.clone();
    let heartbeat_handle = std::thread::spawn(move || {
        loop {
            match heartbeat_stop_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let progress =
                        std::fs::read_to_string(heartbeat_workspace.indexing_progress_path())
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
            }
        }
    });

    let result = retry_transient_tantivy_writes(|| {
        index_workspace_inner(
            workspace,
            embedding_model,
            trust_live_watcher,
            watcher_paths,
            skip_gitignore,
        )
    });
    let result = result.and_then(|summary| {
        record_indexed_skip_gitignore(workspace, skip_gitignore)?;
        Ok(summary)
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
    let _ = heartbeat_stop_tx.send(());
    let _ = heartbeat_handle.join();
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
                if is_retryable_tantivy_write_error(&error)
                    && attempt + 1 < TANTIVY_INDEX_RETRY_ATTEMPTS =>
            {
                let delay_ms = TANTIVY_INDEX_RETRY_BASE_DELAY_MS << attempt;
                tracing::warn!(
                    attempt = attempt + 1,
                    delay_ms,
                    "retrying index after retryable Tantivy write error"
                );
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the retry loop always returns on its final attempt")
}

fn is_retryable_tantivy_write_error(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        let Some(error) = source.downcast_ref::<tantivy::TantivyError>() else {
            return false;
        };

        match error {
            tantivy::TantivyError::OpenWriteError(OpenWriteError::IoError { io_error, .. })
            | tantivy::TantivyError::IoError(io_error) => {
                io_error.kind() == std::io::ErrorKind::PermissionDenied
            }
            tantivy::TantivyError::ErrorInThread(message) => {
                message.contains("index writer was killed") && message.contains("io::Error")
            }
            _ => false,
        }
    })
}

fn index_workspace_inner(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
    trust_live_watcher: bool,
    watcher_paths: Option<&[PathBuf]>,
    skip_gitignore: bool,
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
    if trust_live_watcher
        && workspace.is_watcher_alive()
        && workspace_is_indexed(workspace)
        && workspace_index_matches_skip_gitignore(workspace, skip_gitignore)
    {
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
                            skip_gitignore,
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
                return index_workspace_inner(
                    workspace,
                    embedding_model,
                    trust_live_watcher,
                    None,
                    skip_gitignore,
                );
            }
        }

        if (!base_sqlite.exists() || !base_merkle.exists())
            && !workspace.has_overlay()
            && let Some(main_root) = workspace.main_worktree_root()
        {
            eprintln!("  ⚡ base workspace is not indexed, running full base indexing first...");
            let base_workspace = crate::workspace::Workspace::resolve(&main_root)?;
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
                return index_workspace_inner(
                    workspace,
                    embedding_model,
                    trust_live_watcher,
                    None,
                    skip_gitignore,
                );
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
    // Save the Merkle snapshot only after every store commits. An earlier
    // snapshot could claim that files exist in a partial index after a crash.
    let (mut diff, pending_snapshot, clear_overlay_paths) = if let Some(overlay_diff) = overlay_mode
    {
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
            && workspace_index_matches_skip_gitignore(workspace, skip_gitignore)
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

    let pending_snapshot = pending_snapshot.map(Arc::new);
    let current_snapshot = pending_snapshot.clone().or_else(|| {
        MerkleSnapshot::load(&workspace.merkle_snapshot_path())
            .ok()
            .map(Arc::new)
    });
    add_included_file_dependents(
        workspace,
        &mut diff,
        &clear_overlay_paths,
        current_snapshot.as_deref(),
    )?;
    add_file_edge_dependents(
        workspace,
        &mut diff,
        &clear_overlay_paths,
        current_snapshot.as_deref(),
    )?;

    let discovery_ms = index_started.elapsed().as_secs_f64() * 1_000.0;
    let persist_started = std::time::Instant::now();

    // Determine which stores to write to: overlay or main
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    let is_fresh_index = !workspace_is_indexed(workspace);
    let fresh_staging = if !use_overlay && is_fresh_index {
        Some(FreshIndexStaging::create(workspace)?)
    } else {
        None
    };
    let (sqlite_path, tantivy_path, vector_path) = if let Some(staging) = &fresh_staging {
        (
            staging.sqlite_path.clone(),
            staging.tantivy_dir.clone(),
            staging.vector_path.clone(),
        )
    } else if use_overlay {
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

    let defer_secondary_indexes = !use_overlay && is_fresh_index;
    if !use_overlay && fresh_staging.is_none() {
        let preserved_metadata = workspace.read_metadata().ok().flatten();
        if let Err(err) = open_storage_with_options(
            workspace,
            crate::EMBEDDING_DIMENSIONS,
            !defer_secondary_indexes,
        ) {
            tracing::warn!(
                "storage verification failed for {}: {err:#}; rebuilding index storage",
                workspace.root.display()
            );
            rebuild_index_storage(workspace, preserved_metadata.as_ref())?;
            let _ = open_storage_with_options(workspace, crate::EMBEDDING_DIMENSIONS, false)
                .with_context(|| {
                    format!(
                        "failed to reopen index storage after rebuild for {}",
                        workspace.root.display()
                    )
                })?;
        }
    }

    let mut sqlite = Connection::open(&sqlite_path)?;
    if fresh_staging.is_some() {
        apply_fresh_staging_pragmas(&sqlite)?;
    } else {
        apply_bulk_write_pragmas(&sqlite)?;
    }
    create_tables_schema(&sqlite, !defer_secondary_indexes)?;
    if use_overlay {
        create_overlay_tables(&sqlite)?;
    }

    fs::create_dir_all(&tantivy_path)?;
    // Clear stale Tantivy writer lock left by a crash — safe because we
    // already hold the fs2 advisory lock guaranteeing exclusive access.
    let tantivy_lock = tantivy_path.join(".tantivy-writer.lock");
    let _ = fs::remove_file(&tantivy_lock);
    let (tantivy, fields) = open_tantivy_index(&tantivy_path)?;
    let indexed_source_bytes = diff
        .added_or_modified
        .iter()
        .filter_map(|(path, _)| fs::metadata(workspace.root.join(path)).ok())
        .map(|metadata| metadata.len())
        .sum();
    let (writer_threads, writer_memory_budget) = tantivy_writer_settings(indexed_source_bytes);
    tracing::debug!(
        indexed_source_bytes,
        writer_threads,
        writer_memory_budget,
        "configured workload-aware Tantivy writer"
    );
    // Retry with backoff — NFS/overlayfs may delay flock release.
    let mut writer = None;
    for attempt in 0..5u32 {
        match tantivy.writer_with_num_threads(writer_threads, writer_memory_budget) {
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
    let mut vector_tombstones = VectorTombstoneJournals::new(
        workspace.hash_tombstones_path(),
        Some(workspace.neural_tombstones_path()),
    );

    // Periodic commits keep the WAL bounded during large indexes.
    let mut tx = sqlite.transaction()?;
    let mut incremental_stats = if is_fresh_index {
        None
    } else {
        IncrementalStatsDelta::load(&tx)?
    };

    macro_rules! checkpoint_deletion_tombstones {
        () => {
            if vector_tombstones.should_checkpoint() {
                if let Some(stats) = &mut incremental_stats {
                    stats.checkpoint(&tx)?;
                }
                commit_with_vector_tombstones(tx, &mut vector_tombstones)?;
                if !is_fresh_index {
                    writer.commit()?;
                }
                tx = sqlite.transaction()?;
            }
        };
    }

    // Overlay state shadows only paths backed by the base index. Clear paths
    // that have returned to base content or were removed after being overlay-only.
    if use_overlay {
        for rel_path in &clear_overlay_paths {
            let removed_keys = remove_file_chunks(
                &tx,
                &mut writer,
                &fields,
                &mut vector_tombstones,
                rel_path,
                None,
            )?;
            if let Some(stats) = &mut incremental_stats {
                stats.record_removal(rel_path, &removed_keys);
            }
            tx.execute(
                "DELETE FROM tombstones WHERE file_path = ?1",
                params![index_path_string(rel_path)],
            )?;
            checkpoint_deletion_tombstones!();
        }

        for rel_path in &diff.deleted {
            let rel_str = index_path_string(rel_path);
            let removed_keys = remove_file_chunks(
                &tx,
                &mut writer,
                &fields,
                &mut vector_tombstones,
                rel_path,
                None,
            )?;
            if let Some(stats) = &mut incremental_stats {
                stats.record_removal(rel_path, &removed_keys);
            }
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
            checkpoint_deletion_tombstones!();
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
        for rel_path in &diff.deleted {
            let removed_keys = remove_file_chunks(
                &tx,
                &mut writer,
                &fields,
                &mut vector_tombstones,
                rel_path,
                None,
            )?;
            if let Some(stats) = &mut incremental_stats {
                stats.record_removal(rel_path, &removed_keys);
            }
            checkpoint_deletion_tombstones!();
        }
    }

    let total = diff.added_or_modified.len();
    let show_progress = total > 0 && std::io::stderr().is_terminal();

    let t0 = std::time::Instant::now();
    let mut total_chunks_processed = 0;
    let mut indexed_files_with_chunks = 0;
    let mut touched_files = HashSet::new();
    let mut chunks_since_commit = 0;

    // Scanner and chunker run ahead by at most two batches, bounding memory.
    let mut producer = spawn_index_batch_producer(
        workspace,
        &diff,
        current_snapshot.clone(),
        &fields,
        is_fresh_index,
        show_progress,
    );

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

    let mut persist_statements = persist_or_stop!(PersistStatements::prepare(&tx));

    while let Some(file_chunks) = producer.recv() {
        let file_chunks = persist_or_stop!(file_chunks);
        // Persist lexical metadata first. Hash ANN construction is intentionally
        // deferred to background enhancement: on multi-million chunk repos the
        // provisional graph dominated first-index latency and delayed usable
        // BM25/literal results by minutes.
        for indexed_file in file_chunks {
            let rel_path = indexed_file.rel_path;
            let indexed_chunks = indexed_file.chunks;
            let rel_path_string = index_path_string(&rel_path);
            touched_files.insert(rel_path_string.clone());
            indexed_files_with_chunks += usize::from(!indexed_chunks.is_empty());
            total_chunks_processed += indexed_chunks.len();
            chunks_since_commit += indexed_chunks.len();

            if !is_fresh_index {
                let retained_keys = indexed_chunks
                    .iter()
                    .map(|prepared| prepared.chunk.vector_key)
                    .collect::<HashSet<_>>();
                let removed_keys = persist_or_stop!(remove_file_chunks(
                    &tx,
                    &mut writer,
                    &fields,
                    &mut vector_tombstones,
                    &rel_path,
                    Some(&retained_keys),
                ));
                if let Some(stats) = &mut incremental_stats {
                    stats.record_removal(&rel_path, &removed_keys);
                }
            }
            if let Some(stats) = &mut incremental_stats {
                stats.record_insertion(
                    &tx,
                    &rel_path,
                    indexed_chunks
                        .iter()
                        .map(|prepared| prepared.chunk.vector_key),
                )?;
            }

            for included_path in indexed_file.included_paths {
                let included_path = index_path_string(&included_path);
                persist_or_stop!(
                    persist_statements.insert_dependency(&rel_path_string, &included_path)
                );
            }
            for edge in indexed_file.file_edges {
                persist_or_stop!(persist_statements.insert_file_edge(&edge));
            }
            for dependency in indexed_file.unresolved_dependencies {
                persist_or_stop!(persist_statements.insert_unresolved_dependency(&dependency));
            }
            if let Some(signature) = indexed_file.manifest_resolution_signature {
                persist_or_stop!(
                    persist_statements
                        .insert_manifest_resolution_signature(&rel_path_string, &signature)
                );
            }

            // Batch the timestamp syscall per file, not per chunk.
            let now_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            for prepared in indexed_chunks {
                let indexed = &prepared.chunk;
                persist_or_stop!(insert_chunk(
                    &mut persist_statements,
                    indexed,
                    &prepared.compressed_text,
                    &rel_path_string,
                    now_unix
                ));
                persist_or_stop!(writer.add_document(prepared.tantivy_doc));
            }

            // Bound SQLite WAL growth and deletion journal memory. Fresh
            // indexes publish Tantivy once at the end; incremental indexes
            // publish each committed deletion batch.
            if chunks_since_commit >= 25_000 || vector_tombstones.should_checkpoint() {
                persist_or_stop!(persist_statements.flush_symbols());
                drop(persist_statements);
                if let Some(stats) = &mut incremental_stats {
                    persist_or_stop!(stats.checkpoint(&tx));
                }
                persist_or_stop!(commit_with_vector_tombstones(tx, &mut vector_tombstones));
                if !is_fresh_index {
                    persist_or_stop!(writer.commit());
                }
                tx = persist_or_stop!(sqlite.transaction());
                persist_statements = persist_or_stop!(PersistStatements::prepare(&tx));
                chunks_since_commit = 0;
            }
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

    let secondary_indexes_started = Instant::now();
    persist_statements.flush_symbols()?;
    drop(persist_statements);
    if defer_secondary_indexes {
        create_secondary_indexes(&tx)?;
    }
    finalize_graph_indexes(&tx)?;
    let secondary_indexes_ms = secondary_indexes_started.elapsed().as_secs_f64() * 1_000.0;

    // Update cached stats before committing so status reads are O(1).
    let vector_key_count_started = Instant::now();
    let (chunk_count, file_count, vector_key_count) = if let Some(stats) = incremental_stats {
        stats.final_counts(&tx)?
    } else {
        (
            total_chunks_processed as i64,
            indexed_files_with_chunks as i64,
            tx.query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
                row.get(0)
            })?,
        )
    };
    let vector_key_count_ms = vector_key_count_started.elapsed().as_secs_f64() * 1_000.0;
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

    let sqlite_commit_started = Instant::now();
    commit_with_vector_tombstones(tx, &mut vector_tombstones)?;
    let sqlite_commit_ms = sqlite_commit_started.elapsed().as_secs_f64() * 1_000.0;

    let tantivy_commit_started = Instant::now();
    writer.commit()?;
    let tantivy_commit_ms = tantivy_commit_started.elapsed().as_secs_f64() * 1_000.0;
    let tantivy_merge_started = Instant::now();
    writer.wait_merging_threads()?;
    let tantivy_merge_ms = tantivy_merge_started.elapsed().as_secs_f64() * 1_000.0;
    if fresh_staging.is_some() {
        apply_default_write_pragmas(&sqlite)?;
        sqlite.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    }
    drop(tantivy);
    drop(sqlite);

    let staging_publish_started = Instant::now();
    let staging_publish_ms = if let Some(staging) = fresh_staging {
        staging.promote(workspace)?;
        staging_publish_started.elapsed().as_secs_f64() * 1_000.0
    } else {
        0.0
    };

    let metadata_started = Instant::now();
    finalize_workspace_index_state(workspace, pending_snapshot)?;
    let metadata_ms = metadata_started.elapsed().as_secs_f64() * 1_000.0;
    let total_chunks = if use_overlay {
        count_workspace_chunks(workspace)?
    } else {
        chunk_count as usize
    };

    Ok(IndexingSummary {
        workspace_id: workspace.id.clone(),
        indexed_files: touched_files.len(),
        deleted_files: diff.deleted.len(),
        total_chunks,
        phase_timings: IndexingPhaseTimings {
            discovery_ms,
            persist_ms,
            finalize_ms: finalize_started.elapsed().as_secs_f64() * 1_000.0,
            secondary_indexes_ms,
            vector_key_count_ms,
            sqlite_commit_ms,
            tantivy_commit_ms,
            tantivy_merge_ms,
            staging_publish_ms,
            metadata_ms,
        },
    })
}

fn finalize_workspace_index_state(
    workspace: &Workspace,
    pending_snapshot: Option<Arc<MerkleSnapshot>>,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let existing = workspace
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
    workspace.write_metadata(&WorkspaceMetadata {
        id: workspace.id.clone(),
        root: workspace.root.clone(),
        created_at_unix: existing.created_at_unix,
        last_indexed_at_unix: Some(now),
        watch_enabled: existing.watch_enabled,
        skip_gitignore: existing.skip_gitignore,
        // Background vector enhancement uses this lexical generation to
        // detect concurrent edits and resume against the latest commit.
        index_generation: existing.index_generation + 1,
    })?;
    workspace.write_index_format_version()?;

    // Snapshot becomes authoritative only after all stores and metadata commit.
    if let Some(snapshot) = pending_snapshot {
        snapshot.save(&workspace.merkle_snapshot_path())?;
    }
    Ok(())
}

fn add_included_file_dependents(
    workspace: &Workspace,
    diff: &mut MerkleDiff,
    clear_overlay_paths: &[PathBuf],
    current_snapshot: Option<&MerkleSnapshot>,
) -> Result<()> {
    let Some(current_snapshot) = current_snapshot else {
        return Ok(());
    };

    let mut changed_paths = diff
        .added_or_modified
        .iter()
        .map(|(path, _)| index_path_string(path))
        .chain(diff.deleted.iter().map(|path| index_path_string(path)))
        .chain(
            clear_overlay_paths
                .iter()
                .map(|path| index_path_string(path)),
        )
        .collect::<HashSet<_>>();
    if changed_paths.is_empty() {
        return Ok(());
    }

    let mut sqlite_paths = vec![workspace.sqlite_path(), workspace.overlay_sqlite_path()];
    if let Some(base_index_dir) = &workspace.base_index_dir {
        sqlite_paths.push(base_index_dir.join("metadata.sqlite3"));
    }
    sqlite_paths.sort();
    sqlite_paths.dedup();

    let mut owners = HashSet::new();
    for sqlite_path in sqlite_paths {
        if !sqlite_path.is_file() {
            continue;
        }
        let Ok(conn) = Connection::open_with_flags(
            sqlite_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            continue;
        };
        let table_exists = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'included_file_dependencies'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        if !table_exists {
            continue;
        }

        let Ok(mut stmt) = conn.prepare(
            "SELECT owner_path
             FROM included_file_dependencies
             WHERE included_path = ?1",
        ) else {
            continue;
        };
        for changed_path in &changed_paths {
            let Ok(rows) = stmt.query_map(params![changed_path], |row| row.get::<_, String>(0))
            else {
                continue;
            };
            owners.extend(rows.filter_map(Result::ok));
        }
    }

    let deleted = diff
        .deleted
        .iter()
        .map(|path| index_path_string(path))
        .collect::<HashSet<_>>();
    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort();
    for owner in owners {
        if changed_paths.contains(&owner) || deleted.contains(&owner) {
            continue;
        }
        let Some(snapshot_hash) = current_snapshot.files.get(&owner) else {
            continue;
        };
        let owner_path = PathBuf::from(&owner);
        if !workspace.root.join(&owner_path).is_file() {
            continue;
        }
        diff.added_or_modified
            .push((owner_path, snapshot_hash.ends_with("-1")));
        changed_paths.insert(owner);
    }

    Ok(())
}

fn add_file_edge_dependents(
    workspace: &Workspace,
    diff: &mut MerkleDiff,
    clear_overlay_paths: &[PathBuf],
    current_snapshot: Option<&MerkleSnapshot>,
) -> Result<()> {
    let Some(current_snapshot) = current_snapshot else {
        return Ok(());
    };
    if diff.added_or_modified.is_empty()
        && diff.deleted.is_empty()
        && clear_overlay_paths.is_empty()
    {
        return Ok(());
    }

    let restored_from_base = clear_overlay_paths
        .iter()
        .map(|path| index_path_string(path))
        .collect::<HashSet<_>>();
    let changed = diff
        .added_or_modified
        .iter()
        .map(|(path, _)| index_path_string(path))
        .chain(diff.deleted.iter().map(|path| index_path_string(path)))
        .chain(restored_from_base.iter().cloned())
        .collect::<HashSet<_>>();
    let deleted = diff
        .deleted
        .iter()
        .map(|path| index_path_string(path))
        .collect::<HashSet<_>>();
    let added_or_modified = diff
        .added_or_modified
        .iter()
        .map(|(path, _)| index_path_string(path))
        .collect::<HashSet<_>>();
    let current_manifest_signatures = diff
        .added_or_modified
        .iter()
        .map(|(path, _)| path)
        .chain(clear_overlay_paths)
        .filter_map(|path| {
            let content = fs::read_to_string(workspace.root.join(path)).ok()?;
            crate::context_graph::manifest_resolution_signature(path, &content)
                .map(|signature| (index_path_string(path), signature))
        })
        .collect::<HashMap<_, _>>();
    let candidate_lookup_keys = diff
        .added_or_modified
        .iter()
        .map(|(path, _)| path)
        .chain(clear_overlay_paths)
        .flat_map(|path| crate::context_graph::path_lookup_keys(path))
        .collect::<BTreeSet<_>>();

    let base_sqlite_path = workspace
        .base_index_dir
        .as_ref()
        .map(|base_index_dir| base_index_dir.join("metadata.sqlite3"));
    let overlay_shadowed = crate::context_graph::overlay_shadowed_paths(workspace);
    let mut sqlite_paths = vec![workspace.overlay_sqlite_path(), workspace.sqlite_path()];
    if let Some(base_sqlite_path) = &base_sqlite_path {
        sqlite_paths.push(base_sqlite_path.clone());
    }
    let mut seen_sqlite_paths = HashSet::new();
    sqlite_paths.retain(|path| seen_sqlite_paths.insert(path.clone()));

    let mut persisted_paths = HashSet::new();
    let mut old_manifest_signatures = HashMap::new();
    let mut owners = HashSet::new();
    let mut unresolved = BTreeSet::new();
    for sqlite_path in &sqlite_paths {
        if !sqlite_path.is_file() {
            continue;
        }
        let Ok(conn) = Connection::open_with_flags(
            sqlite_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            continue;
        };

        if sqlite_table_exists(&conn, "chunks") {
            let Ok(mut statement) = conn
                .prepare_cached("SELECT EXISTS(SELECT 1 FROM chunks WHERE file_path = ?1 LIMIT 1)")
            else {
                continue;
            };
            let base_index = base_sqlite_path.as_ref() == Some(sqlite_path);
            for path in &added_or_modified {
                if base_index && overlay_shadowed.contains(path) {
                    continue;
                }
                if statement
                    .query_row([path], |row| row.get::<_, bool>(0))
                    .unwrap_or(false)
                {
                    persisted_paths.insert(path.clone());
                }
            }
        }

        if sqlite_table_exists(&conn, "manifest_resolution_signatures") {
            let Ok(mut statement) = conn.prepare_cached(
                "SELECT signature FROM manifest_resolution_signatures WHERE file_path = ?1",
            ) else {
                continue;
            };
            let base_index = base_sqlite_path.as_ref() == Some(sqlite_path);
            for path in current_manifest_signatures.keys() {
                if base_index && overlay_shadowed.contains(path) {
                    continue;
                }
                if let Ok(signature) = statement.query_row([path], |row| row.get::<_, String>(0)) {
                    old_manifest_signatures
                        .entry(path.clone())
                        .or_insert(signature);
                }
            }
        }

        if sqlite_table_exists(&conn, "file_edges") {
            let Ok(mut statement) = conn.prepare_cached(
                "SELECT source_path FROM file_edges
                 WHERE target_path = ?1 AND kind IN (?2, ?3)",
            ) else {
                continue;
            };
            for path in &deleted {
                let Ok(rows) = statement.query_map(
                    params![
                        path,
                        crate::context_graph::FileEdgeKind::Dependency as i64,
                        crate::context_graph::FileEdgeKind::Config as i64,
                    ],
                    |row| row.get::<_, String>(0),
                ) else {
                    continue;
                };
                owners.extend(rows.filter_map(Result::ok));
            }
        }

        if sqlite_table_exists(&conn, "unresolved_file_dependencies") {
            let Ok(mut statement) = conn.prepare_cached(
                "SELECT source_path, language, spec
                 FROM unresolved_file_dependencies
                 WHERE lookup_key = ?1
                 ORDER BY source_path, language, spec",
            ) else {
                continue;
            };
            for lookup_key in &candidate_lookup_keys {
                let Ok(rows) = statement.query_map([lookup_key], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                }) else {
                    continue;
                };
                unresolved.extend(rows.filter_map(Result::ok));
            }
        }
    }

    let changed_resolution_manifests = current_manifest_signatures
        .iter()
        .filter(|(path, signature)| old_manifest_signatures.get(*path) != Some(*signature))
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    if !changed_resolution_manifests.is_empty() {
        for sqlite_path in &sqlite_paths {
            if !sqlite_path.is_file() {
                continue;
            }
            let Ok(conn) = Connection::open_with_flags(
                sqlite_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ) else {
                continue;
            };
            if !sqlite_table_exists(&conn, "file_edges") {
                continue;
            }
            let Ok(mut statement) = conn.prepare_cached(
                "SELECT source_path FROM file_edges WHERE target_path = ?1 AND kind = ?2",
            ) else {
                continue;
            };
            let base_index = base_sqlite_path.as_ref() == Some(sqlite_path);
            for path in &changed_resolution_manifests {
                if base_index && overlay_shadowed.contains(*path) {
                    continue;
                }
                let Ok(rows) = statement.query_map(
                    params![path, crate::context_graph::FileEdgeKind::Config as i64],
                    |row| row.get::<_, String>(0),
                ) else {
                    continue;
                };
                owners.extend(rows.filter_map(Result::ok));
            }
        }
    }

    let new_targets = added_or_modified
        .difference(&persisted_paths)
        .chain(restored_from_base.iter())
        .cloned()
        .collect::<HashSet<_>>();
    if !new_targets.is_empty() {
        for (source, language, spec) in unresolved {
            if changed.contains(&source) || deleted.contains(&source) {
                continue;
            }
            let Some(target) = crate::context_graph::resolve_dependency_spec(
                &workspace.root,
                Some(current_snapshot),
                Path::new(&source),
                &language,
                &spec,
            ) else {
                continue;
            };
            if new_targets.contains(&index_path_string(&target)) {
                owners.insert(source);
            }
        }
    }
    let added_manifests = new_targets
        .iter()
        .filter(|path| crate::context_graph::is_manifest_path(Path::new(path)))
        .map(PathBuf::from)
        .collect::<HashSet<_>>();
    if !added_manifests.is_empty() {
        for source in current_snapshot.files.keys() {
            if changed.contains(source) || deleted.contains(source) {
                continue;
            }
            if crate::context_graph::configuration_target(
                &workspace.root,
                Some(current_snapshot),
                Path::new(source),
            )
            .is_some_and(|manifest| added_manifests.contains(&manifest))
            {
                owners.insert(source.clone());
            }
        }
    }

    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort();
    for owner in owners {
        if changed.contains(&owner) || deleted.contains(&owner) {
            continue;
        }
        let Some(snapshot_hash) = current_snapshot.files.get(&owner) else {
            continue;
        };
        let owner_path = PathBuf::from(&owner);
        if !workspace.root.join(&owner_path).is_file() {
            continue;
        }
        diff.added_or_modified
            .push((owner_path, snapshot_hash.ends_with("-1")));
    }

    Ok(())
}

fn sqlite_table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
        )",
        [table],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn load_rust_doc_includes(
    root: &Path,
    owner_rel_path: &Path,
    includes: &[RustDocInclude],
    current_snapshot: Option<&MerkleSnapshot>,
) -> (Vec<Chunk>, Vec<PathBuf>) {
    let canonical_root = fs::canonicalize(root).ok();
    let mut chunks = Vec::new();
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();
    let mut total_bytes = 0u64;

    for include in includes.iter().take(MAX_RUST_DOC_INCLUDES_PER_SOURCE) {
        let Some(included_rel_path) =
            normalize_workspace_relative_include(owner_rel_path, &include.path)
        else {
            continue;
        };
        if included_rel_path == owner_rel_path || !seen.insert(included_rel_path.clone()) {
            continue;
        }
        dependencies.push(included_rel_path.clone());

        let included_key = index_path_string(&included_rel_path);
        if !current_snapshot.is_some_and(|snapshot| snapshot.files.contains_key(&included_key)) {
            continue;
        }
        if chunks.len() >= MAX_RUST_DOC_INCLUDE_CHUNKS_PER_SOURCE {
            continue;
        }

        let Some(canonical_root) = canonical_root.as_ref() else {
            continue;
        };
        let Ok(canonical_target) = fs::canonicalize(root.join(&included_rel_path)) else {
            continue;
        };
        if !canonical_target.starts_with(canonical_root) {
            continue;
        }
        let Ok(metadata) = canonical_target.metadata() else {
            continue;
        };
        if !metadata.is_file()
            || metadata.len() > MAX_RUST_DOC_INCLUDE_BYTES
            || total_bytes.saturating_add(metadata.len()) > MAX_RUST_DOC_INCLUDE_TOTAL_BYTES
        {
            continue;
        }
        let Ok(bytes) = fs::read(&canonical_target) else {
            continue;
        };
        if !is_indexable_file(&included_rel_path, &bytes) {
            continue;
        }
        total_bytes += bytes.len() as u64;
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|err| String::from_utf8_lossy(&err.into_bytes()).into_owned());
        let remaining = MAX_RUST_DOC_INCLUDE_CHUNKS_PER_SOURCE.saturating_sub(chunks.len());
        chunks.extend(
            chunk_rust_doc_include(
                owner_rel_path,
                include.source_line,
                &included_rel_path,
                &text,
            )
            .into_iter()
            .take(remaining),
        );
    }

    (chunks, dependencies)
}

fn normalize_workspace_relative_include(owner_rel_path: &Path, include: &Path) -> Option<PathBuf> {
    if include.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in owner_rel_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
    {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    for component in include.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!normalized.as_os_str().is_empty()).then_some(normalized)
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

fn vector_store_covers_all_keys(
    sqlite: &Connection,
    vector_index: &VectorStore,
    total_chunks: usize,
) -> Result<bool> {
    if vector_index.size() != total_chunks {
        return Ok(false);
    }

    let mut stmt = sqlite.prepare("SELECT DISTINCT vector_key FROM chunks")?;
    let rows = stmt.query_map([], |row| Ok(row.get::<_, i64>(0)? as u64))?;
    for row in rows {
        if !vector_index.contains(row?) {
            return Ok(false);
        }
    }
    Ok(true)
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
        crate::vector_store::VectorTier::Hash,
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

    let scan_sql = if vector_store_covers_all_keys(&sqlite, &vector_index, total_chunks)? {
        "SELECT vector_key, text FROM chunks WHERE 0"
    } else {
        "SELECT vector_key, text FROM chunks"
    };
    let mut stmt = sqlite.prepare(scan_sql)?;
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

        let text = try_decompress_text(raw)
            .with_context(|| format!("failed to read stored text for vector key {key}"))?;
        batch.push((key, text));
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
    if total_chunks == 0 && removed_tombstones {
        VectorStore::reset(
            &vector_path,
            hash_model.dimensions(),
            HASH_VECTOR_QUANTIZATION,
            crate::vector_store::VectorTier::Hash,
        )?;
    } else if newly_processed > 0 || removed_tombstones {
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
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    let sqlite_path = if use_overlay {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    if !sqlite_path.exists() {
        return Ok(0);
    }

    if use_overlay
        && let Some(base_dir) = &workspace.base_index_dir
        && let Ok(raw) = fs::read(base_dir.join("neural_model.json"))
        && let Ok(base_identity) =
            serde_json::from_slice::<crate::embedding::NeuralModelIdentity>(&raw)
        && let Some(active_identity) = neural_model.model_identity()
    {
        anyhow::ensure!(
            base_identity == *active_identity,
            "worktree neural model does not match the base workspace"
        );
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
        crate::vector_store::remove_store_files(&workspace.vector_neural_path());
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

    let sqlite = open_sqlite(&sqlite_path)?;

    // Phase 1: Collect all vector_keys to determine which still need embedding.
    // This avoids decompressing text for the ~31% already done.
    let total_chunks: usize = sqlite
        .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0) as usize;

    let vector_path = workspace.vector_neural_path();
    let vector_store_existed = vector_path.exists();
    let mut vector_index = VectorStore::open(
        &vector_path,
        neural_model.dimensions(),
        NEURAL_VECTOR_QUANTIZATION,
        crate::vector_store::VectorTier::Neural,
    )?;
    let claimed_tombstones = claim_vector_tombstones(
        &workspace.neural_tombstones_path(),
        &workspace.neural_tombstones_processing_path(),
    )?;
    let removed_tombstones = claimed_tombstones.is_some();
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
    // Re-check the accelerator batch size at each batch boundary so a CUDA GPU
    // that becomes busy with another workload backs off during long enhancement.
    let initial_batch_size = neural_enhance_batch_size(neural_model);
    let mut current_batch_size = initial_batch_size;
    let mut last_batch_size_refresh = Instant::now();
    let mut batch: Vec<(u64, String)> = Vec::with_capacity(initial_batch_size);
    let mut batch_keys = HashSet::with_capacity(initial_batch_size);

    // A nearly-complete resume should not pull every compressed text blob
    // through SQLite only to discard it after a vector-store membership check.
    // Discover missing keys from the covering index, then point-fetch text for
    // those keys. Keep the sequential scan for fresh or incomplete stores,
    // where indexed point lookups cost more than one table pass.
    let sparse_missing_keys =
        if total_chunks > 0 && existing.saturating_mul(4) >= total_chunks.saturating_mul(3) {
            let mut missing = Vec::with_capacity(remaining);
            let mut stmt = sqlite.prepare("SELECT DISTINCT vector_key FROM chunks")?;
            let rows = stmt.query_map([], |row| Ok(row.get::<_, i64>(0)? as u64))?;
            for row in rows {
                let key = row?;
                if !vector_index.contains(key) {
                    missing.push(key);
                }
            }
            Some(missing)
        } else {
            None
        };

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

    {
        let mut process_row = |key: u64, raw: Vec<u8>| -> Result<()> {
            if vector_index.contains(key) || !batch_keys.insert(key) {
                return Ok(());
            }

            let text = try_decompress_text(raw)
                .with_context(|| format!("failed to read stored text for vector key {key}"))?;
            batch.push((key, text));

            if batch.len() >= current_batch_size {
                while neural_model.respects_system_constraints()
                    && let Some(reason) = check_system_constraints()
                {
                    let _ = std::fs::write(&paused_path, &reason);
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
                let _ = std::fs::remove_file(&paused_path);

                let processed_len = batch.len();
                process_batch(&mut batch, &mut newly_processed, &mut vector_index)?;
                batch_keys.clear();
                progress_count += processed_len;
                let _ = std::fs::write(&progress_path, progress_count.to_string());
                if last_batch_size_refresh.elapsed() >= NEURAL_BATCH_SIZE_REFRESH_INTERVAL {
                    current_batch_size = neural_enhance_batch_size(neural_model);
                    last_batch_size_refresh = Instant::now();
                }

                if newly_processed.is_multiple_of(16_384) {
                    vector_index.save()?;
                }
            }
            Ok(())
        };

        if let Some(missing_keys) = sparse_missing_keys {
            let mut stmt =
                sqlite.prepare("SELECT text FROM chunks WHERE vector_key = ?1 LIMIT 1")?;
            for key in missing_keys {
                let raw = stmt.query_row(params![key as i64], |row| row.get::<_, Vec<u8>>(0))?;
                process_row(key, raw)?;
            }
        } else {
            let mut stmt = sqlite.prepare("SELECT vector_key, text FROM chunks")?;
            let rows = stmt.query_map([], |row| {
                let key = row.get::<_, i64>(0)? as u64;
                let raw: Vec<u8> = row.get(1)?;
                Ok((key, raw))
            })?;
            for row in rows {
                let (key, raw) = row?;
                process_row(key, raw)?;
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
    if total_chunks == 0 && removed_tombstones {
        VectorStore::reset(
            &vector_path,
            neural_model.dimensions(),
            NEURAL_VECTOR_QUANTIZATION,
            crate::vector_store::VectorTier::Neural,
        )?;
    } else if newly_processed > 0 || removed_tombstones || !vector_store_existed {
        vector_index.save()?;
    }
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
    build_indexed_chunk_with_occurrence(chunk, is_ignored, 0)
}

fn build_indexed_chunk_with_occurrence(
    chunk: Chunk,
    is_ignored: bool,
    occurrence: usize,
) -> IndexedChunk {
    // Source bounds are presentation metadata, not embedding identity: adding
    // lines above an unchanged definition must not invalidate its vector.
    let vector_key = vector_key_for_chunk(&chunk.file_path, &chunk.text, occurrence);
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

fn vector_key_for_chunk(file_path: &Path, text: &str, occurrence: usize) -> u64 {
    let mut key_data = Vec::with_capacity(text.len() + 64);
    key_data.extend_from_slice(index_path_string(file_path).as_bytes());
    key_data.extend_from_slice(&occurrence.to_le_bytes());
    key_data.extend_from_slice(text.as_bytes());
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

fn remove_file_chunks(
    sqlite: &Connection,
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    vector_tombstones: &mut VectorTombstoneJournals,
    rel_path: &Path,
    retained_keys: Option<&HashSet<u64>>,
) -> Result<Vec<u64>> {
    let rel_str = index_path_string(rel_path);
    let keys = chunk_vector_keys_for_file(sqlite, &rel_str)?;

    writer.delete_term(Term::from_field_text(fields.file_path, &rel_str));

    if let Some(retained_keys) = retained_keys {
        let obsolete_keys = keys
            .iter()
            .copied()
            .filter(|key| !retained_keys.contains(key))
            .collect::<Vec<_>>();
        vector_tombstones.record(&obsolete_keys);
    } else {
        vector_tombstones.record(&keys);
    }

    crate::symbols::remove_file_graph(sqlite, &rel_str)?;
    sqlite.execute("DELETE FROM chunks WHERE file_path = ?1", params![rel_str])?;
    sqlite.execute(
        "DELETE FROM included_file_dependencies WHERE owner_path = ?1",
        params![rel_str],
    )?;
    sqlite.execute(
        "DELETE FROM file_edges WHERE source_path = ?1",
        params![rel_str],
    )?;
    sqlite.execute(
        "DELETE FROM file_edges WHERE target_path = ?1 AND kind = ?2",
        params![rel_str, crate::context_graph::FileEdgeKind::Test as i64],
    )?;
    sqlite.execute(
        "DELETE FROM unresolved_file_dependencies WHERE source_path = ?1",
        params![rel_str],
    )?;
    sqlite.execute(
        "DELETE FROM manifest_resolution_signatures WHERE file_path = ?1",
        params![rel_str],
    )?;
    Ok(keys)
}

const MAX_TRACKED_INCREMENTAL_VECTOR_KEYS: usize = 512;

struct IncrementalStatsDelta {
    base_chunk_count: i64,
    base_file_count: i64,
    base_vector_key_count: i64,
    chunk_delta: i64,
    initial_file_presence: HashMap<String, bool>,
    initial_vector_key_presence: HashMap<u64, bool>,
    vector_key_count_requires_full_scan: bool,
}

impl IncrementalStatsDelta {
    fn load(conn: &Connection) -> Result<Option<Self>> {
        let read = |key: &str| {
            conn.query_row(
                "SELECT value FROM _stats WHERE key = ?1",
                params![key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
        };
        let Some(base_chunk_count) = read("chunk_count")? else {
            return Ok(None);
        };
        let Some(base_file_count) = read("file_count")? else {
            return Ok(None);
        };
        let Some(base_vector_key_count) = read("vector_key_count")? else {
            return Ok(None);
        };
        Ok(Some(Self {
            base_chunk_count,
            base_file_count,
            base_vector_key_count,
            chunk_delta: 0,
            initial_file_presence: HashMap::new(),
            initial_vector_key_presence: HashMap::new(),
            vector_key_count_requires_full_scan: false,
        }))
    }

    fn record_removal(&mut self, rel_path: &Path, vector_keys: &[u64]) {
        self.initial_file_presence
            .entry(index_path_string(rel_path))
            .or_insert(!vector_keys.is_empty());
        self.chunk_delta -= vector_keys.len() as i64;
        for key in vector_keys {
            if self.vector_key_count_requires_full_scan
                || self.initial_vector_key_presence.contains_key(key)
            {
                continue;
            }
            if self.initial_vector_key_presence.len() >= MAX_TRACKED_INCREMENTAL_VECTOR_KEYS {
                self.initial_vector_key_presence.clear();
                self.vector_key_count_requires_full_scan = true;
                continue;
            }
            self.initial_vector_key_presence.insert(*key, true);
        }
    }

    fn record_insertion(
        &mut self,
        conn: &Connection,
        rel_path: &Path,
        vector_keys: impl IntoIterator<Item = u64>,
    ) -> Result<()> {
        self.initial_file_presence
            .entry(index_path_string(rel_path))
            .or_insert(false);
        let mut chunk_count = 0;
        for key in vector_keys {
            chunk_count += 1;
            if self.vector_key_count_requires_full_scan
                || self.initial_vector_key_presence.contains_key(&key)
            {
                continue;
            }
            if self.initial_vector_key_presence.len() >= MAX_TRACKED_INCREMENTAL_VECTOR_KEYS {
                self.initial_vector_key_presence.clear();
                self.vector_key_count_requires_full_scan = true;
                continue;
            }
            let present = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM chunks WHERE vector_key = ?1)",
                params![key as i64],
                |row| row.get(0),
            )?;
            self.initial_vector_key_presence.insert(key, present);
        }
        self.chunk_delta += chunk_count;
        Ok(())
    }

    fn chunk_and_file_counts(&self, conn: &Connection) -> Result<(i64, i64)> {
        let mut file_delta = 0;
        for (path, initial) in &self.initial_file_presence {
            let present: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM chunks WHERE file_path = ?1)",
                params![path],
                |row| row.get(0),
            )?;
            file_delta += present as i64 - *initial as i64;
        }

        Ok((
            self.base_chunk_count + self.chunk_delta,
            self.base_file_count + file_delta,
        ))
    }

    fn checkpoint(&mut self, conn: &Connection) -> Result<()> {
        let (chunk_count, file_count) = self.chunk_and_file_counts(conn)?;
        let vector_key_count = self.vector_key_count(conn)?;
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('chunk_count', ?1)",
            params![chunk_count],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('file_count', ?1)",
            params![file_count],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO _stats (key, value) VALUES ('vector_key_count', ?1)",
            params![vector_key_count],
        )?;
        self.base_chunk_count = chunk_count;
        self.base_file_count = file_count;
        self.base_vector_key_count = vector_key_count;
        self.chunk_delta = 0;
        self.initial_file_presence.clear();
        self.initial_vector_key_presence.clear();
        self.vector_key_count_requires_full_scan = false;
        Ok(())
    }

    fn vector_key_count(&self, conn: &Connection) -> Result<i64> {
        if self.vector_key_count_requires_full_scan {
            return conn
                .query_row("SELECT COUNT(DISTINCT vector_key) FROM chunks", [], |row| {
                    row.get(0)
                })
                .map_err(Into::into);
        }
        let mut vector_key_delta = 0;
        for (key, initial) in &self.initial_vector_key_presence {
            let present: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM chunks WHERE vector_key = ?1)",
                params![*key as i64],
                |row| row.get(0),
            )?;
            vector_key_delta += present as i64 - *initial as i64;
        }
        Ok(self.base_vector_key_count + vector_key_delta)
    }

    fn final_counts(self, conn: &Connection) -> Result<(i64, i64, i64)> {
        let (chunk_count, file_count) = self.chunk_and_file_counts(conn)?;
        let vector_key_count = self.vector_key_count(conn)?;

        Ok((chunk_count, file_count, vector_key_count))
    }
}

/// Stale vector keys collected for one SQLite transaction. The matching
/// journals are synced immediately before that transaction commits.
struct VectorTombstoneJournals {
    hash_path: PathBuf,
    neural_path: Option<PathBuf>,
    pending_payload: Vec<u8>,
    max_pending_bytes: usize,
}

impl VectorTombstoneJournals {
    fn new(hash_path: PathBuf, neural_path: Option<PathBuf>) -> Self {
        Self::with_max_pending_bytes(
            hash_path,
            neural_path,
            MAX_VECTOR_TOMBSTONE_TRANSACTION_BYTES,
        )
    }

    fn with_max_pending_bytes(
        hash_path: PathBuf,
        neural_path: Option<PathBuf>,
        max_pending_bytes: usize,
    ) -> Self {
        Self {
            hash_path,
            neural_path,
            pending_payload: Vec::new(),
            max_pending_bytes: max_pending_bytes.max(1),
        }
    }

    fn record(&mut self, keys: &[u64]) {
        for key in keys {
            writeln!(&mut self.pending_payload, "{key}")
                .expect("writing vector tombstones to memory cannot fail");
        }
    }

    fn should_checkpoint(&self) -> bool {
        self.pending_payload.len() >= self.max_pending_bytes
    }

    fn flush(&mut self) -> Result<()> {
        if self.pending_payload.is_empty() {
            return Ok(());
        }

        append_and_sync_vector_tombstones(&self.hash_path, &self.pending_payload)?;
        if let Some(path) = &self.neural_path {
            append_and_sync_vector_tombstones(path, &self.pending_payload)?;
        }
        self.pending_payload.clear();
        Ok(())
    }
}

fn append_and_sync_vector_tombstones(path: &Path, payload: &[u8]) -> Result<()> {
    if payload.is_empty() {
        return Ok(());
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(payload)?;
    file.sync_data()?;
    Ok(())
}

fn commit_with_vector_tombstones(
    tx: rusqlite::Transaction<'_>,
    vector_tombstones: &mut VectorTombstoneJournals,
) -> Result<()> {
    // Flush first so a crash can cause extra repair but cannot commit stale vectors.
    vector_tombstones.flush()?;
    tx.commit()?;
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
    if matches!(chunk.kind.as_str(), "Documentation" | "documentation") {
        return chunk
            .text
            .lines()
            .skip(2)
            .map(str::trim)
            .map(|line| line.trim_start_matches('#').trim())
            .filter(|line| !line.is_empty() && !line.starts_with("```"))
            .take(12)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(512)
            .collect();
    }

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
    first_code_line_range(&chunk.text)
        .map(|range| chunk.text[range].to_string())
        .unwrap_or_default()
}

fn build_chunk_doc(
    fields: &TantivyFields,
    chunk: &IndexedChunk,
    file_path: &str,
) -> TantivyDocument {
    let signature = fields
        .signature
        .map(|_| extract_signature(chunk))
        .unwrap_or_default();
    let serialized_capacity = chunk
        .text
        .len()
        .saturating_add(file_path.len().saturating_mul(2))
        .saturating_add(chunk.language.len())
        .saturating_add(chunk.kind.len())
        .saturating_add(signature.len())
        .saturating_add(128);
    let mut doc = TantivyDocument::with_capacity(serialized_capacity);
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
    if let Some(f) = fields.signature
        && !signature.is_empty()
    {
        doc.add_text(f, signature);
    }
    doc
}

fn insert_chunk(
    statements: &mut PersistStatements<'_>,
    chunk: &IndexedChunk,
    compressed_text: &[u8],
    file_path: &str,
    now_unix: i64,
) -> Result<()> {
    let is_ignored_int = if chunk.is_ignored { 1i64 } else { 0i64 };
    statements.chunk_insert.execute(params![
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
    statements.queue_symbols(chunk, statements.conn.last_insert_rowid())?;
    Ok(())
}

struct PersistStatements<'conn> {
    conn: &'conn Connection,
    chunk_insert: Statement<'conn>,
    dependency_insert: Statement<'conn>,
    file_edge_insert: Statement<'conn>,
    unresolved_dependency_insert: Statement<'conn>,
    manifest_resolution_signature_insert: Statement<'conn>,
    symbol_rows: Vec<(String, i64)>,
    symbol_insert_sql: String,
}

impl<'conn> PersistStatements<'conn> {
    fn prepare(conn: &'conn Connection) -> Result<Self> {
        Ok(Self {
            conn,
            chunk_insert: conn.prepare(
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
            )?,
            dependency_insert: conn.prepare(
                "INSERT OR IGNORE INTO included_file_dependencies (
                    owner_path, included_path
                ) VALUES (?1, ?2)",
            )?,
            file_edge_insert: conn.prepare(
                "INSERT OR IGNORE INTO file_edges (
                    source_path, target_path, kind
                ) VALUES (?1, ?2, ?3)",
            )?,
            unresolved_dependency_insert: conn.prepare(
                "INSERT OR IGNORE INTO unresolved_file_dependencies (
                    source_path, language, spec, lookup_key
                ) VALUES (?1, ?2, ?3, ?4)",
            )?,
            manifest_resolution_signature_insert: conn.prepare(
                "INSERT OR REPLACE INTO manifest_resolution_signatures (
                    file_path, signature
                ) VALUES (?1, ?2)",
            )?,
            symbol_rows: Vec::with_capacity(SYMBOL_INSERT_BATCH_ROWS),
            symbol_insert_sql: String::with_capacity(
                "INSERT OR REPLACE INTO symbols (normalized_name, chunk_key) VALUES ".len()
                    + SYMBOL_INSERT_BATCH_ROWS * "(?, ?),".len(),
            ),
        })
    }

    fn insert_dependency(&mut self, owner_path: &str, included_path: &str) -> Result<()> {
        self.dependency_insert
            .execute(params![owner_path, included_path])?;
        Ok(())
    }

    fn insert_file_edge(&mut self, edge: &crate::context_graph::FileEdge) -> Result<()> {
        crate::context_graph::persist_file_edge(&mut self.file_edge_insert, edge)
    }

    fn insert_unresolved_dependency(
        &mut self,
        dependency: &crate::context_graph::UnresolvedDependency,
    ) -> Result<()> {
        self.unresolved_dependency_insert.execute(params![
            index_path_string(&dependency.source_path),
            &dependency.language,
            &dependency.spec,
            &dependency.lookup_key,
        ])?;
        Ok(())
    }

    fn insert_manifest_resolution_signature(
        &mut self,
        file_path: &str,
        signature: &str,
    ) -> Result<()> {
        self.manifest_resolution_signature_insert
            .execute(params![file_path, signature])?;
        Ok(())
    }

    fn queue_symbols(&mut self, chunk: &IndexedChunk, chunk_key: i64) -> Result<()> {
        crate::symbols::append_chunk_definition_rows(chunk, chunk_key, &mut self.symbol_rows);
        if self.symbol_rows.len() >= SYMBOL_INSERT_BATCH_ROWS {
            self.flush_symbols()?;
        }
        Ok(())
    }

    fn flush_symbols(&mut self) -> Result<()> {
        while !self.symbol_rows.is_empty() {
            let batch_len = self.symbol_rows.len().min(SYMBOL_INSERT_BATCH_ROWS);
            self.symbol_insert_sql.clear();
            self.symbol_insert_sql
                .push_str("INSERT OR REPLACE INTO symbols (normalized_name, chunk_key) VALUES ");
            for index in 0..batch_len {
                if index > 0 {
                    self.symbol_insert_sql.push(',');
                }
                self.symbol_insert_sql.push_str("(?, ?)");
            }

            {
                let mut params: Vec<&dyn ToSql> = Vec::with_capacity(batch_len * 2);
                for (name, chunk_key) in &self.symbol_rows[..batch_len] {
                    params.push(name);
                    params.push(chunk_key);
                }
                self.conn
                    .execute(&self.symbol_insert_sql, params.as_slice())?;
            }
            self.symbol_rows.drain(..batch_len);
        }
        Ok(())
    }
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
        // CLI size reporting is approximate, so overlay tombstones are not subtracted.
    }
    Ok(count)
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
            text: try_decompress_text(raw_text).with_context(|| {
                format!("failed to read stored text for vector key {vector_key}")
            })?,
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
    fetch_chunks_by_vector_keys_batch_impl(conn, keys, true)
}

/// Batch-fetch only stored chunk text by vector key.
///
/// Reranking already has candidate metadata from Tantivy or the metadata-only
/// ANN hydration pass. Avoid selecting and rebuilding that metadata a second
/// time when only the compressed text blob is needed.
pub fn fetch_chunk_texts_by_vector_keys_batch(
    conn: &Connection,
    keys: &[u64],
) -> Result<HashMap<u64, String>> {
    let mut result = HashMap::with_capacity(keys.len());
    if keys.is_empty() {
        return Ok(result);
    }

    // SQLite supports up to 999 bind parameters; batch in groups of 500.
    let mut query = String::new();
    for batch in keys.chunks(500) {
        query.clear();
        query.push_str("SELECT vector_key, text FROM chunks WHERE vector_key IN (");
        push_sql_placeholders(&mut query, batch.len());
        query.push(')');
        let mut stmt = conn.prepare_cached(&query)?;

        let mut rows = stmt.query(params_from_iter(batch.iter().map(|key| *key as i64)))?;
        while let Some(row) = rows.next()? {
            let vector_key = row.get::<_, i64>(0)? as u64;
            let raw_text: Vec<u8> = row.get(1)?;
            let text = try_decompress_text(raw_text).with_context(|| {
                format!("failed to read stored text for vector key {vector_key}")
            })?;
            result.insert(vector_key, text);
        }
    }

    Ok(result)
}

fn push_sql_placeholders(query: &mut String, count: usize) {
    for idx in 0..count {
        if idx > 0 {
            query.push(',');
        }
        query.push('?');
    }
}

/// Batch-fetch chunk metadata without reading or decompressing stored text.
///
/// Search fusion only needs text for its bounded rerank set, so ANN candidate
/// discovery can defer the larger blob work until those candidates are known.
pub fn fetch_chunk_metadata_by_vector_keys_batch(
    conn: &Connection,
    keys: &[u64],
) -> Result<HashMap<u64, IndexedChunk>> {
    fetch_chunks_by_vector_keys_batch_impl(conn, keys, false)
}

fn fetch_chunks_by_vector_keys_batch_impl(
    conn: &Connection,
    keys: &[u64],
    include_text: bool,
) -> Result<HashMap<u64, IndexedChunk>> {
    let mut result = HashMap::with_capacity(keys.len());
    if keys.is_empty() {
        return Ok(result);
    }

    // SQLite supports up to 999 bind parameters; batch in groups of 500.
    let mut query = String::new();
    let text_column = if include_text { "text" } else { "x''" };
    for batch in keys.chunks(500) {
        query.clear();
        query.push_str("SELECT file_path, start_line, end_line, language, kind, ");
        query.push_str(text_column);
        query.push_str(", vector_key, is_ignored FROM chunks WHERE vector_key IN (");
        push_sql_placeholders(&mut query, batch.len());
        query.push(')');
        let mut stmt = conn.prepare_cached(&query)?;

        let mut rows = stmt.query(params_from_iter(batch.iter().map(|key| *key as i64)))?;
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
                text: if include_text {
                    try_decompress_text(raw_text).with_context(|| {
                        format!("failed to read stored text for vector key {vector_key}")
                    })?
                } else {
                    String::new()
                },
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
    use std::collections::{BTreeMap, HashSet};
    use std::fs;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::chunking::{Chunk, ChunkKind};
    use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
    use crate::vector_store::ScalarKind;
    use crate::workspace::Workspace;

    use super::*;

    fn indexed_texts_for_file(workspace: &Workspace, file_path: &str) -> Vec<String> {
        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT text FROM chunks WHERE file_path = ?1 ORDER BY start_line, chunk_key")
            .unwrap();
        stmt.query_map(params![file_path], |row| row.get::<_, Vec<u8>>(0))
            .unwrap()
            .map(|row| decompress_text(row.unwrap()))
            .collect()
    }

    #[test]
    #[serial]
    fn fresh_index_reports_individual_finalization_phases() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        fs::write(root.path().join("lib.rs"), "pub fn profiled() {}\n").unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let summary = index_workspace(&workspace, &model).unwrap();
        let timings = summary.phase_timings;

        assert!(timings.finalize_ms > 0.0);
        assert!(timings.secondary_indexes_ms > 0.0);
        assert!(timings.vector_key_count_ms > 0.0);
        assert!(timings.sqlite_commit_ms > 0.0);
        assert!(timings.tantivy_commit_ms > 0.0);
        assert!(timings.staging_publish_ms > 0.0);
        assert!(timings.metadata_ms > 0.0);
        assert!(timings.finalize_ms >= timings.tantivy_merge_ms);

        let legacy: IndexingPhaseTimings =
            serde_json::from_str(r#"{"discovery_ms":1.0,"persist_ms":2.0,"finalize_ms":3.0}"#)
                .unwrap();
        assert_eq!(legacy.vector_key_count_ms, 0.0);
    }

    #[test]
    #[serial]
    fn index_workspace_persists_large_rust_source_file_chunks() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chunking.rs"),
            root.path().join("src/chunking.rs"),
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let source = fs::read_to_string(root.path().join("src/chunking.rs")).unwrap();
        let direct_chunks =
            crate::chunking::chunk_source(Path::new("src/chunking.rs"), &source).len();
        assert!(direct_chunks > 0, "direct chunker should produce chunks");
        assert!(
            crate::chunking::is_indexable_file(Path::new("src/chunking.rs"), source.as_bytes()),
            "large Rust source file should pass indexability gate"
        );

        index_workspace(&workspace, &model).unwrap();

        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_path = 'src/chunking.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 0, "large Rust source file should be persisted");
    }

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
    fn persist_statements_flushes_batched_symbols() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_tables(&conn).unwrap();

        let definitions = (0..(SYMBOL_INSERT_BATCH_ROWS + 3))
            .map(|index| format!("pub fn batched_symbol_{index}() {{}}\n"))
            .collect::<String>();
        let big_chunk = IndexedChunk {
            chunk_id: String::new(),
            file_path: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: SYMBOL_INSERT_BATCH_ROWS + 3,
            language: "rust".to_string(),
            kind: "Module".to_string(),
            text: definitions,
            content_hash: "big".to_string(),
            vector_key: 1,
            is_ignored: false,
        };
        let tail_chunk = IndexedChunk {
            chunk_id: String::new(),
            file_path: PathBuf::from("src/tail.rs"),
            start_line: 1,
            end_line: 1,
            language: "rust".to_string(),
            kind: "Function".to_string(),
            text: "pub fn tail_symbol() {}".to_string(),
            content_hash: "tail".to_string(),
            vector_key: 2,
            is_ignored: false,
        };

        let tx = conn.transaction().unwrap();
        let mut statements = PersistStatements::prepare(&tx).unwrap();
        insert_chunk(
            &mut statements,
            &big_chunk,
            big_chunk.text.as_bytes(),
            "src/lib.rs",
            1,
        )
        .unwrap();
        insert_chunk(
            &mut statements,
            &tail_chunk,
            tail_chunk.text.as_bytes(),
            "src/tail.rs",
            1,
        )
        .unwrap();
        statements.flush_symbols().unwrap();
        drop(statements);
        tx.commit().unwrap();

        let count = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(count, (SYMBOL_INSERT_BATCH_ROWS + 4) as i64);
        let tail_count = conn
            .query_row(
                "SELECT COUNT(*) FROM symbols WHERE normalized_name = 'tail_symbol'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(tail_count, 1);
    }

    #[test]
    fn vector_tombstone_journals_batch_keys_until_flush() {
        let dir = tempdir().unwrap();
        let hash_path = dir.path().join("hash.tombstones");
        let neural_path = dir.path().join("neural.tombstones");
        let mut journals =
            VectorTombstoneJournals::new(hash_path.clone(), Some(neural_path.clone()));

        journals.record(&[11, 12]);
        journals.record(&[21, 22, 23]);
        assert!(!hash_path.exists());
        assert!(!neural_path.exists());

        journals.flush().unwrap();
        let expected = "11\n12\n21\n22\n23\n";
        assert_eq!(fs::read_to_string(&hash_path).unwrap(), expected);
        assert_eq!(fs::read_to_string(&neural_path).unwrap(), expected);

        journals.flush().unwrap();
        assert_eq!(fs::read_to_string(hash_path).unwrap(), expected);
        assert_eq!(fs::read_to_string(neural_path).unwrap(), expected);
    }

    #[test]
    fn mass_delete_tombstones_checkpoint_with_bounded_payload() {
        let dir = tempdir().unwrap();
        let hash_path = dir.path().join("hash.tombstones");
        let mut journals =
            VectorTombstoneJournals::with_max_pending_bytes(hash_path.clone(), None, 128);
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE deleted_keys (key INTEGER PRIMARY KEY)", [])
            .unwrap();
        let mut tx = conn.transaction().unwrap();
        let mut checkpoints = 0;
        let mut largest_payload = 0;

        for key in 0..10_000u64 {
            tx.execute(
                "INSERT INTO deleted_keys (key) VALUES (?1)",
                params![key as i64],
            )
            .unwrap();
            journals.record(&[key]);
            largest_payload = largest_payload.max(journals.pending_payload.len());
            if journals.should_checkpoint() {
                commit_with_vector_tombstones(tx, &mut journals).unwrap();
                checkpoints += 1;
                tx = conn.transaction().unwrap();
            }
        }
        commit_with_vector_tombstones(tx, &mut journals).unwrap();

        assert!(checkpoints > 100);
        assert!(
            largest_payload < 160,
            "payload grew to {largest_payload} bytes"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM deleted_keys", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            10_000
        );
        assert_eq!(
            fs::read_to_string(hash_path).unwrap().lines().count(),
            10_000
        );
    }

    #[test]
    fn incremental_stats_checkpoint_persists_and_resets_delta() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (file_path TEXT NOT NULL, vector_key INTEGER NOT NULL);
             CREATE TABLE _stats (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             INSERT INTO chunks VALUES ('a.rs', 1), ('b.rs', 2);
             INSERT INTO _stats VALUES ('chunk_count', 2), ('file_count', 2),
                 ('vector_key_count', 2);",
        )
        .unwrap();
        let mut stats = IncrementalStatsDelta::load(&conn).unwrap().unwrap();

        conn.execute("DELETE FROM chunks WHERE file_path = 'a.rs'", [])
            .unwrap();
        stats.record_removal(Path::new("a.rs"), &[1]);
        stats
            .record_insertion(&conn, Path::new("c.rs"), [3, 4])
            .unwrap();
        conn.execute("INSERT INTO chunks VALUES ('c.rs', 3), ('c.rs', 4)", [])
            .unwrap();
        stats.checkpoint(&conn).unwrap();

        assert_eq!(
            conn.query_row(
                "SELECT value FROM _stats WHERE key = 'chunk_count'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row(
                "SELECT value FROM _stats WHERE key = 'file_count'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT value FROM _stats WHERE key = 'vector_key_count'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            3
        );

        conn.execute("DELETE FROM chunks WHERE file_path = 'b.rs'", [])
            .unwrap();
        stats.record_removal(Path::new("b.rs"), &[2]);
        assert_eq!(stats.final_counts(&conn).unwrap(), (2, 1, 2));
    }

    #[test]
    fn incremental_vector_key_delta_handles_shared_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (file_path TEXT NOT NULL, vector_key INTEGER NOT NULL);
             CREATE INDEX idx_chunks_vector_key ON chunks(vector_key);
             CREATE TABLE _stats (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             INSERT INTO chunks VALUES ('a.rs', 1), ('b.rs', 1);
             INSERT INTO _stats VALUES ('chunk_count', 2), ('file_count', 2),
                 ('vector_key_count', 1);",
        )
        .unwrap();
        let mut stats = IncrementalStatsDelta::load(&conn).unwrap().unwrap();

        conn.execute("DELETE FROM chunks WHERE file_path = 'a.rs'", [])
            .unwrap();
        stats.record_removal(Path::new("a.rs"), &[1]);
        stats
            .record_insertion(&conn, Path::new("c.rs"), [1, 2])
            .unwrap();
        conn.execute("INSERT INTO chunks VALUES ('c.rs', 1), ('c.rs', 2)", [])
            .unwrap();

        assert_eq!(stats.final_counts(&conn).unwrap(), (3, 2, 2));
    }

    #[test]
    fn incremental_vector_key_delta_falls_back_for_large_changes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (file_path TEXT NOT NULL, vector_key INTEGER NOT NULL);
             CREATE INDEX idx_chunks_vector_key ON chunks(vector_key);
             CREATE TABLE _stats (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             INSERT INTO chunks VALUES ('base.rs', 1);
             INSERT INTO _stats VALUES ('chunk_count', 1), ('file_count', 1),
                 ('vector_key_count', 1);",
        )
        .unwrap();
        let mut stats = IncrementalStatsDelta::load(&conn).unwrap().unwrap();
        let added_keys = 2..=(MAX_TRACKED_INCREMENTAL_VECTOR_KEYS as u64 + 2);
        stats
            .record_insertion(&conn, Path::new("bulk.rs"), added_keys.clone())
            .unwrap();
        assert!(stats.vector_key_count_requires_full_scan);

        let mut insert = conn
            .prepare("INSERT INTO chunks VALUES ('bulk.rs', ?1)")
            .unwrap();
        for key in added_keys {
            insert.execute(params![key as i64]).unwrap();
        }
        drop(insert);

        let expected = MAX_TRACKED_INCREMENTAL_VECTOR_KEYS as i64 + 2;
        assert_eq!(stats.final_counts(&conn).unwrap(), (expected, 2, expected));
    }

    #[test]
    fn failed_sqlite_commit_leaves_precommit_vector_tombstones() {
        let dir = tempdir().unwrap();
        let hash_path = dir.path().join("hash.tombstones");
        let mut journals = VectorTombstoneJournals::new(hash_path.clone(), None);
        journals.record(&[42]);

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE parents (id INTEGER PRIMARY KEY);
             CREATE TABLE children (
                 parent_id INTEGER NOT NULL,
                 FOREIGN KEY (parent_id) REFERENCES parents(id)
                     DEFERRABLE INITIALLY DEFERRED
             );",
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO children (parent_id) VALUES (1)", [])
            .unwrap();

        let error = commit_with_vector_tombstones(tx, &mut journals).unwrap_err();
        assert!(error.to_string().contains("FOREIGN KEY constraint failed"));
        assert_eq!(fs::read_to_string(hash_path).unwrap(), "42\n");
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
    fn shifted_unchanged_chunk_keeps_its_vector_key() {
        let make = |start_line, content_hash: &str| Chunk {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("src/lib.rs"),
            start_line,
            end_line: start_line + 4,
            text: "// src/lib.rs\n\npub fn stable() {}\n".to_string(),
            language: "rust".to_string(),
            kind: ChunkKind::Function,
            content_hash: content_hash.to_string(),
        };

        assert_eq!(
            build_indexed_chunk(make(10, "before"), false).vector_key,
            build_indexed_chunk(make(30, "after"), false).vector_key,
            "changing source offsets must not invalidate an unchanged embedding"
        );
    }

    #[test]
    fn identical_chunks_in_one_file_keep_distinct_vector_keys() {
        let make = || Chunk {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("src/lib.rs"),
            start_line: 10,
            end_line: 14,
            text: "// src/lib.rs\n\npub fn duplicated() {}\n".to_string(),
            language: "rust".to_string(),
            kind: ChunkKind::Function,
            content_hash: "same-content".to_string(),
        };

        assert_ne!(
            build_indexed_chunk_with_occurrence(make(), false, 0).vector_key,
            build_indexed_chunk_with_occurrence(make(), false, 1).vector_key,
            "repeated boilerplate must not collapse distinct retrieval locations"
        );
    }

    #[test]
    fn rust_doc_include_uses_concise_module_description_signature() {
        let chunk = chunk_rust_doc_include(
            Path::new("src/middleware/mod.rs"),
            3,
            Path::new("src/docs/middleware.md"),
            "# Intro\n\naxum integrates with Tower middleware.\n\n# Applying middleware\n\nRouter layers wrap routes.\n",
        )
        .into_iter()
        .next()
        .unwrap();
        let indexed = build_indexed_chunk(chunk, false);
        let signature = extract_signature(&indexed);

        assert!(signature.contains("axum integrates with Tower middleware"));
        assert!(signature.contains("Router layers wrap routes"));
        assert!(signature.len() <= 512);
    }

    #[test]
    fn java_signature_skips_documentation_and_annotations() {
        let chunk = Chunk {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("src/GsonBuilder.java"),
            start_line: 10,
            end_line: 20,
            text: "// src/GsonBuilder.java\n\n/**\n * Registers an adapter.\n */\n@CanIgnoreReturnValue\npublic GsonBuilder registerTypeAdapter(Type type, Object adapter) {\n}\n"
                .to_string(),
            language: "java".to_string(),
            kind: ChunkKind::Function,
            content_hash: "java-signature".to_string(),
        };
        let indexed = build_indexed_chunk(chunk, false);

        assert_eq!(
            extract_signature(&indexed),
            "public GsonBuilder registerTypeAdapter(Type type, Object adapter) {"
        );
    }

    #[test]
    fn dropping_index_batch_producer_cancels_blocked_sender() {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Result<IndexedFileBatch>>(0);
        let handle = std::thread::spawn(move || {
            assert!(
                sender.send(Ok(Vec::new())).is_err(),
                "receiver drop must cancel blocked producer send"
            );
        });

        drop(IndexBatchProducer::new(receiver, handle));
    }

    #[test]
    fn index_batch_producer_propagates_worker_panic() {
        let (_sender, receiver) = std::sync::mpsc::sync_channel::<Result<IndexedFileBatch>>(0);
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
    fn index_batch_producer_reports_source_read_failure() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let (_index, fields) = open_tantivy_index(&workspace.tantivy_dir()).unwrap();
        let diff = MerkleDiff {
            added_or_modified: vec![(PathBuf::from("disappeared.rs"), false)],
            deleted: Vec::new(),
        };
        let producer = spawn_index_batch_producer(&workspace, &diff, None, &fields, false, false);

        let error = match producer.recv().unwrap() {
            Ok(_) => panic!("missing source should fail the producer"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("disappeared.rs"),
            "unexpected producer error: {error:#}"
        );
        producer.finish().unwrap();
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
    fn reindex_removes_stale_symbol_definition_rows() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        let source = root.path().join("lib.rs");
        fs::write(&source, "pub fn old_tax() -> u64 { 1 }\n").unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert_eq!(
            crate::symbols::search_symbols(
                &workspace,
                "old_tax",
                crate::symbols::SymbolSearchMode::Definitions,
                Some(10),
                None,
            )
            .unwrap()
            .len(),
            1
        );

        fs::write(
            &source,
            "pub fn replacement_tax_calculator() -> u64 { 2 }\n",
        )
        .unwrap();
        index_workspace(&workspace, &model).unwrap();

        assert!(
            crate::symbols::search_symbols(
                &workspace,
                "old_tax",
                crate::symbols::SymbolSearchMode::Definitions,
                Some(10),
                None,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            crate::symbols::search_symbols(
                &workspace,
                "replacement_tax_calculator",
                crate::symbols::SymbolSearchMode::Definitions,
                Some(10),
                None,
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    #[serial]
    fn rust_doc_include_is_indexed_into_owner_and_refreshed_with_dependency() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/middleware")).unwrap();
        fs::create_dir_all(root.path().join("src/docs")).unwrap();
        fs::write(
            root.path().join("src/middleware/mod.rs"),
            "#![doc = include_str!(\"../docs/middleware.md\")]\n\
             pub fn router() {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/docs/middleware.md"),
            "Axum maps Tower middleware layers onto the router.\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let original = indexed_texts_for_file(&workspace, "src/middleware/mod.rs");
        assert!(
            original
                .iter()
                .any(|text| text.contains("maps Tower middleware layers"))
        );
        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let dependency: String = conn
            .query_row(
                "SELECT included_path FROM included_file_dependencies
                 WHERE owner_path = 'src/middleware/mod.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dependency, "src/docs/middleware.md");
        drop(conn);

        fs::write(
            root.path().join("src/docs/middleware.md"),
            "Router layers now use a replacement cardinal middleware explanation.\n",
        )
        .unwrap();
        let refresh = index_workspace(&workspace, &model).unwrap();
        assert!(
            refresh.indexed_files >= 2,
            "the included document and its owner should refresh"
        );
        let updated = indexed_texts_for_file(&workspace, "src/middleware/mod.rs");
        assert!(
            updated
                .iter()
                .any(|text| text.contains("replacement cardinal middleware"))
        );
        assert!(
            updated
                .iter()
                .all(|text| !text.contains("maps Tower middleware layers"))
        );

        fs::remove_file(root.path().join("src/docs/middleware.md")).unwrap();
        index_workspace(&workspace, &model).unwrap();
        let deleted = indexed_texts_for_file(&workspace, "src/middleware/mod.rs");
        assert!(
            deleted
                .iter()
                .all(|text| !text.contains("replacement cardinal middleware"))
        );

        fs::write(
            root.path().join("src/docs/middleware.md"),
            "Recreated cardinal middleware dependency content.\n",
        )
        .unwrap();
        index_workspace(&workspace, &model).unwrap();
        let recreated = indexed_texts_for_file(&workspace, "src/middleware/mod.rs");
        assert!(
            recreated
                .iter()
                .any(|text| text.contains("Recreated cardinal middleware"))
        );
    }

    #[test]
    #[serial]
    fn rust_doc_include_respects_gitignore_until_dependency_becomes_visible() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(root.path())
            .output()
            .unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("docs")).unwrap();
        fs::write(
            root.path().join("src/lib.rs"),
            "#![doc = include_str!(\"../docs/private.md\")]\n",
        )
        .unwrap();
        fs::write(
            root.path().join("docs/private.md"),
            "private cardinal documentation must remain ignored\n",
        )
        .unwrap();
        fs::write(root.path().join(".gitignore"), "docs/private.md\n").unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert!(
            indexed_texts_for_file(&workspace, "src/lib.rs")
                .iter()
                .all(|text| !text.contains("private cardinal documentation"))
        );

        fs::write(root.path().join(".gitignore"), "").unwrap();
        index_workspace(&workspace, &model).unwrap();
        assert!(
            indexed_texts_for_file(&workspace, "src/lib.rs")
                .iter()
                .any(|text| text.contains("private cardinal documentation"))
        );
    }

    #[test]
    #[serial]
    fn clean_git_noop_invalidates_when_repository_excludes_change() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(["-c", "commit.gpgSign=false"])
                .args(args)
                .current_dir(root.path())
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?} failed");
        };
        git(&["init", "-b", "main"]);
        fs::write(
            root.path().join("visible.rs"),
            "pub fn visible_marker() {}\n",
        )
        .unwrap();
        git(&["add", "visible.rs"]);
        git(&["commit", "-m", "base"]);

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        assert!(!indexed_texts_for_file(&workspace, "visible.rs").is_empty());

        let no_change = index_workspace(&workspace, &model).unwrap();
        assert_eq!(no_change.indexed_files, 0);
        assert_eq!(no_change.deleted_files, 0);

        fs::write(root.path().join(".git/info/exclude"), "visible.rs\n").unwrap();
        let excluded = index_workspace(&workspace, &model).unwrap();
        assert_eq!(excluded.deleted_files, 1);
        assert!(indexed_texts_for_file(&workspace, "visible.rs").is_empty());
    }

    #[test]
    #[serial]
    fn adding_dependency_reindexes_importer_with_unresolved_spec() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/lib.rs"),
            "pub(crate) mod helper;\npub fn run() { helper::work(); }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let unresolved = conn
            .query_row(
                "SELECT COUNT(*) FROM unresolved_file_dependencies
                 WHERE source_path = 'src/lib.rs' AND spec = 'mod/helper'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(unresolved, 1);
        drop(conn);

        fs::write(root.path().join("src/helper.rs"), "pub fn work() {}\n").unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 2);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let edge = conn
            .query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'src/lib.rs'
                   AND target_path = 'src/helper.rs'
                   AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Dependency as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(edge, 1);
        let unresolved = conn
            .query_row(
                "SELECT COUNT(*) FROM unresolved_file_dependencies
                 WHERE source_path = 'src/lib.rs' AND spec = 'mod/helper'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(unresolved, 0);
    }

    #[test]
    #[serial]
    fn adding_jvm_and_dotnet_tests_reindexes_source_owners() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        let java_source = Path::new("src/main/java/com/acme/Auth.java");
        let java_test = Path::new("src/test/java/com/acme/AuthTest.java");
        let dotnet_source = Path::new("src/Auth/Services/Token.cs");
        let dotnet_test = Path::new("tests/Auth.Tests/Services/TokenTests.cs");
        fs::create_dir_all(root.path().join(java_source.parent().unwrap())).unwrap();
        fs::create_dir_all(root.path().join(dotnet_source.parent().unwrap())).unwrap();
        fs::write(root.path().join(java_source), "class Auth {}\n").unwrap();
        fs::write(root.path().join(dotnet_source), "class Token {}\n").unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        for source in [java_source, dotnet_source] {
            let unresolved = conn
                .query_row(
                    "SELECT COUNT(*) FROM unresolved_file_dependencies
                     WHERE source_path = ?1 AND language = 'context_test'
                       AND spec = 'conventional_test'",
                    params![source.to_string_lossy()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
            assert_eq!(
                unresolved,
                1,
                "missing compact mapping for {}",
                source.display()
            );
        }
        drop(conn);

        fs::create_dir_all(root.path().join(java_test.parent().unwrap())).unwrap();
        fs::create_dir_all(root.path().join(dotnet_test.parent().unwrap())).unwrap();
        fs::write(root.path().join(java_test), "class AuthTest {}\n").unwrap();
        fs::write(root.path().join(dotnet_test), "class TokenTests {}\n").unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 4);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        for (source, test) in [(java_source, java_test), (dotnet_source, dotnet_test)] {
            let edge = conn
                .query_row(
                    "SELECT COUNT(*) FROM file_edges
                     WHERE source_path = ?1 AND target_path = ?2 AND kind = ?3",
                    params![
                        source.to_string_lossy(),
                        test.to_string_lossy(),
                        crate::context_graph::FileEdgeKind::Test as i64,
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap();
            assert_eq!(edge, 1, "missing test edge for {}", source.display());
        }
    }

    #[test]
    #[serial]
    fn adding_rust_path_module_reindexes_importer() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/generated")).unwrap();
        fs::write(
            root.path().join("src/lib.rs"),
            "#[path = \"generated/auth.rs\"]\nmod auth;\npub fn run() { auth::work(); }\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let unresolved = conn
            .query_row(
                "SELECT COUNT(DISTINCT spec) FROM unresolved_file_dependencies
                 WHERE source_path = 'src/lib.rs'
                   AND spec = 'pathmod/generated/auth.rs'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(unresolved, 1);
        drop(conn);

        fs::write(
            root.path().join("src/generated/auth.rs"),
            "pub fn work() {}\n",
        )
        .unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 2);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let edge = conn
            .query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'src/lib.rs'
                   AND target_path = 'src/generated/auth.rs'
                   AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Dependency as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(edge, 1);
    }

    #[test]
    #[serial]
    fn adding_python_relative_member_reindexes_importer() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("app/auth")).unwrap();
        fs::write(
            root.path().join("app/auth/service.py"),
            "from . import helper\n\ndef run(): return helper.work()\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        fs::write(
            root.path().join("app/auth/helper.py"),
            "def work(): return True\n",
        )
        .unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 2);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let edge = conn
            .query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'app/auth/service.py'
                   AND target_path = 'app/auth/helper.py'
                   AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Dependency as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(edge, 1);
    }

    #[test]
    #[serial]
    fn adding_manifest_reindexes_configured_sources() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/lib.rs"), "pub fn run() {}\n").unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 2);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let edge = conn
            .query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'src/lib.rs'
                   AND target_path = 'Cargo.toml'
                   AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Config as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(edge, 1);
        drop(conn);

        fs::remove_file(root.path().join("Cargo.toml")).unwrap();
        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, 1);
        assert_eq!(summary.deleted_files, 1);

        let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let edge = conn
            .query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'src/lib.rs' AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Config as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(edge, 0);
    }

    #[test]
    #[serial]
    fn changing_manifest_resolution_identity_reindexes_importers_only_when_needed() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = 'old-package'\nversion = '0.1.0'\n",
        )
        .unwrap();
        fs::write(root.path().join("src/auth.rs"), "pub struct Session;\n").unwrap();
        fs::write(
            root.path().join("tests/integration.rs"),
            "use new_package::auth::Session;\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let dependency_count = || {
            let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM file_edges
                 WHERE source_path = 'tests/integration.rs'
                   AND target_path = 'src/auth.rs'
                   AND kind = ?1",
                [crate::context_graph::FileEdgeKind::Dependency as i64],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        assert_eq!(dependency_count(), 0);

        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = 'old-package'\nversion = '0.2.0'\n",
        )
        .unwrap();
        let version_summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(version_summary.indexed_files, 1);
        assert_eq!(dependency_count(), 0);

        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = 'new-package'\nversion = '0.2.0'\n",
        )
        .unwrap();
        let rename_summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(rename_summary.indexed_files, 3);
        assert_eq!(dependency_count(), 1);
    }

    #[test]
    fn rust_doc_include_rejects_workspace_escape_and_oversized_file() {
        assert_eq!(
            normalize_workspace_relative_include(
                Path::new("src/lib.rs"),
                Path::new("../../outside.md")
            ),
            None
        );
        assert_eq!(
            normalize_workspace_relative_include(
                Path::new("src/lib.rs"),
                Path::new("../docs/guide.md")
            ),
            Some(PathBuf::from("docs/guide.md"))
        );

        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("docs")).unwrap();
        fs::write(
            root.path().join("docs/large.md"),
            vec![b'a'; MAX_RUST_DOC_INCLUDE_BYTES as usize + 1],
        )
        .unwrap();
        let snapshot = MerkleSnapshot {
            root_hash: String::new(),
            files: BTreeMap::from([("docs/large.md".to_string(), "test-metadata-0".to_string())]),
        };
        let (chunks, dependencies) = load_rust_doc_includes(
            root.path(),
            Path::new("src/lib.rs"),
            &[RustDocInclude {
                source_line: 1,
                path: PathBuf::from("../docs/large.md"),
            }],
            Some(&snapshot),
        );
        assert!(chunks.is_empty());
        assert_eq!(dependencies, vec![PathBuf::from("docs/large.md")]);
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
            crate::vector_store::VectorTier::Hash,
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
            crate::vector_store::VectorTier::Hash,
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
            crate::vector_store::VectorTier::Hash,
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
            crate::vector_store::VectorTier::Neural,
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
    fn targeted_watcher_update_cannot_certify_unreconciled_filter_mode() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(root.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(
            root.path().join("visible.rs"),
            "pub fn visible_migration_marker() {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("ignored.rs"),
            "pub fn ignored_migration_marker() {}\n",
        )
        .unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        fs::remove_file(workspace.index_dir.join(INDEXED_SKIP_GITIGNORE_FILE)).unwrap();
        let mut metadata = workspace.read_metadata().unwrap().unwrap();
        metadata.skip_gitignore = true;
        workspace.write_metadata(&metadata).unwrap();

        index_workspace_paths_for_watcher(&workspace, &model, &[PathBuf::from("visible.rs")])
            .unwrap();

        assert!(workspace_index_matches_skip_gitignore(&workspace, true));
        assert!(
            indexed_texts_for_file(&workspace, "ignored.rs")
                .iter()
                .any(|text| text.contains("ignored_migration_marker"))
        );
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
            crate::vector_store::VectorTier::Neural,
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
        let vector_modified = fs::metadata(workspace.vector_neural_path())
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let n2 = enhance_workspace_neural(&workspace, &second_model).unwrap();
        assert_eq!(n2, 0, "second enhance should skip already-processed chunks");
        assert_eq!(
            fs::metadata(workspace.vector_neural_path())
                .unwrap()
                .modified()
                .unwrap(),
            vector_modified,
            "no-op enhancement must not rewrite the vector store"
        );
        assert_eq!(
            fs::read_to_string(workspace.neural_backend_path()).unwrap(),
            "first backend",
            "no-op enhancement must not rewrite recorded backend"
        );
    }

    #[test]
    #[serial]
    fn shifted_unchanged_chunks_reuse_hash_and_neural_embeddings() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        let source_path = root.path().join("payments.rs");
        let source = (0..12)
            .map(|index| {
                format!("pub fn reconcile_payment_{index:02}() -> usize {{ {index} }}\n\n")
            })
            .collect::<String>();
        fs::write(&source_path, &source).unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let neural_model = RecordedTestEmbedding {
            model: HashEmbeddingModel::new(EMBEDDING_DIMENSIONS),
            backend: "test local neural backend",
            identity: crate::embedding::configured_neural_model_identity(),
        };
        let indexed = index_workspace(&workspace, &hash_model).unwrap();
        assert_eq!(
            enhance_workspace_hash(&workspace, &hash_model).unwrap(),
            indexed.total_chunks
        );
        assert_eq!(
            enhance_workspace_neural(&workspace, &neural_model).unwrap(),
            indexed.total_chunks
        );

        fs::write(&source_path, format!("\n\n{source}")).unwrap();
        index_workspace(&workspace, &hash_model).unwrap();
        assert!(!workspace.hash_tombstones_path().exists());
        assert!(!workspace.neural_tombstones_path().exists());
        assert_eq!(enhance_workspace_hash(&workspace, &hash_model).unwrap(), 0);
        assert_eq!(
            enhance_workspace_neural(&workspace, &neural_model).unwrap(),
            0
        );

        let changed = format!("\n\n{source}").replace(
            "pub fn reconcile_payment_05() -> usize { 5 }",
            "pub fn reconcile_payment_05() -> usize { 500 }",
        );
        fs::write(&source_path, changed).unwrap();
        index_workspace(&workspace, &hash_model).unwrap();
        assert_eq!(enhance_workspace_hash(&workspace, &hash_model).unwrap(), 1);
        assert_eq!(
            enhance_workspace_neural(&workspace, &neural_model).unwrap(),
            1
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
    fn foreground_reindex_batches_vector_tombstones_for_file_burst() {
        const FILES: usize = 32;
        const FUNCTIONS_PER_FILE: usize = 4;

        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        for file_index in 0..FILES {
            let source = (0..FUNCTIONS_PER_FILE)
                .map(|function_index| {
                    format!(
                        "pub fn original_{file_index}_{function_index}() -> usize {{ {function_index} }}\n"
                    )
                })
                .collect::<String>();
            fs::write(root.path().join(format!("file_{file_index}.rs")), source).unwrap();
        }

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let expected = {
            let conn = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
            let mut stmt = conn.prepare("SELECT vector_key FROM chunks").unwrap();
            stmt.query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .map(|row| row.unwrap() as u64)
                .collect::<HashSet<_>>()
        };
        assert_eq!(expected.len(), FILES * FUNCTIONS_PER_FILE);

        for file_index in 0..FILES {
            let source = (0..FUNCTIONS_PER_FILE)
                .map(|function_index| {
                    format!(
                        "pub fn replacement_{file_index}_{function_index}() -> usize {{ {} }}\n",
                        function_index + 100
                    )
                })
                .collect::<String>();
            fs::write(root.path().join(format!("file_{file_index}.rs")), source).unwrap();
        }

        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(summary.indexed_files, FILES);
        for path in [
            workspace.hash_tombstones_path(),
            workspace.neural_tombstones_path(),
        ] {
            let keys = fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|line| line.parse::<u64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(keys.len(), expected.len());
            assert_eq!(keys.into_iter().collect::<HashSet<_>>(), expected);
        }
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
    fn deleting_last_file_drains_vector_tombstones() {
        const VECTOR_COUNT: usize = 5_000;
        const EMPTY_STORE_MAX_BYTES: u64 = 16 * 1024;

        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        let source = root.path().join("mod.rs");
        fs::write(&source, "pub fn removed() -> i32 { 1 }\n").unwrap();

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

        let sqlite = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
        let indexed_key = sqlite
            .query_row("SELECT vector_key FROM chunks LIMIT 1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap() as u64;
        drop(sqlite);
        let mut vector_keys = vec![indexed_key];
        let mut next_key = 10_000_000_u64;
        while vector_keys.len() < VECTOR_COUNT {
            if next_key != indexed_key {
                vector_keys.push(next_key);
            }
            next_key += 1;
        }

        for (path, dimensions, quantization, tier) in [
            (
                workspace.vector_path(),
                hash_model.dimensions(),
                HASH_VECTOR_QUANTIZATION,
                crate::vector_store::VectorTier::Hash,
            ),
            (
                workspace.vector_neural_path(),
                neural_model.dimensions(),
                NEURAL_VECTOR_QUANTIZATION,
                crate::vector_store::VectorTier::Neural,
            ),
        ] {
            let mut store = VectorStore::open(&path, dimensions, quantization, tier).unwrap();
            store
                .reserve_additional(VECTOR_COUNT.saturating_sub(store.size()))
                .unwrap();
            for key in vector_keys.iter().copied().skip(1) {
                store.add_unchecked(key, vec![1.0; dimensions]).unwrap();
            }
            store.save().unwrap();
            assert_eq!(store.size(), VECTOR_COUNT);
            assert!(fs::metadata(path).unwrap().len() > EMPTY_STORE_MAX_BYTES);
        }

        fs::remove_file(source).unwrap();
        let summary = index_workspace(&workspace, &hash_model).unwrap();
        assert_eq!(summary.total_chunks, 0);
        let tombstones = vector_keys
            .iter()
            .map(|key| format!("{key}\n"))
            .collect::<String>();
        fs::write(workspace.hash_tombstones_path(), &tombstones).unwrap();
        fs::write(workspace.neural_tombstones_path(), tombstones).unwrap();
        assert!(workspace.hash_tombstones_path().exists());
        assert!(workspace.neural_tombstones_path().exists());
        assert!(workspace.needs_hash_enhancement());
        assert!(workspace.needs_neural_enhancement());

        assert_eq!(enhance_workspace_hash(&workspace, &hash_model).unwrap(), 0);
        assert!(!workspace.hash_tombstones_path().exists());
        assert!(!workspace.hash_tombstones_processing_path().exists());
        assert!(workspace.needs_neural_enhancement());

        assert_eq!(
            enhance_workspace_neural(&workspace, &neural_model).unwrap(),
            0
        );
        for path in [
            workspace.neural_tombstones_path(),
            workspace.neural_tombstones_processing_path(),
        ] {
            assert!(!path.exists(), "{} should be removed", path.display());
        }
        assert!(!workspace.needs_hash_enhancement());
        assert!(!workspace.needs_neural_enhancement());

        let hash_store = VectorStore::open_readonly(
            &workspace.vector_path(),
            EMBEDDING_DIMENSIONS,
            HASH_VECTOR_QUANTIZATION,
            crate::vector_store::VectorTier::Hash,
        )
        .unwrap();
        assert_eq!(hash_store.size(), 0);
        assert!(
            fs::metadata(workspace.vector_path()).unwrap().len() <= EMPTY_STORE_MAX_BYTES,
            "empty hash store retained allocated graph capacity"
        );
        let neural_store = VectorStore::open_readonly(
            &workspace.vector_neural_path(),
            neural_model.dimensions(),
            NEURAL_VECTOR_QUANTIZATION,
            crate::vector_store::VectorTier::Neural,
        )
        .unwrap();
        assert_eq!(neural_store.size(), 0);
        assert!(
            fs::metadata(workspace.vector_neural_path()).unwrap().len() <= EMPTY_STORE_MAX_BYTES,
            "empty neural store retained allocated graph capacity"
        );
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
    fn retryable_tantivy_write_errors_are_classified() {
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
        let worker_io = anyhow::Error::from(tantivy::TantivyError::ErrorInThread(
            "An index writer was killed.. A worker thread encountered an error \
             (io::Error most likely) or panicked."
                .to_owned(),
        ));
        let worker_panic = anyhow::Error::from(tantivy::TantivyError::ErrorInThread(
            "worker panicked while indexing".to_owned(),
        ));

        assert!(is_retryable_tantivy_write_error(&denied));
        assert!(is_retryable_tantivy_write_error(&worker_io));
        assert!(!is_retryable_tantivy_write_error(&missing));
        assert!(!is_retryable_tantivy_write_error(&worker_panic));
    }

    #[test]
    fn whole_index_retry_retries_tantivy_worker_io_error() {
        let attempts = std::cell::Cell::new(0);

        let result: Result<()> = retry_transient_tantivy_writes(|| {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            if attempt == 0 {
                return Err(anyhow::Error::from(tantivy::TantivyError::ErrorInThread(
                    "An index writer was killed.. A worker thread encountered an error \
                     (io::Error most likely) or panicked."
                        .to_owned(),
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
        let index_dir = tempfile::tempdir().unwrap();
        let (_index, fields) = super::open_tantivy_index(index_dir.path()).unwrap();
        let prepared = super::prepare_indexed_chunk(chunk.clone(), &fields);

        assert_eq!(prepared.chunk.text, chunk.text);
        assert_eq!(super::decompress_text(prepared.compressed_text), chunk.text);
        assert_eq!(
            prepared
                .tantivy_doc
                .get_first(fields.vector_key)
                .and_then(|value| value.as_u64()),
            Some(chunk.vector_key)
        );
    }

    #[test]
    fn batch_text_fetch_reads_only_requested_chunk_text() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                vector_key INTEGER PRIMARY KEY,
                text BLOB NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (vector_key, text) VALUES (?1, ?2)",
            params![7_i64, super::compress_text("requested chunk")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (vector_key, text) VALUES (?1, ?2)",
            params![8_i64, super::compress_text("other chunk")],
        )
        .unwrap();

        let texts = fetch_chunk_texts_by_vector_keys_batch(&conn, &[7, 99]).unwrap();

        assert_eq!(texts.len(), 1);
        assert_eq!(texts.get(&7).map(String::as_str), Some("requested chunk"));
    }

    #[test]
    fn batch_text_fetch_rejects_corrupted_compressed_chunks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                vector_key INTEGER PRIMARY KEY,
                text BLOB NOT NULL
            );",
        )
        .unwrap();
        let mut corrupted = zstd::encode_all(&b"stored chunk"[..], 1).unwrap();
        corrupted.truncate(corrupted.len() - 3);
        conn.execute(
            "INSERT INTO chunks (vector_key, text) VALUES (?1, ?2)",
            params![7_i64, corrupted],
        )
        .unwrap();

        let error = fetch_chunk_texts_by_vector_keys_batch(&conn, &[7]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to read stored text for vector key 7")
        );
    }

    #[test]
    fn batch_text_fetch_handles_multiple_sqlite_parameter_batches() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                vector_key INTEGER PRIMARY KEY,
                text BLOB NOT NULL
            );",
        )
        .unwrap();
        {
            let mut stmt = conn
                .prepare("INSERT INTO chunks (vector_key, text) VALUES (?1, ?2)")
                .unwrap();
            for key in 1_i64..=501 {
                stmt.execute(params![key, super::compress_text(&format!("chunk {key}"))])
                    .unwrap();
            }
        }

        let keys = (1_u64..=501).collect::<Vec<_>>();
        let texts = fetch_chunk_texts_by_vector_keys_batch(&conn, &keys).unwrap();

        assert_eq!(texts.len(), 501);
        assert_eq!(texts.get(&1).map(String::as_str), Some("chunk 1"));
        assert_eq!(texts.get(&501).map(String::as_str), Some("chunk 501"));
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

        let entries = fs::read_dir(&workspace.index_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![std::ffi::OsString::from("index.lock")]);
    }

    #[test]
    #[serial]
    fn remove_workspace_index_waits_for_existing_lock() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        fs::write(workspace.index_dir.join("stale"), "data").unwrap();
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(workspace.lock_path())
            .unwrap();
        fs2::FileExt::lock_exclusive(&lock_file).unwrap();

        let (sender, receiver) = std::sync::mpsc::channel();
        let workspace_to_remove = workspace.clone();
        let handle = std::thread::spawn(move || {
            sender
                .send(remove_workspace_index(&workspace_to_remove))
                .unwrap();
        });

        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "removal must wait for current lock holder"
        );
        fs2::FileExt::unlock(&lock_file).unwrap();
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        handle.join().unwrap();
        assert!(!workspace.index_dir.join("stale").exists());
        assert!(workspace.lock_path().exists());
    }

    #[test]
    #[serial]
    fn remove_workspace_index_keeps_missing_index_absent() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let workspace = Workspace::resolve(root.path()).unwrap();
        assert!(!workspace.index_dir.exists());
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
}
