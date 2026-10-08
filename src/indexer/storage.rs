use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, Error as SqliteError};
use tantivy::Index as TantivyIndex;
use tantivy::directory::error::{DeleteError, LockError, OpenReadError, OpenWriteError};
use tantivy::directory::{
    Directory, DirectoryLock, FileHandle, Lock, MmapDirectory, WatchCallback, WatchHandle, WritePtr,
};
use tantivy::schema::{
    FAST, Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
};

use crate::text::{
    CODE_TOKENIZER_NAME, TRIGRAM_TOKENIZER_NAME, build_code_analyzer, build_trigram_analyzer,
};
use crate::vector_store::{ScalarKind, VectorStore, VectorTier};
use crate::workspace::Workspace;

const TANTIVY_WRITE_RETRY_ATTEMPTS: u32 = 16;
const TANTIVY_WRITE_RETRY_MAX_DELAY_MS: u64 = 800;

#[derive(Debug, Clone)]
pub struct TantivyFields {
    pub vector_key: Field,
    pub file_path: Field,
    pub start_line: Field,
    pub end_line: Field,
    pub language: Field,
    pub kind: Field,
    pub text: Field,
    pub text_trigrams: Option<Field>,
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

#[derive(Debug, Clone)]
struct RetryingDirectory<D> {
    inner: D,
    #[cfg(test)]
    commit_failure: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl<D> RetryingDirectory<D> {
    fn new(inner: D) -> Self {
        Self {
            inner,
            #[cfg(test)]
            commit_failure: None,
        }
    }
}

impl<D> Directory for RetryingDirectory<D>
where
    D: Directory + Clone + std::fmt::Debug,
{
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> std::result::Result<std::sync::Arc<dyn FileHandle>, OpenReadError> {
        self.inner.get_file_handle(path)
    }

    fn delete(&self, path: &Path) -> std::result::Result<(), DeleteError> {
        self.inner.delete(path)
    }

    fn exists(&self, path: &Path) -> std::result::Result<bool, OpenReadError> {
        self.inner.exists(path)
    }

    fn open_write(&self, path: &Path) -> std::result::Result<WritePtr, OpenWriteError> {
        open_write_with_retry(|| self.inner.open_write(path))
    }

    fn atomic_read(&self, path: &Path) -> std::result::Result<Vec<u8>, OpenReadError> {
        self.inner.atomic_read(path)
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        #[cfg(test)]
        if path == Path::new("meta.json")
            && self
                .commit_failure
                .as_ref()
                .is_some_and(|enabled| enabled.load(std::sync::atomic::Ordering::SeqCst))
            && matches!(self.inner.exists(path), Ok(true))
        {
            return Err(std::io::Error::other(
                "injected Tantivy metadata publication failure",
            ));
        }
        self.inner.atomic_write(path, data)
    }

    fn sync_directory(&self) -> std::io::Result<()> {
        self.inner.sync_directory()
    }

    fn acquire_lock(&self, lock: &Lock) -> std::result::Result<DirectoryLock, LockError> {
        self.inner.acquire_lock(lock)
    }

    fn watch(&self, watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        self.inner.watch(watch_callback)
    }
}

fn open_write_with_retry<F>(mut open: F) -> std::result::Result<WritePtr, OpenWriteError>
where
    F: FnMut() -> std::result::Result<WritePtr, OpenWriteError>,
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
            Err(error) => return Err(error),
        }
    }
    unreachable!("retry loop always returns on its final attempt")
}

#[cfg(not(target_os = "macos"))]
type IndexDirectory = MmapDirectory;
#[cfg(target_os = "macos")]
type IndexDirectory = BarrierSyncDirectory;

#[cfg(not(target_os = "macos"))]
fn open_index_directory(path: &Path) -> Result<IndexDirectory> {
    Ok(MmapDirectory::open(path)?)
}

#[cfg(target_os = "macos")]
fn open_index_directory(path: &Path) -> Result<IndexDirectory> {
    BarrierSyncDirectory::open(path)
}

