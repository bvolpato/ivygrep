use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tantivy::schema::{Field, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index as TantivyIndex, TantivyDocument, Term, doc};

use crate::chunking::{Chunk, chunk_source, is_indexable_file};
use crate::embedding::EmbeddingModel;
use crate::merkle::{MerkleDiff, MerkleSnapshot};
use crate::vector_store::VectorStore;
use crate::workspace::{Workspace, WorkspaceMetadata};

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
    workspace.metadata_path().exists()
        && workspace.sqlite_path().exists()
        && workspace.tantivy_dir().exists()
        && workspace.vector_path().exists()
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
    let vectors = VectorStore::open(&vector_path, embedding_dimensions)?;
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

    let result = index_workspace_inner(workspace, embedding_model);

    let _ = fs2::FileExt::unlock(&lock_file);
    result
}

fn index_workspace_inner(
    workspace: &Workspace,
    embedding_model: &dyn EmbeddingModel,
) -> Result<IndexingSummary> {
    let _ = open_storage(workspace, embedding_model.dimensions())?;

    let old_snapshot = MerkleSnapshot::load(&workspace.merkle_snapshot_path())?;
    let new_snapshot = MerkleSnapshot::build(&workspace.root)?;
    let diff = old_snapshot.diff(&new_snapshot);

    if diff.added_or_modified.is_empty()
        && diff.deleted.is_empty()
        && workspace_is_indexed(workspace)
    {
        return Ok(IndexingSummary {
            workspace_id: workspace.id.clone(),
            indexed_files: 0,
            deleted_files: 0,
            total_chunks: count_chunks(&workspace.sqlite_path())?,
        });
    }

    let mut sqlite = Connection::open(workspace.sqlite_path())?;
    create_tables(&sqlite)?;

    let (tantivy, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
    let mut writer = tantivy.writer(50_000_000)?;

    let mut vector_index =
        VectorStore::open(&workspace.vector_path(), embedding_model.dimensions())?;

    // Batch all SQLite writes in a single transaction for ~10-50x speedup.
    let tx = sqlite.transaction()?;

    apply_deletions(&tx, &mut writer, &fields, &mut vector_index, &diff.deleted)?;

    let total = diff.added_or_modified.len();
    let show_progress = total > 0 && std::io::stderr().is_terminal();
    let progress_counter = std::sync::atomic::AtomicUsize::new(0);

    let t0 = std::time::Instant::now();

    // Phase 1: Parallel read + chunk (truly parallel — no locks).
    if show_progress {
        eprint!("\r\x1b[K  reading & chunking...");
    }
    let file_chunks: Vec<_> = diff
        .added_or_modified
        .par_iter()
        .filter_map(|rel_path| {
            let abs_path = workspace.root.join(rel_path);
            if !abs_path.exists() {
                progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return None;
            }

            let content_bytes = fs::read(&abs_path).ok()?;
            if !is_indexable_file(rel_path, &content_bytes) {
                progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return None;
            }

            let content = match String::from_utf8(content_bytes) {
                Ok(text) => text,
                Err(err) => String::from_utf8_lossy(&err.into_bytes()).into_owned(),
            };

            let chunks = chunk_source(rel_path, &content);
            let indexed: Vec<_> = chunks.into_iter().map(build_indexed_chunk).collect();

            if show_progress {
                let n = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                eprint!("\r\x1b[K  [{}/{}] chunking...", n, total);
            }

            if indexed.is_empty() {
                return None;
            }
            Some((rel_path.clone(), indexed))
        })
        .collect();

    let t1 = std::time::Instant::now();

    // Phase 2: Batch embed ALL chunks in one call (no per-file mutex contention).
    let total_chunks: usize = file_chunks.iter().map(|(_, c)| c.len()).sum();
    if show_progress {
        eprint!(
            "\r\x1b[K  embedding {} chunks ({} files)...",
            total_chunks,
            file_chunks.len()
        );
    }

    let all_texts: Vec<&str> = file_chunks
        .iter()
        .flat_map(|(_, chunks)| chunks.iter().map(|c| c.text.as_str()))
        .collect();
    let all_embeddings = embedding_model.embed_batch(&all_texts);

    let t2 = std::time::Instant::now();

    // Phase 3: Sequential write to stores (fast — just DB inserts).
    if show_progress {
        eprint!("\r\x1b[K  writing index...");
    }
    let mut touched_files = HashSet::new();
    let mut embed_idx = 0;
    for (rel_path, indexed_chunks) in &file_chunks {
        touched_files.insert(rel_path.to_string_lossy().to_string());

        remove_file_chunks(&tx, &mut writer, &fields, &mut vector_index, rel_path)?;

        for indexed in indexed_chunks {
            let embedding = all_embeddings[embed_idx].clone();
            embed_idx += 1;
            vector_index.upsert(indexed.vector_key, embedding);
            insert_chunk(&tx, indexed)?;
            add_chunk_doc(&mut writer, &fields, indexed)?;
        }
    }

    let t3 = std::time::Instant::now();
    if total > 0 {
        eprint!(
            "\r\x1b[K  ✓ {} files, {} chunks — read {:.1}s, embed {:.1}s, write {:.1}s\n",
            file_chunks.len(),
            total_chunks,
            t1.duration_since(t0).as_secs_f64(),
            t2.duration_since(t1).as_secs_f64(),
            t3.duration_since(t2).as_secs_f64(),
        );
    }

    tx.commit()?;

    writer.commit()?;
    writer.wait_merging_threads()?;

    vector_index.save()?;

    new_snapshot.save(&workspace.merkle_snapshot_path())?;

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
        total_chunks: count_chunks(&workspace.sqlite_path())?,
    })
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
    let mut hasher = Sha256::new();
    hasher.update(content_hash.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let mut value = u64::from_le_bytes(bytes);
    value &= i64::MAX as u64;
    value
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
        chunk.text,
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
    let conn = Connection::open(sqlite_path)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    Ok(count as usize)
}

pub fn open_sqlite(sqlite_path: &Path) -> Result<Connection> {
    let conn = Connection::open(sqlite_path)?;
    create_tables(&conn)?;
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
    schema.add_text_field("text", TEXT | STORED);
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
        let chunk = IndexedChunk {
            chunk_id: row.get::<_, String>(0)?,
            file_path: PathBuf::from(row.get::<_, String>(1)?),
            start_line: row.get::<_, i64>(2)? as usize,
            end_line: row.get::<_, i64>(3)? as usize,
            language: row.get(4)?,
            kind: row.get(5)?,
            text: row.get(6)?,
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

    let text = search_doc
        .get_first(fields.text)
        .and_then(|v| v.as_str())?
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
}
