use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use tantivy::schema::{Field, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index as TantivyIndex, TantivyDocument, Term, doc};

use crate::chunking::{Chunk, chunk_source, is_indexable_file};
use crate::embedding::EmbeddingModel;
use crate::merkle::{MerkleDiff, MerkleSnapshot};
use crate::vector_store::{ScalarKind, VectorStore};
use crate::workspace::{Workspace, WorkspaceMetadata};

const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];

fn compress_text(text: &str) -> Vec<u8> {
    zstd::encode_all(text.as_bytes(), 3).unwrap_or_else(|_| text.as_bytes().to_vec())
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
}

#[derive(Debug, Clone)]
pub struct TantivyFields {
    pub chunk_id: Field,
    pub file_path: Field,
    pub start_line: Field,
    pub end_line: Field,
    pub language: Field,
    pub kind: Field,
    pub text: Field,
    pub content_hash: Field,
}

#[derive(Debug, Clone)]
pub struct StorageHandles {
    pub sqlite_path: PathBuf,
    pub tantivy_dir: PathBuf,
    pub vector_path: PathBuf,
}

pub fn workspace_is_indexed(workspace: &Workspace) -> bool {
    workspace.sqlite_path().exists()
        && workspace.tantivy_dir().exists()
        && workspace.vector_path().exists()
        && match workspace.read_metadata() {
            Ok(Some(m)) => m.last_indexed_at_unix.is_some(),
            _ => false,
        }
}