/// Tantivy ends every segment file and every atomic write with
/// `File::sync_data`. On macOS that call is `F_FULLFSYNC`, which waits until
/// the drive has emptied its cache. One commit issued about 30 of them.
///
/// This directory writes the same files in the same order, but ends each
/// write with `F_BARRIERFSYNC`. A barrier keeps the order of writes on the
/// drive and does not wait for the cache. `sync_directory` stays a full flush.
/// Tantivy calls it before it publishes `meta.json`, so the segments that a
/// published `meta.json` names reached stable storage first.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct BarrierSyncDirectory {
    inner: MmapDirectory,
    root: PathBuf,
    #[cfg(test)]
    syncs: std::sync::Arc<SyncCounts>,
}

#[cfg(all(test, target_os = "macos"))]
#[derive(Debug, Default)]
struct SyncCounts {
    barrier: std::sync::atomic::AtomicUsize,
    full: std::sync::atomic::AtomicUsize,
}

#[cfg(target_os = "macos")]
impl BarrierSyncDirectory {
    fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            inner: MmapDirectory::open(path)?,
            // MmapDirectory resolves relative names against the canonical path.
            root: path.canonicalize()?,
            #[cfg(test)]
            syncs: Default::default(),
        })
    }

    /// Write the file's data to the drive and order it before later writes.
    fn barrier_sync(&self, file: &fs::File) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;

        #[cfg(test)]
        self.syncs
            .barrier
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // SAFETY: the descriptor stays open for the call, and this command
        // takes no argument.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_BARRIERFSYNC) } == 0 {
            return Ok(());
        }
        // Some file systems have no barrier. Use the full flush that Tantivy
        // issues itself.
        file.sync_data()
    }
}

#[cfg(target_os = "macos")]
struct BarrierFileWriter {
    file: fs::File,
    directory: BarrierSyncDirectory,
}

#[cfg(target_os = "macos")]
impl std::io::Write for BarrierFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl tantivy::directory::TerminatingWrite for BarrierFileWriter {
    fn terminate_ref(&mut self, _: tantivy::directory::AntiCallToken) -> std::io::Result<()> {
        self.directory.barrier_sync(&self.file)
    }
}

#[cfg(target_os = "macos")]
impl Directory for BarrierSyncDirectory {
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> std::result::Result<std::sync::Arc<dyn FileHandle>, OpenReadError> {
        self.inner.get_file_handle(path)
    }

    fn delete(&self, path: &Path) -> std::result::Result<(), DeleteError> {
        self.inner.delete(path)
    }

    fn exists(&self, path: &Path) -> std::result::Result<bool, OpenReadError> {
        self.inner.exists(path)
    }

    fn open_write(&self, path: &Path) -> std::result::Result<WritePtr, OpenWriteError> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(path))
            .map_err(|io_error| {
                if io_error.kind() == std::io::ErrorKind::AlreadyExists {
                    OpenWriteError::FileAlreadyExists(path.to_path_buf())
                } else {
                    OpenWriteError::wrap_io_error(io_error, path.to_path_buf())
                }
            })?;
        Ok(std::io::BufWriter::new(Box::new(BarrierFileWriter {
            file,
            directory: self.clone(),
        })))
    }

    fn atomic_read(&self, path: &Path) -> std::result::Result<Vec<u8>, OpenReadError> {
        self.inner.atomic_read(path)
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;

        // Names that start with a dot are not managed by Tantivy, as its own
        // temporary files are not.
        let temporary = self
            .root
            .join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
        let written = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .and_then(|mut file| {
                file.write_all(data)?;
                self.barrier_sync(&file)
            })
            .and_then(|()| fs::rename(&temporary, self.root.join(path)));
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written
    }

    fn sync_directory(&self) -> std::io::Result<()> {
        #[cfg(test)]
        self.syncs
            .full
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.sync_directory()
    }

    fn acquire_lock(&self, lock: &Lock) -> std::result::Result<DirectoryLock, LockError> {
        self.inner.acquire_lock(lock)
    }

    fn watch(&self, watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        self.inner.watch(watch_callback)
    }
}

pub fn open_storage(workspace: &Workspace, embedding_dimensions: usize) -> Result<StorageHandles> {
    open_storage_with_options(workspace, embedding_dimensions, true)
}

pub(super) fn open_storage_with_options(
    workspace: &Workspace,
    embedding_dimensions: usize,
    create_secondary_indexes: bool,
) -> Result<StorageHandles> {
    workspace.ensure_dirs()?;
    fs::create_dir_all(workspace.tantivy_dir())?;

    let sqlite_path = workspace.sqlite_path();
    let conn = Connection::open(&sqlite_path)?;
    create_tables_with_options(&conn, create_secondary_indexes)?;
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

pub(super) fn ensure_hash_vector_store(path: &Path, embedding_dimensions: usize) -> Result<()> {
    if path.exists() {
        let _ = VectorStore::open_readonly(
            path,
            embedding_dimensions,
            ScalarKind::F16,
            VectorTier::Hash,
        )?;
    } else {
        VectorStore::open(
            path,
            embedding_dimensions,
            ScalarKind::F16,
            VectorTier::Hash,
        )?
        .save()?;
    }
    Ok(())
}

pub fn open_sqlite(sqlite_path: &Path) -> Result<Connection> {
    let conn = Connection::open(sqlite_path)?;
    create_tables(&conn)?;
    Ok(conn)
}

/// Open SQLite in read-only mode for search and status queries.
pub fn open_sqlite_readonly(sqlite_path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        sqlite_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch(
        "PRAGMA mmap_size = 2147483648;
         PRAGMA cache_size = -65536;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(conn)
}

pub(super) fn create_tables(conn: &Connection) -> Result<()> {
    create_tables_with_options(conn, true)
}

pub(super) fn create_tables_with_options(conn: &Connection, create_indexes: bool) -> Result<()> {
    apply_default_write_pragmas(conn)?;
    create_tables_schema(conn, create_indexes)
}

pub(super) fn apply_default_write_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

pub(super) fn apply_bulk_write_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -16000;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(())
}

pub(super) fn apply_fresh_staging_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         PRAGMA locking_mode = EXCLUSIVE;
         PRAGMA cache_size = -64000;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(())
}