pub fn remove_workspace_index(workspace: &Workspace) -> Result<()> {
    if workspace.index_dir.exists() {
        fs::remove_dir_all(&workspace.index_dir)?;
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
    let vectors = VectorStore::open(&vector_path, embedding_dimensions, ScalarKind::F16)?;
    vectors.save()?;

    Ok(StorageHandles {
        sqlite_path,
        tantivy_dir,
        vector_path,
    })
}

pub fn index_workspace(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<IndexingSummary> {
    workspace.ensure_dirs()?;

    // Acquire an exclusive file lock to prevent concurrent writes to the
    // vector store (usearch) and other index files. The lock is advisory
    // and automatically released when `_lock_file` is dropped.
    let lock_path = workspace.lock_path();
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;
    fs2::FileExt::lock_exclusive(&lock_file)
        .with_context(|| format!("failed to acquire index lock {}", lock_path.display()))?;

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

    let result = index_workspace_inner(workspace, embedding_model);

    let _ = fs2::FileExt::unlock(&lock_file);
    result
}

fn index_workspace_inner(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<IndexingSummary> {
    let _ = open_storage(workspace, embedding_model.dimensions())?;

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
            watch_enabled: true,
        })?;
    }

    // Trust-but-verify: if a live watcher daemon is confirmed, skip the
    // expensive Merkle rebuild entirely. The watcher already triggered
    // re-indexing for any changed files through filesystem events.
    if workspace.is_watcher_alive() && workspace_is_indexed(workspace) {
        return Ok(IndexingSummary {
            workspace_id: workspace.id.clone(),
            indexed_files: 0,
            deleted_files: 0,
            total_chunks: count_chunks(&workspace.sqlite_path())?,
        });
    }

    // ── Worktree overlay ─────────────────────────────────────────────────
    // If this is a git worktree and the base has a fresh index, create a
    // thin overlay containing only divergent files instead of copying the
    // entire base. The base index is referenced by path, not copied.
    let overlay_mode = if let Some(ref base_dir) = workspace.base_index_dir {
        let base_sqlite = base_dir.join("metadata.sqlite3");
        let base_merkle = base_dir.join("merkle_snapshot.json");

        if (!base_sqlite.exists() || !base_merkle.exists())
            && !workspace.has_overlay()
            && let Some(main_root) = workspace.main_worktree_root()
        {
            eprintln!("  ⚡ base workspace is not indexed, running full base indexing first...");
            let base_workspace = crate::workspace::Workspace::resolve(&main_root)?;
            // We recursively call index_workspace on the base. It will acquire its
            // own safe lock and index natively.
            let _ = index_workspace(&base_workspace, embedding_model)?;
            eprintln!("  ⚡ base indexing complete, proceeding with overlay...");
        }

        if base_sqlite.exists() && base_merkle.exists() && !workspace.has_overlay() {
            eprintln!("  ⚡ creating worktree overlay (no copy)...");
            let _ = fs::write(workspace.indexing_progress_path(), "building overlay");

            // Record base reference
            let main_root = workspace
                .main_worktree_root()
                .context("cannot find main worktree root")?;
            let base_ref = serde_json::json!({
                "base_index_dir": base_dir.to_string_lossy(),
                "base_workspace_root": main_root.to_string_lossy(),
                "created_at_unix": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
            fs::write(
                workspace.base_ref_path(),
                serde_json::to_vec_pretty(&base_ref)?,
            )?;

            // Content-based diff: base root vs worktree root
            let _ = fs::write(
                workspace.indexing_progress_path(),
                "scanning (content-based)",
            );
            let old = MerkleSnapshot::build_content_based(&main_root)?;
            let new = MerkleSnapshot::build_content_based(&workspace.root)?;
            let diff = old.diff(&new);

            eprintln!(
                "  ⚡ overlay delta: {} added/modified, {} deleted",
                diff.added_or_modified.len(),
                diff.deleted.len()
            );

            // Save the content-based snapshot as the worktree's Merkle state
            // Future incremental diffs will use mtime mode from this baseline
            new.save(&workspace.merkle_snapshot_path())?;

            Some(diff)
        } else if workspace.has_overlay() {
            // Overlay already exists — do normal incremental update on overlay stores
            None
        } else {
            // Base doesn't exist yet — fall through to full index
            None
        }
    } else {
        None
    };

    // When not in overlay creation mode, use the standard Merkle diff path
    let diff = if let Some(overlay_diff) = overlay_mode {
        overlay_diff
    } else if workspace.has_overlay() {
        // Incremental update to existing overlay
        let old = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
        let _ = fs::write(workspace.indexing_progress_path(), "scanning");
        let new = MerkleSnapshot::build(&workspace.root)?;
        let d = old.diff(&new);
        new.save(&workspace.merkle_snapshot_path())?;
        d
    } else {
        // Standard full-index path (non-worktree or base not available)
        let old = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
        let _ = fs::write(workspace.indexing_progress_path(), "scanning");
        let new = MerkleSnapshot::build(&workspace.root)?;
        let d = old.diff(&new);
        if d.added_or_modified.is_empty() && d.deleted.is_empty() && workspace_is_indexed(workspace)
        {
            return Ok(IndexingSummary {
                workspace_id: workspace.id.clone(),
                indexed_files: 0,
                deleted_files: 0,
                total_chunks: count_workspace_chunks(workspace).unwrap_or(0),
            });
        }
        new.save(&workspace.merkle_snapshot_path())?;
        d
    };

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

    let mut sqlite = Connection::open(&sqlite_path)?;
    create_tables(&sqlite)?;
    if use_overlay {
        create_overlay_tables(&sqlite)?;
    }

    fs::create_dir_all(&tantivy_path)?;
    let (tantivy, fields) = open_tantivy_index(&tantivy_path)?;
    let mut writer = tantivy.writer(50_000_000)?;

    let mut vector_index =
        VectorStore::open(&vector_path, embedding_model.dimensions(), ScalarKind::F16)?;

    // Batch SQLite writes in a transaction for ~10-50x speedup.
    // Mutable so we can periodically commit and avert massive WAL files.
    let mut tx = sqlite.transaction()?;

    // In overlay mode, tombstone deleted files instead of removing from base
    if use_overlay {
        for rel_path in &diff.deleted {
            let rel_str = rel_path.to_string_lossy().to_string();
            tx.execute(
                "INSERT OR IGNORE INTO tombstones (file_path) VALUES (?1)",
                params![rel_str],
            )?;
            // Also remove from overlay if it was previously added there
            tx.execute("DELETE FROM chunks WHERE file_path = ?1", params![rel_str])?;
        }
    } else {
        apply_deletions(&tx, &mut writer, &fields, &mut vector_index, &diff.deleted)?;
    }

    let total = diff.added_or_modified.len();
    let show_progress = total > 0 && std::io::stderr().is_terminal();
    let progress_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let t0 = std::time::Instant::now();
    let mut total_chunks_processed = 0;
    let mut touched_files = HashSet::new();
    let mut chunks_since_commit = 0;

    // Stream through batches to rigidly bound memory footprints.
    // 4096 files is highly parallelizable while capping memory overhead effectively.
    let (tx_batch, rx_batch) =
        std::sync::mpsc::sync_channel::<Vec<(std::path::PathBuf, Vec<IndexedChunk>)>>(2);

    let progress_counter_clone = progress_counter.clone();
    let root_clone = workspace.root.clone();
    let progress_path_clone = workspace.indexing_progress_path();
    let diff_paths: Vec<_> = diff.added_or_modified.clone();

    let _ = fs::write(&progress_path_clone, format!("0/{total}"));

    std::thread::spawn(move || {
        for batch_paths in diff_paths.chunks(4096) {
            let file_chunks: Vec<_> = batch_paths
                .par_iter()
                .filter_map(|rel_path| {
                    let abs_path = root_clone.join(rel_path);
                    if !abs_path.exists() {
                        progress_counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }

                    let content_bytes = fs::read(&abs_path).ok()?;
                    if !is_indexable_file(rel_path, &content_bytes) {
                        progress_counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }

                    let content = match String::from_utf8(content_bytes) {
                        Ok(text) => text,
                        Err(err) => String::from_utf8_lossy(&err.into_bytes()).into_owned(),
                    };

                    let chunks = chunk_source(rel_path, &content);
                    let indexed: Vec<_> = chunks.into_iter().map(build_indexed_chunk).collect();

                    let n = progress_counter_clone
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        + 1;
                    if show_progress && n.is_multiple_of(100) {
                        eprint!("\r\x1b[K  [{n}/{total}] chunking...");
                    }
                    if n.is_multiple_of(500) {
                        let _ = fs::write(&progress_path_clone, format!("{n}/{total}"));
                    }

                    if indexed.is_empty() {
                        return None;
                    }
                    Some((rel_path.clone(), indexed))
                })
                .collect();

            if !file_chunks.is_empty() && tx_batch.send(file_chunks).is_err() {
                break;
            }
        }
    });

    while let Ok(file_chunks) = rx_batch.recv() {
        // Phase 2: Batch embed (very fast hashing model).
        let all_texts: Vec<&str> = file_chunks
            .iter()
            .flat_map(|(_, chunks)| chunks.iter().map(|c| c.text.as_str()))
            .collect();

        let all_embeddings = embedding_model.embed_batch(&all_texts);

        // Phase 3: Sequential sync to persistence layers.
        let mut embed_idx = 0;
        for (rel_path, indexed_chunks) in &file_chunks {
            touched_files.insert(rel_path.to_string_lossy().to_string());
            total_chunks_processed += indexed_chunks.len();
            chunks_since_commit += indexed_chunks.len();

            remove_file_chunks(&tx, &mut writer, &fields, &mut vector_index, rel_path)?;

            // In overlay mode, tombstone the base version so search suppresses
            // the stale base chunks for this file path.
            if use_overlay {
                let rel_str = rel_path.to_string_lossy().to_string();
                tx.execute(
                    "INSERT OR IGNORE INTO tombstones (file_path) VALUES (?1)",
                    params![rel_str],
                )?;
            }

            for indexed in indexed_chunks {
                let embedding = all_embeddings[embed_idx].clone();
                embed_idx += 1;
                vector_index.upsert(indexed.vector_key, embedding);
                insert_chunk(&tx, indexed)?;
                add_chunk_doc(&mut writer, &fields, indexed)?;
            }
        }

        // Prevent memory/WAL ballooning on massive repositories
        if chunks_since_commit >= 100_000 {
            tx.commit()?;
            writer.commit()?;
            vector_index.save()?;
            tx = sqlite.transaction()?;
            chunks_since_commit = 0;
        }
    }

    let t1 = std::time::Instant::now();
    if total > 0 {
        eprint!(
            "\r\x1b[K  ✓ {} files, {} chunks — indexed completely in {:.1}s\n",
            touched_files.len(),
            total_chunks_processed,
            t1.duration_since(t0).as_secs_f64()
        );
    }

    // Update cached stats before committing so status reads are O(1).
    let chunk_count: i64 = tx
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .unwrap_or(0);
    let file_count: i64 = tx
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
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

    tx.commit()?;

    writer.commit()?;
    writer.wait_merging_threads()?;

    vector_index.save()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let metadata = WorkspaceMetadata {
        id: workspace.id.clone(),
        root: workspace.root.clone(),
        created_at_unix: workspace
            .read_metadata()?
            .map(|m| m.created_at_unix)
            .unwrap_or(now),
        last_indexed_at_unix: Some(now),
        watch_enabled: true,
    };
    workspace.write_metadata(&metadata)?;

    Ok(IndexingSummary {
        workspace_id: workspace.id.clone(),
        indexed_files: touched_files.len(),
        deleted_files: diff.deleted.len(),
        total_chunks: count_workspace_chunks(workspace).unwrap_or(0),
    })
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

#[cfg(target_os = "macos")]
fn parse_system_load(load1: f64, cpus: f64) -> Option<String> {
    // High system load is defined as the 1-minute load average exceeding
    // 80% of the total available logical CPU cores.
    // Example: On an 8-core machine, a 1-minute load average > 6.4 triggers pausing.
    if load1 > cpus * 0.8 {
        Some(format!(
            "High System Load ({:.1} > {:.1} max)",
            load1,
            cpus * 0.8
        ))
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

#[cfg(not(target_os = "macos"))]
fn check_system_constraints() -> Option<String> {
    None
}

/// Compute neural (ONNX) embeddings for all chunks and save as a separate
/// vector store. This is designed to run in a background thread after the
/// fast hash-based index returns results to the user.
pub fn enhance_workspace_neural(
    workspace: &Workspace,
    neural_model: &dyn EmbeddingModel,
) -> Result<usize> {
    let sqlite = open_sqlite(&workspace.sqlite_path())?;

    let mut stmt = sqlite.prepare("SELECT vector_key, text FROM chunks ORDER BY vector_key")?;
    let rows = stmt.query_map([], |row| {
        let key = row.get::<_, i64>(0)? as u64;
        let raw: Vec<u8> = row.get(1)?;
        Ok((key, decompress_text(raw)))
    })?;

    let mut vector_index = VectorStore::open(
        &workspace.vector_neural_path(),
        neural_model.dimensions(),
        ScalarKind::F32,
    )?;

    // Use a smaller batch size (16) to ensure Apple Silicon / CoreML stays within the L2 cache
    // and doesn't trigger thermal throttling or high memory transfer overhead while still
    // being 2x faster than the original setup.
    let mut batch = Vec::with_capacity(16);
    let mut newly_processed = 0;

    // To support resumption, we'll discover how many are already in the index
    // so we can correctly broadcast our starting percentage.
    let mut progress_count = 0;

    let process_batch =
        |batch: &mut Vec<(u64, String)>, count: &mut usize, v_index: &mut VectorStore| {
            if batch.is_empty() {
                return;
            }

            // Strictly cap each string to ~1024 bytes (slightly more than the 256 token limit of the model)
            // to definitively bound ONNX memory allocation prior to tokenization.
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
                v_index.upsert(*key, embedding);
            }
            *count += batch.len();
            batch.clear();
        };

    let progress_path = workspace.enhancing_progress_path();
    let paused_path = workspace.enhancing_paused_path();

    for row in rows.flatten() {
        if vector_index.contains(row.0) {
            progress_count += 1;
            continue;
        }

        batch.push(row);
        if batch.len() >= 16 {
            while let Some(reason) = check_system_constraints() {
                let _ = std::fs::write(&paused_path, &reason);
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            let _ = std::fs::remove_file(&paused_path);

            process_batch(&mut batch, &mut newly_processed, &mut vector_index);
            progress_count += 16;

            if progress_count % 256 == 0 {
                // Periodically update the human-readable progress file
                let _ = std::fs::write(&progress_path, progress_count.to_string());
            }

            if newly_processed % 8192 == 0 {
                // Periodically save to disk to support partial resumption
                // and immediate hybrid-search upgrades.
                let _ = vector_index.save();
            }
        }
    }

    // Process any remaining tail
    let tail_len = batch.len();
    process_batch(&mut batch, &mut newly_processed, &mut vector_index);
    progress_count += tail_len;

    let _ = std::fs::write(&progress_path, progress_count.to_string());
    vector_index.save()?;

    Ok(newly_processed)
}

fn build_indexed_chunk(chunk: Chunk) -> IndexedChunk {
    let vector_key = vector_key_from_content_hash(&chunk.content_hash);
    let kind = format!("{:?}", chunk.kind);

    IndexedChunk {
        chunk_id: chunk.id.to_string(),
        file_path: chunk.file_path,
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        language: chunk.language,
        kind,
        text: chunk.text,
        content_hash: chunk.content_hash,
        vector_key,
    }
}

fn vector_key_from_content_hash(content_hash: &str) -> u64 {
    let digest = xxhash_rust::xxh3::xxh3_128(content_hash.as_bytes()).to_le_bytes();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let mut value = u64::from_le_bytes(bytes);
    value &= i64::MAX as u64;
    value
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
    vector_index: &mut VectorStore,
    deleted: &[PathBuf],
) -> Result<()> {
    for rel_path in deleted {
        remove_file_chunks(sqlite, writer, fields, vector_index, rel_path)?;
    }
    Ok(())
}

fn remove_file_chunks(
    sqlite: &Connection,
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    vector_index: &mut VectorStore,
    rel_path: &Path,
) -> Result<()> {
    let rel_str = rel_path.to_string_lossy().to_string();
    let keys = chunk_vector_keys_for_file(sqlite, &rel_str)?;

    writer.delete_term(Term::from_field_text(fields.file_path, &rel_str));

    for key in keys {
        vector_index.remove(key);
    }

    sqlite.execute("DELETE FROM chunks WHERE file_path = ?1", params![rel_str])?;
    Ok(())
}

fn add_chunk_doc(
    writer: &mut tantivy::IndexWriter,
    fields: &TantivyFields,
    chunk: &IndexedChunk,
) -> Result<()> {
    writer.add_document(doc!(
        fields.chunk_id => chunk.chunk_id.clone(),
        fields.file_path => chunk.file_path.to_string_lossy().to_string(),
        fields.start_line => chunk.start_line as u64,
        fields.end_line => chunk.end_line as u64,
        fields.language => chunk.language.clone(),
        fields.kind => chunk.kind.clone(),
        fields.text => chunk.text.clone(),
        fields.content_hash => chunk.content_hash.clone()
    ))?;
    Ok(())
}

fn insert_chunk(conn: &Connection, chunk: &IndexedChunk) -> Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT OR REPLACE INTO chunks (
            chunk_id,
            file_path,
            start_line,
            end_line,
            language,
            kind,
            text,
            content_hash,
            vector_key,
            modified_unix
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    stmt.execute(params![
        chunk.chunk_id,
        chunk.file_path.to_string_lossy().to_string(),
        chunk.start_line as i64,
        chunk.end_line as i64,
        chunk.language,
        chunk.kind,
        compress_text(&chunk.text),
        chunk.content_hash,
        chunk.vector_key as i64,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
    ])?;
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
    Ok(conn)
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS chunks (
            chunk_id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            language TEXT NOT NULL,
            kind TEXT NOT NULL,
            text TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            vector_key INTEGER NOT NULL,
            modified_unix INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS _stats (
            key TEXT PRIMARY KEY,
            value INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_file_path ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_vector_key ON chunks(vector_key);
        CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
        "#,
    )?;
    Ok(())
}

fn build_schema() -> Schema {
    let mut schema = Schema::builder();
    schema.add_text_field("chunk_id", STRING | STORED);
    schema.add_text_field("file_path", STRING | STORED);
    schema.add_u64_field("start_line", STORED);
    schema.add_u64_field("end_line", STORED);
    schema.add_text_field("language", STRING | STORED);
    schema.add_text_field("kind", STRING | STORED);
    // Not STORED — full text lives in SQLite
    schema.add_text_field("text", TEXT);
    schema.add_text_field("content_hash", STRING | STORED);
    schema.build()
}

pub fn open_tantivy_index(path: &Path) -> Result<(TantivyIndex, TantivyFields)> {
    fs::create_dir_all(path)?;

    let schema = build_schema();
    let index = if path.join("meta.json").exists() {
        TantivyIndex::open_in_dir(path)?
    } else {
        TantivyIndex::create_in_dir(path, schema.clone())?
    };

    let schema = index.schema();
    let fields = TantivyFields {
        chunk_id: schema.get_field("chunk_id")?,
        file_path: schema.get_field("file_path")?,
        start_line: schema.get_field("start_line")?,
        end_line: schema.get_field("end_line")?,
        language: schema.get_field("language")?,
        kind: schema.get_field("kind")?,
        text: schema.get_field("text")?,
        content_hash: schema.get_field("content_hash")?,
    };

    Ok((index, fields))
}

pub fn fetch_chunk_by_vector_key(
    conn: &Connection,
    vector_key: u64,
) -> Result<Option<IndexedChunk>> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, file_path, start_line, end_line, language, kind, text, content_hash, vector_key
         FROM chunks
         WHERE vector_key = ?1
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![vector_key as i64])?;
    if let Some(row) = rows.next()? {
        let raw_text: Vec<u8> = row.get(6)?;
        let chunk = IndexedChunk {
            chunk_id: row.get::<_, String>(0)?,
            file_path: PathBuf::from(row.get::<_, String>(1)?),
            start_line: row.get::<_, i64>(2)? as usize,
            end_line: row.get::<_, i64>(3)? as usize,
            language: row.get(4)?,
            kind: row.get(5)?,
            text: decompress_text(raw_text),
            content_hash: row.get(7)?,
            vector_key: row.get::<_, i64>(8)? as u64,
        };

        return Ok(Some(chunk));
    }

    Ok(None)
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
    let chunk_id = search_doc
        .get_first(fields.chunk_id)
        .and_then(|v| v.as_str())?
        .to_string();

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

    let content_hash = search_doc
        .get_first(fields.content_hash)
        .and_then(|v| v.as_str())?
        .to_string();

    let vector_key = vector_key_from_content_hash(&content_hash);

    Some(IndexedChunk {
        chunk_id,
        file_path,
        start_line,
        end_line,
        language,
        kind,
        text,
        content_hash,
        vector_key,
    })
}

pub fn diff_for_workspace(workspace: &Workspace) -> Result<MerkleDiff> {
    let old_snapshot = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
    let new_snapshot = MerkleSnapshot::build(&workspace.root)?;
    Ok(old_snapshot.diff(&new_snapshot))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::embedding::HashEmbeddingModel;
    use crate::workspace::Workspace;

    use super::*;

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
        };
        std::fs::write(
            workspace.index_dir.join("workspace.json"),
            serde_json::to_string(&md_fixed).unwrap(),
        )
        .unwrap();
        assert!(workspace_is_indexed(&workspace));
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

        // Phase 2: enhance with neural (using hash as stand-in for ONNX in tests)
        let neural_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let enhanced = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert_eq!(enhanced, summary.total_chunks);

        // Verify neural vector store was created
        assert!(workspace.vector_neural_path().exists());

        // Verify the neural store has correct number of vectors
        let store = crate::vector_store::VectorStore::open(
            &workspace.vector_neural_path(),
            EMBEDDING_DIMENSIONS,
            crate::vector_store::ScalarKind::F32,
        )
        .unwrap();
        assert_eq!(store.size(), enhanced);
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

        let neural_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

        let n1 = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert!(n1 > 0, "first enhance should process chunks");
        let n2 = enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert_eq!(n2, 0, "second enhance should skip already-processed chunks");
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