pub(super) fn create_tables_schema(conn: &Connection, create_indexes: bool) -> Result<()> {
    conn.execute_batch(
        r#"
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
            name TEXT,
            owner TEXT,
            PRIMARY KEY (normalized_name, chunk_key)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS included_file_dependencies (
            owner_path TEXT NOT NULL,
            included_path TEXT NOT NULL,
            PRIMARY KEY (owner_path, included_path)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS file_edges (
            source_path TEXT NOT NULL,
            target_path TEXT NOT NULL,
            kind INTEGER NOT NULL,
            PRIMARY KEY (source_path, target_path, kind)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS unresolved_file_dependencies (
            source_path TEXT NOT NULL,
            language TEXT NOT NULL,
            spec TEXT NOT NULL,
            lookup_key TEXT NOT NULL,
            PRIMARY KEY (source_path, language, spec, lookup_key)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS resolved_file_dependencies (
            source_path TEXT NOT NULL,
            language TEXT NOT NULL,
            spec TEXT NOT NULL,
            lookup_key TEXT NOT NULL,
            target_path TEXT NOT NULL,
            PRIMARY KEY (source_path, language, spec, lookup_key)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS manifest_resolution_signatures (
            file_path TEXT PRIMARY KEY,
            signature TEXT NOT NULL
        ) WITHOUT ROWID;
        "#,
    )?;
    if create_indexes {
        create_secondary_indexes(conn)?;
    }

    add_is_ignored_column(conn)?;
    migrate_legacy_symbols_table(conn)?;
    if create_indexes {
        create_symbol_indexes(conn)?;
    }
    Ok(())
}

fn add_is_ignored_column(conn: &Connection) -> Result<()> {
    match conn.execute(
        "ALTER TABLE chunks ADD COLUMN is_ignored INTEGER NOT NULL DEFAULT 0;",
        [],
    ) {
        Ok(_) => Ok(()),
        Err(error) if is_duplicate_is_ignored_column(&error) => Ok(()),
        Err(error) => Err(error).context("failed to add chunks.is_ignored column"),
    }
}

fn is_duplicate_is_ignored_column(error: &SqliteError) -> bool {
    matches!(
        error,
        SqliteError::SqliteFailure(_, Some(message))
            if message.eq_ignore_ascii_case("duplicate column name: is_ignored")
    )
}

/// Upgrades pre-v22 `symbols` tables (single name per chunk, or name-only
/// rows) to the current layout. Legacy rows carry no display name or owner
/// (readers fall back to the normalized name); the format bump rebuilds them
/// with parser-derived names on the next index.
fn migrate_legacy_symbols_table(conn: &Connection) -> Result<()> {
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
    let missing_symbol_metadata = !columns.iter().any(|(name, _)| name == "name");
    drop(table_info);

    if legacy_single_symbol_schema || missing_symbol_metadata {
        conn.execute_batch(
            r#"
            BEGIN IMMEDIATE;
            DROP TABLE IF EXISTS symbols_legacy;
            ALTER TABLE symbols RENAME TO symbols_legacy;
            CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                name TEXT,
                owner TEXT,
                PRIMARY KEY (normalized_name, chunk_key)
            ) WITHOUT ROWID;
            INSERT OR IGNORE INTO symbols (normalized_name, chunk_key, name, owner)
                SELECT normalized_name, chunk_key, NULL, NULL FROM symbols_legacy;
            DROP TABLE symbols_legacy;
            COMMIT;
            "#,
        )?;
    }
    Ok(())
}

/// Symbol rows are keyed by name; file removal needs the reverse lookup.
/// Fresh indexes defer this with the other secondary indexes.
pub(super) fn create_symbol_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_symbols_chunk_key ON symbols(chunk_key);")?;
    Ok(())
}

pub(super) fn create_secondary_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_chunks_file_path ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_vector_key ON chunks(vector_key);
        CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
        CREATE INDEX IF NOT EXISTS idx_included_file_dependencies_path
            ON included_file_dependencies(included_path);
        CREATE INDEX IF NOT EXISTS idx_file_edges_target
            ON file_edges(target_path, source_path);
        CREATE INDEX IF NOT EXISTS idx_resolved_file_dependencies_lookup
            ON resolved_file_dependencies(lookup_key, source_path);
        CREATE INDEX IF NOT EXISTS idx_unresolved_file_dependencies_lookup
            ON unresolved_file_dependencies(lookup_key, source_path);
        "#,
    )?;
    create_symbol_indexes(conn)
}

pub(super) fn finalize_graph_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch("DROP INDEX IF EXISTS idx_symbols_name;")?;
    Ok(())
}

fn build_schema() -> Schema {
    let code_indexing = TextFieldIndexing::default()
        .set_tokenizer(CODE_TOKENIZER_NAME)
        .set_index_option(IndexRecordOption::WithFreqs);
    let code_text_opts = TextOptions::default().set_indexing_options(code_indexing.clone());
    let trigram_indexing = TextFieldIndexing::default()
        .set_tokenizer(TRIGRAM_TOKENIZER_NAME)
        .set_index_option(IndexRecordOption::Basic);
    let trigram_text_opts = TextOptions::default().set_indexing_options(trigram_indexing);
    let boosted_aux_indexing = TextFieldIndexing::default()
        .set_tokenizer(CODE_TOKENIZER_NAME)
        .set_index_option(IndexRecordOption::Basic);
    let boosted_aux_text_opts = TextOptions::default().set_indexing_options(boosted_aux_indexing);

    let mut schema = Schema::builder();
    // A fast column lets candidate collection order equal scores by chunk key
    // without reading stored documents.
    schema.add_u64_field("vector_key", STORED | FAST);
    schema.add_text_field("file_path", STRING | STORED);
    schema.add_u64_field("start_line", STORED);
    schema.add_u64_field("end_line", STORED);
    schema.add_text_field("language", STRING | STORED);
    schema.add_text_field("kind", STRING | STORED);
    schema.add_text_field("text", code_text_opts.clone());
    schema.add_text_field("text_trigrams", trigram_text_opts);
    schema.add_u64_field("is_ignored", STORED);
    schema.add_text_field("file_path_text", boosted_aux_text_opts.clone());
    schema.add_text_field("signature", boosted_aux_text_opts);
    schema.build()
}

pub fn open_tantivy_index(path: &Path) -> Result<(TantivyIndex, TantivyFields)> {
    fs::create_dir_all(path)?;

    let schema = build_schema();
    let directory = RetryingDirectory::new(open_index_directory(path)?);
    #[cfg(test)]
    let directory = RetryingDirectory {
        commit_failure: test_support::commit_failure(path),
        ..directory
    };
    let index = if path.join("meta.json").exists() {
        TantivyIndex::open(directory)?
    } else {
        TantivyIndex::open_or_create(directory, schema)?
    };

    index
        .tokenizers()
        .register(CODE_TOKENIZER_NAME, build_code_analyzer());
    index
        .tokenizers()
        .register(TRIGRAM_TOKENIZER_NAME, build_trigram_analyzer());

    let schema = index.schema();
    let fields = TantivyFields {
        vector_key: schema.get_field("vector_key")?,
        file_path: schema.get_field("file_path")?,
        start_line: schema.get_field("start_line")?,
        end_line: schema.get_field("end_line")?,
        language: schema.get_field("language")?,
        kind: schema.get_field("kind")?,
        text: schema.get_field("text")?,
        text_trigrams: schema.get_field("text_trigrams").ok(),
        is_ignored: schema.get_field("is_ignored").ok(),
        file_path_text: schema.get_field("file_path_text").ok(),
        signature: schema.get_field("signature").ok(),
    };

    Ok((index, fields))
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock};

    use parking_lot::Mutex;

    use super::*;

    static FAILED_COMMITS: LazyLock<Mutex<HashMap<PathBuf, Arc<AtomicBool>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    pub(crate) struct CommitFailure {
        path: PathBuf,
        enabled: Arc<AtomicBool>,
    }

    impl Drop for CommitFailure {
        fn drop(&mut self) {
            self.enabled.store(false, Ordering::SeqCst);
            FAILED_COMMITS.lock().remove(&self.path);
        }
    }

    pub(crate) fn fail_tantivy_commits(path: &Path) -> CommitFailure {
        let enabled = Arc::new(AtomicBool::new(true));
        assert!(
            FAILED_COMMITS
                .lock()
                .insert(path.to_owned(), enabled.clone())
                .is_none()
        );
        CommitFailure {
            path: path.to_owned(),
            enabled,
        }
    }

    pub(super) fn commit_failure(path: &Path) -> Option<Arc<AtomicBool>> {
        let failures = FAILED_COMMITS.lock();
        path.ancestors()
            .find_map(|ancestor| failures.get(ancestor).cloned())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tantivy::directory::{Directory, RamDirectory, TerminatingWrite};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn segment_writes_retry_transient_permission_denials() {
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
    fn segment_writes_do_not_retry_other_errors() {
        let attempts = std::cell::Cell::new(0);
        let path = PathBuf::from("segment.term");
        let result = open_write_with_retry(|| {
            attempts.set(attempts.get() + 1);
            Err(OpenWriteError::FileAlreadyExists(path.clone()))
        });

        assert!(matches!(result, Err(OpenWriteError::FileAlreadyExists(_))));
        assert_eq!(attempts.get(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn barrier_directory_keeps_tantivy_write_contracts() {
        use std::io::Write;

        let temp = tempdir().unwrap();
        let directory = BarrierSyncDirectory::open(temp.path()).unwrap();
        let segment = Path::new("segment.term");

        let mut writer = directory.open_write(segment).unwrap();
        writer.write_all(b"postings").unwrap();
        writer.terminate().unwrap();
        assert_eq!(directory.atomic_read(segment).unwrap(), b"postings");
        assert!(matches!(
            directory.open_write(segment),
            Err(OpenWriteError::FileAlreadyExists(_))
        ));

        let meta = Path::new("meta.json");
        directory.atomic_write(meta, b"first").unwrap();
        directory.atomic_write(meta, b"second").unwrap();
        assert_eq!(directory.atomic_read(meta).unwrap(), b"second");

        let mut names = fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["meta.json", "segment.term"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn commit_orders_segment_writes_and_flushes_once_before_publication() {
        use std::sync::atomic::Ordering;

        let temp = tempdir().unwrap();
        let directory = BarrierSyncDirectory::open(temp.path()).unwrap();
        let mut schema = Schema::builder();
        let text = schema.add_text_field("text", tantivy::schema::TEXT);
        let index =
            TantivyIndex::open_or_create(RetryingDirectory::new(directory.clone()), schema.build())
                .unwrap();
        let mut writer = index
            .writer_with_num_threads::<tantivy::TantivyDocument>(1, 20_000_000)
            .unwrap();
        writer.add_document(tantivy::doc!(text => "first")).unwrap();
        writer.commit().unwrap();

        let barriers = directory.syncs.barrier.load(Ordering::SeqCst);
        let full = directory.syncs.full.load(Ordering::SeqCst);
        writer
            .add_document(tantivy::doc!(text => "second"))
            .unwrap();
        writer.commit().unwrap();

        // One full flush makes the new segment durable before `meta.json`
        // names it. Every segment file and atomic write ends with a barrier.
        assert_eq!(directory.syncs.full.load(Ordering::SeqCst) - full, 1);
        assert!(directory.syncs.barrier.load(Ordering::SeqCst) - barriers >= 5);

        let reader = index.reader().unwrap();
        assert_eq!(reader.searcher().num_docs(), 2);
    }

    #[test]
    fn symbols_migrate_to_many_names_per_chunk() {
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
            "INSERT INTO symbols (normalized_name, chunk_key, name, owner)
             VALUES (?1, ?2, NULL, NULL)",
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
    fn name_only_symbols_gain_metadata_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
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
             INSERT INTO chunks VALUES (7, 'src/router.rs', 1, 2, 'rust', 'Class', '', 7, 0, 0);
             CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;
             INSERT INTO symbols (normalized_name, chunk_key) VALUES ('router', 7);",
        )
        .unwrap();

        create_tables(&conn).unwrap();

        let row = conn
            .query_row(
                "SELECT normalized_name, name, owner FROM symbols WHERE chunk_key = 7",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row, ("router".to_string(), None, None));
    }

    #[test]
    fn duplicate_column_is_the_only_suppressed_migration_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite3");
        let conn = Connection::open(&path).unwrap();
        create_tables_schema(&conn, false).unwrap();
        conn.execute("ALTER TABLE chunks DROP COLUMN is_ignored", [])
            .unwrap();
        drop(conn);

        let readonly = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        let error = create_tables_schema(&readonly, false).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to add chunks.is_ignored column")
        );
    }

    #[test]
    fn graph_finalization_removes_redundant_symbol_index() {
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

    #[test]
    fn table_creation_can_defer_secondary_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        create_tables_with_options(&conn, false).unwrap();

        let count_indexes = |conn: &Connection| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index'
                   AND name IN (
                     'idx_chunks_file_path',
                     'idx_chunks_vector_key',
                     'idx_chunks_language',
                     'idx_included_file_dependencies_path'
                   )",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count_indexes(&conn), 0);

        create_secondary_indexes(&conn).unwrap();

        assert_eq!(count_indexes(&conn), 4);
    }
}
