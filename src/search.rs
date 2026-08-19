use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::Connection;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::{
    BooleanQuery, BoostQuery, Occur, Query, QueryParser, RegexQuery, TermQuery, TermSetQuery,
};
use tantivy::schema::IndexRecordOption;
use tantivy::tokenizer::TokenStream;

use crate::embedding::EmbeddingModel;
use crate::indexer::{
    IndexedChunk, fetch_chunk_by_id, fetch_chunk_by_vector_key,
    fetch_chunk_metadata_by_vector_keys_batch, fetch_chunk_texts_by_vector_keys_batch,
    fetch_chunks_by_vector_keys_batch, open_sqlite_readonly, open_tantivy_index,
    reconcile_worktree_overlay,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
#[path = "search_execution.rs"]
mod execution;
#[path = "search_fusion.rs"]
mod fusion;
#[path = "search_presentation.rs"]
mod presentation;

use crate::search_routing::{
    QueryIntent, QueryRouting, corpus_candidate_multiplier, neural_fallback_needed, raw_query_terms,
};
use crate::text::{build_code_analyzer, singularize_token, split_identifier_segments};
use crate::vector_store::{
    HASH_VECTOR_QUANTIZATION, NEURAL_VECTOR_QUANTIZATION, VectorMatch, VectorStore,
};
use crate::workspace::{Workspace, WorkspaceScope, index_path_string};
pub(crate) use execution::hybrid_search_with_context_and_neural_job;
use fusion::fuse_rrf_with_context;
use presentation::{
    HitPresentation, LineSpan, PresentationQuery, line_spans,
    should_use_compact_identifier_matching, snippet_bounds, to_hit,
};
#[cfg(test)]
use presentation::{find_focus_line, line_at};

pub(crate) const DEFAULT_SEARCH_LIMIT: usize = 50;
pub const MAX_SEARCH_CONTEXT_LINES: usize = 100;
pub const MAX_SEARCH_RESULT_LIMIT: usize = 1_000;

#[derive(Debug, Clone)]
pub struct RawIndexedChunk {
    pub chunk_id: String,
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub kind: String,
    pub raw_text: Vec<u8>,
    pub content_hash: String,
    pub vector_key: u64,
    pub is_ignored: bool,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: Option<usize>,
    pub context: usize,
    pub type_filter: Option<String>,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub scope_filter: Option<WorkspaceScope>,
    pub skip_gitignore: bool,
    pub force_neural: bool,
    pub progress_tx: Option<std::sync::mpsc::Sender<(String, usize, usize)>>,
    /// When set to `true`, the search should bail out as soon as possible.
    pub cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

pub fn validate_forced_neural_workspaces(
    workspaces: &[Workspace],
    force_neural: bool,
) -> Result<()> {
    if !force_neural {
        return Ok(());
    }

    let identities = workspaces
        .iter()
        .map(|workspace| (workspace, workspace_neural_model_identity(workspace)))
        .collect::<Vec<_>>();
    let missing = identities
        .iter()
        .filter(|(_, identity)| identity.is_none())
        .map(|(workspace, _)| workspace.root.display().to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        anyhow::bail!(
            "neural search was required, but these workspaces have no neural vectors: {}",
            missing.join(", ")
        );
    }

    let expected_identity = crate::embedding::configured_neural_model_identity();
    let incompatible = identities
        .iter()
        .filter(|(_, identity)| identity.as_ref() != Some(&expected_identity))
        .map(|(workspace, _)| workspace.root.display().to_string())
        .collect::<Vec<_>>();
    if !incompatible.is_empty() {
        anyhow::bail!(
            "neural search was required, but these workspaces use an incompatible neural model: {}",
            incompatible.join(", ")
        );
    }
    Ok(())
}

pub(crate) fn workspace_neural_model_identity(
    workspace: &Workspace,
) -> Option<crate::embedding::NeuralModelIdentity> {
    neural_model_identity_from_index_dir(&workspace.index_dir)
        .or_else(|| neural_model_identity_from_index_dir(workspace.base_index_dir.as_ref()?))
}

fn neural_model_identity_from_index_dir(
    index_dir: &Path,
) -> Option<crate::embedding::NeuralModelIdentity> {
    let contents = fs::read_to_string(index_dir.join("neural_model.json")).ok()?;
    let identity = serde_json::from_str::<crate::embedding::NeuralModelIdentity>(&contents).ok()?;
    let store = VectorStore::open_readonly(
        &index_dir.join("vectors_neural.usearch"),
        identity.dimensions,
        NEURAL_VECTOR_QUANTIZATION,
    )
    .ok()?;
    (store.size() > 0).then_some(identity)
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: None,
            context: 2,
            type_filter: None,
            include_globs: vec![],
            exclude_globs: vec![],
            scope_filter: None,
            skip_gitignore: false,
            force_neural: false,
            progress_tx: None,
            cancel_token: None,
        }
    }
}

impl SearchOptions {
    /// Return the language filter in the canonical form stored by the index.
    ///
    /// Search options can come from raw Web/MCP requests, which accept file
    /// extensions and aliases (for example, `rs`). Unknown filters stay
    /// unchanged so they retain their existing no-match behavior.
    pub(crate) fn canonical_type_filter(&self) -> Option<String> {
        self.type_filter.as_deref().map(|filter| {
            crate::chunking::resolve_type_alias(filter)
                .map(str::to_string)
                .unwrap_or_else(|| filter.to_string())
        })
    }

    pub fn bounded_limit(&self) -> Option<usize> {
        self.limit.map(|limit| {
            if limit == usize::MAX {
                usize::MAX
            } else {
                limit.min(MAX_SEARCH_RESULT_LIMIT)
            }
        })
    }

    pub fn bounded_context(&self) -> usize {
        self.context.min(MAX_SEARCH_CONTEXT_LINES)
    }

    /// Returns `true` when the caller has requested cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|t| t.load(std::sync::atomic::Ordering::Relaxed))
    }
}

use crate::indexer::TantivyFields;

pub struct SearchContext {
    pub sqlite: Connection,
    pub base_sqlite: Option<Connection>,

    pub indexes: Vec<tantivy::Index>,
    pub searchers: Vec<tantivy::Searcher>,
    pub fields: TantivyFields,

    pub hash_vectors: Option<VectorStore>,
    pub base_hash_vectors: Option<VectorStore>,
    pub neural_vectors: Option<VectorStore>,
    pub base_neural_vectors: Option<VectorStore>,
    pub neural_profile: Option<String>,
    pub base_neural_profile: Option<String>,
    pub neural_model: Option<crate::embedding::NeuralModelIdentity>,
    pub base_neural_model: Option<crate::embedding::NeuralModelIdentity>,

    pub tombstones: HashSet<String>,
    pub overlay_files: HashSet<String>,
    file_contents: RefCell<HashMap<PathBuf, CachedFileContent>>,
    glob_path_filters: RefCell<HashMap<GlobPathFilterCacheKey, GlobPathQueryFilter>>,
}

#[derive(Clone)]
struct CachedFileContent {
    len: u64,
    modified_nanos: u128,
    content: Arc<str>,
    lines: Arc<[LineSpan]>,
}

type SemanticCandidatesById = HashMap<u64, (IndexedChunk, f32, HashSet<&'static str>)>;

pub(crate) enum NeuralQueryVectorJob {
    Ready(Vec<f32>),
    Pending(Option<std::thread::JoinHandle<Vec<f32>>>),
}

impl NeuralQueryVectorJob {
    pub(crate) fn pending(handle: std::thread::JoinHandle<Vec<f32>>) -> Self {
        Self::Pending(Some(handle))
    }

    fn finish(&mut self) -> Option<Vec<f32>> {
        match self {
            Self::Ready(vector) => Some(std::mem::take(vector)),
            Self::Pending(handle) => match handle.take()?.join() {
                Ok(vector) => Some(vector),
                Err(_) => {
                    tracing::warn!("precomputed neural query vector task panicked");
                    None
                }
            },
        }
    }
}

impl Drop for NeuralQueryVectorJob {
    fn drop(&mut self) {
        let Self::Pending(handle) = self else {
            return;
        };
        let Some(handle) = handle.take() else {
            return;
        };
        if handle.join().is_err() {
            tracing::warn!("precomputed neural query vector task panicked");
        }
    }
}

fn open_optional_vector_store(
    enabled: bool,
    path: &Path,
    dimensions: usize,
    quantization: crate::vector_store::ScalarKind,
) -> Result<Option<VectorStore>> {
    if !enabled || !path.exists() {
        return Ok(None);
    }
    VectorStore::open_readonly(path, dimensions, quantization)
        .with_context(|| format!("open vector store {}", path.display()))
        .map(Some)
}

fn read_optional_neural_identity(
    path: &Path,
) -> Result<Option<crate::embedding::NeuralModelIdentity>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("read neural model metadata {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("parse neural model metadata {}", path.display()))
        .map(Some)
}

fn read_optional_profile(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let profile = fs::read_to_string(path)
        .with_context(|| format!("read neural profile {}", path.display()))?;
    Ok((!profile.trim().is_empty()).then(|| profile.trim().to_string()))
}

impl SearchContext {
    pub fn load(
        workspace: &Workspace,
        emb_dim: Option<usize>,
        wants_neural_vectors: bool,
    ) -> Result<Self> {
        anyhow::ensure!(
            !workspace.worktree_overlay_is_stale()?,
            "worktree overlay is stale for {}",
            workspace.root.display()
        );
        let wants_hash_vectors = emb_dim.is_some();
        let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
        if use_overlay {
            let overlay_sqlite = open_sqlite_readonly(&workspace.overlay_sqlite_path())?;
            let (overlay_idx, fields) = open_tantivy_index(&workspace.overlay_tantivy_dir())?;
            let overlay_reader = overlay_idx.reader()?;
            let overlay_searcher = overlay_reader.searcher();
            let overlay_hash_vec = open_optional_vector_store(
                wants_hash_vectors,
                &workspace.overlay_vector_path(),
                256,
                HASH_VECTOR_QUANTIZATION,
            )?;

            let base_dir = workspace
                .base_index_dir
                .clone()
                .unwrap_or_else(|| workspace.index_dir.clone());
            let base_sqlite = open_sqlite_readonly(&base_dir.join("metadata.sqlite3"))?;
            let (base_idx, _) = open_tantivy_index(&base_dir.join("tantivy"))?;
            let base_reader = base_idx.reader()?;
            let base_searcher = base_reader.searcher();
            let base_hash_vec = open_optional_vector_store(
                wants_hash_vectors,
                &base_dir.join("vectors.usearch"),
                256,
                HASH_VECTOR_QUANTIZATION,
            )?;
            let base_neural_model =
                read_optional_neural_identity(&base_dir.join("neural_model.json"))?;
            let base_neural_dimensions = base_neural_model
                .as_ref()
                .map_or(384, |identity: &crate::embedding::NeuralModelIdentity| {
                    identity.dimensions
                });
            let base_neural_vec = open_optional_vector_store(
                wants_neural_vectors,
                &base_dir.join("vectors_neural.usearch"),
                base_neural_dimensions,
                NEURAL_VECTOR_QUANTIZATION,
            )?;
            let base_neural_profile = read_optional_profile(&base_dir.join("neural_profile"))?;
            let overlay_neural_model =
                read_optional_neural_identity(&workspace.neural_model_path())?;
            let overlay_neural_dimensions = overlay_neural_model
                .as_ref()
                .map_or(base_neural_dimensions, |identity| identity.dimensions);
            if let (Some(overlay), Some(base)) = (&overlay_neural_model, &base_neural_model) {
                anyhow::ensure!(
                    overlay == base,
                    "worktree neural model does not match the base workspace"
                );
            }
            let overlay_neural_vec = open_optional_vector_store(
                wants_neural_vectors,
                &workspace.vector_neural_path(),
                overlay_neural_dimensions,
                NEURAL_VECTOR_QUANTIZATION,
            )?;
            let overlay_neural_profile = read_optional_profile(&workspace.neural_profile_path())?;

            let mut tombstones = HashSet::new();
            let mut overlay_files = HashSet::new();
            {
                let mut stmt = overlay_sqlite.prepare("SELECT file_path FROM tombstones")?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    tombstones.insert(row.get(0)?);
                }

                let mut stmt = overlay_sqlite.prepare("SELECT DISTINCT file_path FROM chunks")?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    overlay_files.insert(row.get(0)?);
                }
            }

            Ok(Self {
                sqlite: overlay_sqlite,
                base_sqlite: Some(base_sqlite),
                indexes: vec![overlay_idx, base_idx],
                searchers: vec![overlay_searcher, base_searcher],
                fields,
                hash_vectors: overlay_hash_vec,
                base_hash_vectors: base_hash_vec,
                neural_vectors: overlay_neural_vec,
                base_neural_vectors: base_neural_vec,
                neural_profile: overlay_neural_profile,
                base_neural_profile,
                neural_model: overlay_neural_model,
                base_neural_model,
                tombstones,
                overlay_files,
                file_contents: RefCell::new(HashMap::new()),
                glob_path_filters: RefCell::new(HashMap::new()),
            })
        } else {
            let sqlite = open_sqlite_readonly(&workspace.sqlite_path())?;
            let (idx, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
            let reader = idx.reader()?;
            let searcher = reader.searcher();
            let hash_vec = open_optional_vector_store(
                wants_hash_vectors,
                &workspace.vector_path(),
                256,
                HASH_VECTOR_QUANTIZATION,
            )?;
            let neural_model = read_optional_neural_identity(&workspace.neural_model_path())?;
            let neural_dimensions = neural_model
                .as_ref()
                .map_or(384, |identity| identity.dimensions);
            let neural_vec = open_optional_vector_store(
                wants_neural_vectors,
                &workspace.vector_neural_path(),
                neural_dimensions,
                NEURAL_VECTOR_QUANTIZATION,
            )?;
            let neural_profile = read_optional_profile(&workspace.neural_profile_path())?;

            Ok(Self {
                sqlite,
                base_sqlite: None,
                indexes: vec![idx],
                searchers: vec![searcher],
                fields,
                hash_vectors: hash_vec,
                base_hash_vectors: None,
                neural_vectors: neural_vec,
                base_neural_vectors: None,
                neural_profile,
                base_neural_profile: None,
                neural_model,
                base_neural_model: None,
                tombstones: HashSet::new(),
                overlay_files: HashSet::new(),
                file_contents: RefCell::new(HashMap::new()),
                glob_path_filters: RefCell::new(HashMap::new()),
            })
        }
    }

    pub fn is_shadowed_base_file(&self, searcher_idx: usize, file_path: &std::path::Path) -> bool {
        let file_lossy = file_path.to_string_lossy();
        searcher_idx == 1
            && (self.tombstones.contains(file_lossy.as_ref())
                || self.overlay_files.contains(file_lossy.as_ref()))
    }

    pub fn fetch_chunk_by_vector_key(&self, vector_key: u64) -> Result<Option<IndexedChunk>> {
        if let Some(chunk) = fetch_chunk_by_vector_key(&self.sqlite, vector_key)? {
            return Ok(Some(chunk));
        }
        if let Some(base_sqlite) = &self.base_sqlite
            && let Some(chunk) = fetch_chunk_by_vector_key(base_sqlite, vector_key)?
            && !self.is_shadowed_base_file(1, &chunk.file_path)
        {
            return Ok(Some(chunk));
        }
        Ok(None)
    }

    /// Batch-fetch chunks by vector keys, checking overlay then base.
    pub fn fetch_chunks_by_vector_keys_batch(
        &self,
        keys: &[u64],
    ) -> Result<HashMap<u64, IndexedChunk>> {
        let mut result = fetch_chunks_by_vector_keys_batch(&self.sqlite, keys)?;
        if let Some(base_sqlite) = &self.base_sqlite {
            let missing: Vec<u64> = keys
                .iter()
                .filter(|k| !result.contains_key(k))
                .copied()
                .collect();
            if !missing.is_empty() {
                let base_chunks = fetch_chunks_by_vector_keys_batch(base_sqlite, &missing)?;
                for (key, chunk) in base_chunks {
                    if !self.is_shadowed_base_file(1, &chunk.file_path) {
                        result.insert(key, chunk);
                    }
                }
            }
        }
        Ok(result)
    }

    pub(crate) fn representative_hit_for_file(
        &self,
        file_path: &Path,
        task: &str,
        skip_gitignore: bool,
    ) -> Result<Option<SearchHit>> {
        let mut chunks = query_chunks_for_file(&self.sqlite, file_path, skip_gitignore)?;
        if chunks.is_empty()
            && let Some(base_sqlite) = &self.base_sqlite
            && !self.is_shadowed_base_file(1, file_path)
        {
            chunks = query_chunks_for_file(base_sqlite, file_path, skip_gitignore)?;
        }
        if chunks.is_empty() {
            return Ok(None);
        }
        let task_terms = task
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map(str::to_ascii_lowercase)
            .filter(|term| term.len() >= 3)
            .collect::<HashSet<_>>();
        chunks.sort_by(|left, right| {
            representative_chunk_score(right, &task_terms)
                .cmp(&representative_chunk_score(left, &task_terms))
                .then_with(|| left.start_line.cmp(&right.start_line))
        });
        let chunk = chunks.remove(0);
        let path_header = format!("// {}\n\n", chunk.file_path.display());
        let preview = chunk
            .text
            .strip_prefix(&path_header)
            .unwrap_or(&chunk.text)
            .to_string();
        Ok(Some(SearchHit {
            file_path: chunk.file_path,
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            preview,
            reason: "context graph relationship".to_string(),
            score: 0.0,
            sources: Vec::new(),
            neural_requested: false,
            neural_executed: false,
        }))
    }

    /// Batch-fetch stored text for candidates whose metadata is already loaded.
    fn fetch_chunk_texts_by_vector_keys_batch(&self, keys: &[u64]) -> Result<HashMap<u64, String>> {
        let mut result = fetch_chunk_texts_by_vector_keys_batch(&self.sqlite, keys)?;
        if let Some(base_sqlite) = &self.base_sqlite {
            let missing = keys
                .iter()
                .filter(|key| !result.contains_key(key))
                .copied()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                // Worktree overlays need the base file path to reject chunks
                // shadowed by an edited or deleted file.
                let base_chunks = fetch_chunks_by_vector_keys_batch(base_sqlite, &missing)?;
                for (key, chunk) in base_chunks {
                    if !self.is_shadowed_base_file(1, &chunk.file_path) {
                        result.insert(key, chunk.text);
                    }
                }
            }
        }
        Ok(result)
    }

    fn fetch_chunk_metadata_by_vector_keys_batch(
        &self,
        keys: &[u64],
    ) -> Result<HashMap<u64, IndexedChunk>> {
        let mut result = fetch_chunk_metadata_by_vector_keys_batch(&self.sqlite, keys)?;
        if let Some(base_sqlite) = &self.base_sqlite {
            let missing: Vec<u64> = keys
                .iter()
                .filter(|key| !result.contains_key(key))
                .copied()
                .collect();
            if !missing.is_empty() {
                let base_chunks = fetch_chunk_metadata_by_vector_keys_batch(base_sqlite, &missing)?;
                for (key, chunk) in base_chunks {
                    if !self.is_shadowed_base_file(1, &chunk.file_path) {
                        result.insert(key, chunk);
                    }
                }
            }
        }
        Ok(result)
    }

    fn read_file_content(&self, path: &Path) -> Option<CachedFileContent> {
        const MAX_CACHED_FILES: usize = 256;

        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => {
                self.file_contents.borrow_mut().remove(path);
                return None;
            }
        };
        let modified_nanos = metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let len = metadata.len();

        if let Some(cached) = self.file_contents.borrow().get(path)
            && cached.len == len
            && cached.modified_nanos == modified_nanos
        {
            return Some(cached.clone());
        }

        let content = Arc::<str>::from(fs::read_to_string(path).ok()?);
        let lines = line_spans(&content).into();
        let cached = CachedFileContent {
            len,
            modified_nanos,
            content,
            lines,
        };
        let mut cache = self.file_contents.borrow_mut();
        if cache.len() >= MAX_CACHED_FILES && !cache.contains_key(path) {
            cache.clear();
        }
        cache.insert(path.to_path_buf(), cached.clone());
        Some(cached)
    }
}

fn query_chunks_for_file(
    conn: &Connection,
    file_path: &Path,
    skip_gitignore: bool,
) -> Result<Vec<IndexedChunk>> {
    let path = crate::workspace::index_path_string(file_path);
    let mut statement = conn.prepare_cached(
        "SELECT file_path, start_line, end_line, language, kind, text, vector_key, is_ignored
         FROM chunks
         WHERE file_path = ?1 AND (?2 OR is_ignored = 0)
         ORDER BY start_line LIMIT 96",
    )?;
    let rows = statement.query_map(rusqlite::params![path, skip_gitignore], |row| {
        Ok((
            PathBuf::from(row.get::<_, String>(0)?),
            row.get::<_, i64>(1)? as usize,
            row.get::<_, i64>(2)? as usize,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Vec<u8>>(5)?,
            row.get::<_, i64>(6)? as u64,
            row.get::<_, bool>(7)?,
        ))
    })?;
    let mut chunks = Vec::new();
    for row in rows {
        let (file_path, start_line, end_line, language, kind, raw_text, vector_key, is_ignored) =
            row?;
        let text = crate::indexer::try_decompress_text(raw_text).with_context(|| {
            format!(
                "decompress indexed chunk {}:{}-{}",
                file_path.display(),
                start_line,
                end_line
            )
        })?;
        chunks.push(IndexedChunk {
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
        });
    }
    Ok(chunks)
}

fn representative_chunk_score(chunk: &IndexedChunk, task_terms: &HashSet<String>) -> usize {
    let text = chunk.text.to_ascii_lowercase();
    let path = chunk.file_path.to_string_lossy().to_ascii_lowercase();
    let term_score = task_terms
        .iter()
        .map(|term| {
            usize::from(text.contains(term))
                .saturating_mul(8)
                .saturating_add(usize::from(path.contains(term)).saturating_mul(5))
        })
        .fold(0usize, usize::saturating_add);
    let kind_score = usize::from(matches!(
        chunk.kind.as_str(),
        "Function" | "Class" | "Module" | "Struct" | "Trait" | "Interface" | "Enum"
    ))
    .saturating_mul(3);
    term_score
        .saturating_add(kind_score)
        .saturating_add(usize::from(chunk.start_line <= 20))
}

/// Fast index-backed literal text search.
///
/// Uses file-level trigram postings to find candidates, then verifies exact
/// case-insensitive substring matches against live files. Queries without an
/// indexable trigram fall back to a parallel source walk.
pub fn literal_search(
    workspace: &Workspace,
    query_text: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    reconcile_worktree_overlay(workspace, &model)?;
    let ctx = SearchContext::load(workspace, None, false)?;
    literal_search_with_context(&ctx, workspace, query_text, options)
}

pub fn literal_search_with_context(
    ctx: &SearchContext,
    workspace: &Workspace,
    query_text: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let t0 = std::time::Instant::now();
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    let query = query_text.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }
    if options.bounded_limit() == Some(0) {
        return Ok(vec![]);
    }

    let query_lower = query.to_ascii_lowercase();
    let max_hits = options.bounded_limit().unwrap_or(500);
    let context = options.bounded_context();
    let runs = substring_candidate_runs(query);
    let hits = if runs.is_empty() {
        literal_search_walk(workspace, &query_lower, options, max_hits)?
    } else {
        let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
        match substring_candidate_files(workspace, ctx, &runs, options, &path_matcher)? {
            Some(candidate_paths) => {
                literal_search_paths(&query_lower, context, max_hits, &candidate_paths, options)?
            }
            None => literal_search_walk(workspace, &query_lower, options, max_hits)?,
        }
    };

    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    tracing::trace!("literal_total={:?} hits={}", t0.elapsed(), hits.len());
    Ok(hits)
}

fn literal_search_walk(
    workspace: &Workspace,
    query_lower: &str,
    options: &SearchOptions,
    max_hits: usize,
) -> Result<Vec<SearchHit>> {
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let mut paths = Vec::new();
    for entry in crate::walker::source_walker(&workspace.root, options.skip_gitignore).build() {
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        let entry = entry?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.path();
        let rel_path = path.strip_prefix(&workspace.root).unwrap_or(path);
        if options
            .scope_filter
            .as_ref()
            .is_some_and(|scope| !scope.matches(rel_path))
            || !path_matcher.matches(rel_path)
            || options.type_filter.as_deref().is_some_and(|filter| {
                let expected = crate::chunking::resolve_type_alias(filter).unwrap_or(filter);
                crate::chunking::language_for_path(rel_path) != Some(expected)
            })
        {
            continue;
        }
        paths.push((rel_path.to_path_buf(), path.to_path_buf()));
    }

    literal_search_paths(
        query_lower,
        options.bounded_context(),
        max_hits,
        &paths,
        options,
    )
}

fn literal_search_paths(
    query_lower: &str,
    context: usize,
    max_hits: usize,
    paths: &[(PathBuf, PathBuf)],
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let mut hits = paths
        .par_iter()
        .flat_map_iter(|(rel_path, path)| {
            if options.is_cancelled() {
                return Vec::new();
            }
            let Ok(content) = fs::read_to_string(path) else {
                return Vec::new();
            };
            let lines = content.lines().collect::<Vec<_>>();
            lines
                .iter()
                .enumerate()
                .filter(|(_, line)| {
                    !options.is_cancelled() && line.to_ascii_lowercase().contains(query_lower)
                })
                .map(|(index, line)| {
                    let line_number = index.saturating_add(1);
                    let (start_line, end_line) = snippet_bounds(line_number, context, lines.len());
                    SearchHit {
                        file_path: rel_path.clone(),
                        start_line,
                        end_line,
                        preview: lines[start_line.saturating_sub(1)..end_line].join("\n"),
                        reason: format!("literal match: {}", truncate_for_reason(line.trim())),
                        score: 1.0,
                        sources: vec!["literal".to_string()],
                        neural_requested: false,
                        neural_executed: false,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    hits.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
    });
    hits.truncate(max_hits);
    Ok(hits)
}

fn substring_candidate_files(
    workspace: &Workspace,
    ctx: &SearchContext,
    runs: &[String],
    options: &SearchOptions,
    path_matcher: &PathGlobMatcher,
) -> Result<Option<Vec<(PathBuf, PathBuf)>>> {
    let Some(field) = ctx.fields.text_trigrams else {
        return Ok(None);
    };
    let Some(query) = substring_candidate_query(field, runs) else {
        return Ok(None);
    };
    let query = constrain_query_to_scope(query, &ctx.fields, options.scope_filter.as_ref())?;
    let mut paths = HashSet::new();
    for (index, searcher) in ctx.searchers.iter().enumerate() {
        if options.is_cancelled() {
            return Ok(Some(Vec::new()));
        }
        let docs = searcher.search(&query, &TopDocs::with_limit(10_000).order_by_score())?;
        if docs.len() == 10_000 {
            return Ok(None);
        }
        for (_score, address) in docs {
            if options.is_cancelled() {
                return Ok(Some(Vec::new()));
            }
            let doc: TantivyDocument = searcher.doc(address)?;
            let Some(path) = doc
                .get_first(ctx.fields.file_path)
                .and_then(|value| tantivy::schema::Value::as_str(&value))
                .map(PathBuf::from)
            else {
                continue;
            };
            let ignored = ctx
                .fields
                .is_ignored
                .and_then(|field| doc.get_first(field))
                .and_then(|value| tantivy::schema::Value::as_u64(&value))
                .is_some_and(|value| value > 0);
            if ctx.is_shadowed_base_file(index, &path)
                || (!options.skip_gitignore && ignored)
                || options
                    .scope_filter
                    .as_ref()
                    .is_some_and(|scope| !scope.matches(&path))
                || !path_matcher.matches(&path)
                || options.type_filter.as_deref().is_some_and(|filter| {
                    let expected = crate::chunking::resolve_type_alias(filter).unwrap_or(filter);
                    crate::chunking::language_for_path(&path) != Some(expected)
                })
            {
                continue;
            }
            paths.insert(path);
        }
    }
    let mut paths = paths
        .into_iter()
        .map(|path| {
            let full_path = workspace.root.join(&path);
            (path, full_path)
        })
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(Some(paths))
}

pub(crate) fn exact_literal_chunks(
    workspace: &Workspace,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<IndexedChunk>> {
    let ctx = SearchContext::load(workspace, None, false)?;
    exact_literal_chunks_with_context(&ctx, query, options, false)
}

pub(crate) fn exact_literal_chunks_unbounded(
    workspace: &Workspace,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<IndexedChunk>> {
    let ctx = SearchContext::load(workspace, None, false)?;
    exact_literal_chunks_with_context(&ctx, query, options, true)
}

fn exact_literal_chunks_with_context(
    ctx: &SearchContext,
    query: &str,
    options: &SearchOptions,
    unbounded: bool,
) -> Result<Vec<IndexedChunk>> {
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let glob_path_filter = build_glob_path_query_filter(ctx, &path_matcher, options)?;
    collect_literal_candidates(
        ctx,
        query,
        &path_matcher,
        &glob_path_filter,
        options,
        unbounded,
    )
}

/// Use the Tantivy inverted index to find candidate chunks containing the
/// literal query, then verify with regex on the decompressed text.
/// This is O(index_lookup + matched_candidates) instead of O(all_chunks).
fn collect_literal_candidates(
    ctx: &SearchContext,
    query: &str,
    path_matcher: &PathGlobMatcher,
    glob_path_filter: &GlobPathQueryFilter,
    options: &SearchOptions,
    unbounded: bool,
) -> Result<Vec<IndexedChunk>> {
    let candidate_limit = if unbounded {
        ctx.searchers
            .iter()
            .map(|searcher| searcher.num_docs() as usize)
            .sum::<usize>()
            .max(1)
    } else if let Some(limit) = options.bounded_limit() {
        if limit == usize::MAX {
            50_000
        } else {
            limit.saturating_mul(5).clamp(200, 25_000)
        }
    } else {
        250
    };
    let target_hits = if unbounded {
        candidate_limit
    } else {
        options.bounded_limit().unwrap_or(100).min(candidate_limit)
    };
    let candidate_queries = build_lexical_queries(query);
    let matcher = LiteralMatcher::from_queries(
        std::iter::once(query),
        literal_queries_need_specificity_ranking(&candidate_queries),
    )?;

    collect_literal_candidates_for_queries(
        ctx,
        &candidate_queries,
        &matcher,
        path_matcher,
        glob_path_filter,
        options,
        (candidate_limit, target_hits),
    )
}

fn collect_literal_candidates_for_queries(
    ctx: &SearchContext,
    candidate_queries: &[String],
    matcher: &LiteralMatcher,
    path_matcher: &PathGlobMatcher,
    glob_path_filter: &GlobPathQueryFilter,
    options: &SearchOptions,
    limits: (usize, usize),
) -> Result<Vec<IndexedChunk>> {
    let (candidate_limit, target_hits) = limits;
    if literal_queries_allow_incremental_verification(candidate_queries) {
        for query in candidate_queries {
            let candidates = collect_literal_candidate_chunks(
                ctx,
                std::slice::from_ref(query),
                path_matcher,
                glob_path_filter,
                options,
                candidate_limit,
                false,
            )?;
            let verified =
                verify_literal_candidates(candidates, candidate_queries, matcher, target_hits);
            if !verified.is_empty() {
                return Ok(verified);
            }
        }
        if !literal_queries_have_relaxed_variant(candidate_queries) {
            return Ok(Vec::new());
        }
    }

    let candidates = collect_literal_candidate_chunks(
        ctx,
        candidate_queries,
        path_matcher,
        glob_path_filter,
        options,
        candidate_limit,
        false,
    )?;
    let verified = verify_literal_candidates(candidates, candidate_queries, matcher, target_hits);
    if !verified.is_empty() || !literal_queries_have_relaxed_variant(candidate_queries) {
        return Ok(verified);
    }

    let candidates = collect_literal_candidate_chunks(
        ctx,
        candidate_queries,
        path_matcher,
        glob_path_filter,
        options,
        candidate_limit,
        true,
    )?;
    Ok(verify_literal_candidates(
        candidates,
        candidate_queries,
        matcher,
        target_hits,
    ))
}

fn collect_literal_candidate_chunks(
    ctx: &SearchContext,
    candidate_queries: &[String],
    path_matcher: &PathGlobMatcher,
    glob_path_filter: &GlobPathQueryFilter,
    options: &SearchOptions,
    candidate_limit: usize,
    relaxed: bool,
) -> Result<Vec<IndexedChunk>> {
    let mut found_ids = HashSet::<u64>::new();

    let mut candidates: Vec<IndexedChunk> = Vec::new();
    'outer: for lexical_query in candidate_queries {
        let Some(parsed_query) = literal_candidate_query(&ctx.fields, lexical_query, relaxed)
        else {
            continue;
        };
        let parsed_query =
            constrain_query_to_scope(parsed_query, &ctx.fields, options.scope_filter.as_ref())?;
        let parsed_query =
            constrain_query_to_glob_paths(parsed_query, &ctx.fields, glob_path_filter);

        for (i, searcher) in ctx.searchers.iter().enumerate() {
            let docs = searcher.search(
                &parsed_query,
                &TopDocs::with_limit(candidate_limit).order_by_score(),
            )?;

            for (_score, addr) in docs {
                let doc: TantivyDocument = searcher.doc(addr)?;
                if let Some(chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                    .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                    .filter(|c| type_matches(c, options.type_filter.as_deref()))
                    .filter(|c| scope_matches(c, options.scope_filter.as_ref()))
                    .filter(|c| path_matches(c, path_matcher))
                    .filter(|c| options.skip_gitignore || !c.is_ignored)
                    && found_ids.insert(chunk.vector_key)
                {
                    candidates.push(chunk);
                    if candidates.len() >= candidate_limit {
                        break 'outer;
                    }
                }
            }
        }
    }

    let empty_keys: Vec<u64> = candidates
        .iter()
        .filter(|c| c.text.is_empty())
        .map(|c| c.vector_key)
        .collect();
    if !empty_keys.is_empty() {
        let mut batch = ctx.fetch_chunks_by_vector_keys_batch(&empty_keys)?;
        for c in &mut candidates {
            if c.text.is_empty()
                && let Some(full) = batch.remove(&c.vector_key)
            {
                c.text = full.text;
            }
        }
    }

    Ok(candidates)
}

fn verify_literal_candidates(
    candidates: Vec<IndexedChunk>,
    candidate_queries: &[String],
    matcher: &LiteralMatcher,
    target_hits: usize,
) -> Vec<IndexedChunk> {
    if literal_queries_need_specificity_ranking(candidate_queries) {
        let mut verified = Vec::new();
        for chunk in candidates {
            if let Some((longest, count)) = literal_match_specificity(matcher, &chunk.text) {
                verified.push((longest, count, chunk));
            }
        }
        verified.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.vector_key.cmp(&right.2.vector_key))
        });
        verified.truncate(target_hits);
        return verified.into_iter().map(|(_, _, chunk)| chunk).collect();
    }

    let mut verified = Vec::new();
    for chunk in candidates {
        if matcher.is_match(&chunk.text) {
            verified.push(chunk);
            if verified.len() >= target_hits {
                break;
            }
        }
    }
    verified
}

fn literal_queries_have_relaxed_variant(queries: &[String]) -> bool {
    queries
        .iter()
        .any(|query| literal_candidate_terms(query).len() > 1)
}

fn literal_queries_allow_incremental_verification(queries: &[String]) -> bool {
    if literal_queries_need_specificity_ranking(queries) {
        return false;
    }
    queries.first().is_some_and(|query| {
        !query.chars().any(char::is_whitespace)
            && query.chars().any(|ch| {
                ch == '_'
                    || ch == '-'
                    || ch == ':'
                    || ch == '.'
                    || ch == '$'
                    || ch.is_ascii_uppercase()
                    || ch.is_ascii_digit()
            })
    })
}

fn literal_candidate_query(
    fields: &TantivyFields,
    query: &str,
    relaxed: bool,
) -> Option<Box<dyn Query>> {
    let terms = literal_candidate_terms(query);
    if terms.is_empty() {
        return None;
    }

    let mut indexed_fields = Vec::with_capacity(3);
    indexed_fields.push((fields.text, IndexRecordOption::WithFreqs));
    if let Some(field) = fields.file_path_text {
        indexed_fields.push((field, IndexRecordOption::Basic));
    }
    if let Some(field) = fields.signature {
        indexed_fields.push((field, IndexRecordOption::Basic));
    }

    let mut variants = Vec::with_capacity(terms.len().saturating_add(1));
    variants.push((0..terms.len()).collect::<Vec<_>>());
    if relaxed {
        if terms.len() == 2 {
            variants.push(vec![0]);
            variants.push(vec![1]);
        } else if terms.len() > 2 {
            for skipped in 0..terms.len() {
                variants.push(
                    (0..terms.len())
                        .filter(|index| *index != skipped)
                        .collect::<Vec<_>>(),
                );
            }
        }
    }

    let mut variant_queries = Vec::with_capacity(variants.len());
    for variant in variants {
        let mut clauses = Vec::with_capacity(variant.len());
        for term_index in variant {
            clauses.push((
                Occur::Must,
                literal_candidate_term_query(&indexed_fields, &terms[term_index]),
            ));
        }
        variant_queries.push((
            Occur::Should,
            Box::new(BooleanQuery::new(clauses)) as Box<dyn Query>,
        ));
    }

    Some(Box::new(BooleanQuery::new(variant_queries)))
}

pub(crate) fn substring_candidate_query(
    field: tantivy::schema::Field,
    runs: &[String],
) -> Option<Box<dyn Query>> {
    let mut clauses = Vec::new();
    for run in runs.iter().take(3) {
        let offsets = [0, (run.len() - 3) / 2, run.len() - 3];
        let mut previous = None;
        for offset in offsets {
            if previous == Some(offset) {
                continue;
            }
            previous = Some(offset);
            let trigram = run.get(offset..offset.saturating_add(3))?;
            clauses.push((
                Occur::Must,
                Box::new(TermQuery::new(
                    tantivy::Term::from_field_text(field, trigram),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>,
            ));
        }
    }
    (!clauses.is_empty()).then(|| Box::new(BooleanQuery::new(clauses)) as Box<dyn Query>)
}

fn substring_candidate_runs(query: &str) -> Vec<String> {
    query
        .as_bytes()
        .split(|byte| !byte.is_ascii_alphanumeric())
        .filter(|run| run.len() >= 3 && run.is_ascii())
        .map(|run| String::from_utf8_lossy(run).to_ascii_lowercase())
        .collect()
}

fn literal_candidate_term_query(
    indexed_fields: &[(tantivy::schema::Field, IndexRecordOption)],
    term_text: &str,
) -> Box<dyn Query> {
    let term_queries = indexed_fields
        .iter()
        .map(|(field, index_option)| {
            let term = tantivy::Term::from_field_text(*field, term_text);
            (
                Occur::Should,
                Box::new(TermQuery::new(term, *index_option)) as Box<dyn Query>,
            )
        })
        .collect::<Vec<_>>();
    Box::new(BooleanQuery::new(term_queries))
}

fn literal_candidate_terms(query: &str) -> Vec<String> {
    let mut analyzer = build_code_analyzer();
    let mut stream = analyzer.token_stream(query);
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    while stream.advance() {
        let text = stream.token().text.clone();
        if seen.insert(text.clone()) {
            terms.push(text);
        }
    }
    terms
}

fn simple_lexical_query(
    fields: &TantivyFields,
    query: &str,
    conjunction_by_default: bool,
) -> Option<Box<dyn Query>> {
    if !query
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() || ch == '_')
    {
        return None;
    }
    if query
        .split_ascii_whitespace()
        .any(|term| matches!(term, "AND" | "OR" | "NOT"))
    {
        return None;
    }

    let terms = literal_candidate_terms(query);
    if terms.is_empty() {
        return None;
    }

    if !conjunction_by_default && terms.len() == 3 {
        let clauses = terms
            .iter()
            .flat_map(|term| simple_lexical_term_clauses(fields, term))
            .collect::<Vec<_>>();
        return Some(Box::new(BooleanQuery::new(clauses)));
    }

    let occur = if conjunction_by_default {
        Occur::Must
    } else {
        Occur::Should
    };
    let mut clauses = terms
        .iter()
        .map(|term| (occur, simple_lexical_term_query(fields, term)))
        .collect::<Vec<_>>();
    if clauses.len() == 1 {
        Some(clauses.pop().unwrap().1)
    } else {
        Some(Box::new(BooleanQuery::new(clauses)))
    }
}

struct LexicalQueryExecutor<'a> {
    fields: &'a TantivyFields,
    parser: &'a QueryParser,
    conjunctive_numeric_query: bool,
    scope_filter: Option<&'a WorkspaceScope>,
    glob_path_filter: &'a GlobPathQueryFilter,
    can_pushdown_languages: bool,
    allowed_languages: &'a [String],
    searchers: &'a [tantivy::Searcher],
}

impl LexicalQueryExecutor<'_> {
    fn collect_docs(
        &self,
        lexical_query: &str,
        query_candidate_limit: usize,
    ) -> Result<Vec<(usize, f32, TantivyDocument)>> {
        let mut parsed_query = if let Some(query) =
            simple_lexical_query(self.fields, lexical_query, self.conjunctive_numeric_query)
        {
            query
        } else {
            match self.parser.parse_query(lexical_query) {
                Ok(query) => query,
                Err(err) => {
                    tracing::debug!(
                        query_variant = lexical_query,
                        error = %err,
                        "skipping lexical expansion rejected by Tantivy parser"
                    );
                    return Ok(Vec::new());
                }
            }
        };
        parsed_query = constrain_query_to_scope(parsed_query, self.fields, self.scope_filter)?;
        parsed_query =
            constrain_query_to_glob_paths(parsed_query, self.fields, self.glob_path_filter);

        if self.can_pushdown_languages && !self.allowed_languages.is_empty() {
            let lang_queries = self
                .allowed_languages
                .iter()
                .map(|language| {
                    let term = tantivy::Term::from_field_text(self.fields.language, language);
                    (
                        Occur::Should,
                        Box::new(TermQuery::new(term, IndexRecordOption::Basic)) as Box<dyn Query>,
                    )
                })
                .collect();
            parsed_query = Box::new(BooleanQuery::new(vec![
                (Occur::Must, parsed_query),
                (Occur::Must, Box::new(BooleanQuery::new(lang_queries))),
            ]));
        }

        let mut docs = Vec::new();
        for (searcher_index, searcher) in self.searchers.iter().enumerate() {
            for (score, address) in searcher.search(
                &parsed_query,
                &TopDocs::with_limit(query_candidate_limit).order_by_score(),
            )? {
                docs.push((
                    searcher_index,
                    score,
                    searcher.doc::<TantivyDocument>(address)?,
                ));
            }
        }
        Ok(docs)
    }
}

fn simple_lexical_term_query(fields: &TantivyFields, term_text: &str) -> Box<dyn Query> {
    Box::new(BooleanQuery::new(simple_lexical_term_clauses(
        fields, term_text,
    )))
}

fn simple_lexical_term_clauses(
    fields: &TantivyFields,
    term_text: &str,
) -> Vec<(Occur, Box<dyn Query>)> {
    let mut field_queries = Vec::with_capacity(3);
    field_queries.push((
        Occur::Should,
        Box::new(TermQuery::new(
            tantivy::Term::from_field_text(fields.text, term_text),
            IndexRecordOption::WithFreqs,
        )) as Box<dyn Query>,
    ));
    if let Some(field) = fields.file_path_text {
        field_queries.push((
            Occur::Should,
            Box::new(BoostQuery::new(
                Box::new(TermQuery::new(
                    tantivy::Term::from_field_text(field, term_text),
                    IndexRecordOption::Basic,
                )),
                5.0,
            )),
        ));
    }
    if let Some(field) = fields.signature {
        field_queries.push((
            Occur::Should,
            Box::new(BoostQuery::new(
                Box::new(TermQuery::new(
                    tantivy::Term::from_field_text(field, term_text),
                    IndexRecordOption::Basic,
                )),
                5.0,
            )),
        ));
    }
    field_queries
}

fn literal_queries_need_specificity_ranking(queries: &[String]) -> bool {
    queries
        .iter()
        .any(|query| query.len() == 3 && query.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        && queries.iter().any(|query| query.len() >= 5)
}

fn literal_match_specificity(matcher: &LiteralMatcher, text: &str) -> Option<(usize, usize)> {
    matcher.specificity(text)
}

enum LiteralMatcher {
    Ascii(Vec<String>),
    Regex(regex::Regex),
}

impl LiteralMatcher {
    fn from_queries<'a>(
        queries: impl IntoIterator<Item = &'a str>,
        force_regex: bool,
    ) -> Result<Self> {
        let queries = queries.into_iter().collect::<Vec<_>>();
        if !force_regex && queries.len() == 1 && queries[0].is_ascii() {
            return Ok(Self::Ascii(
                queries
                    .into_iter()
                    .map(str::to_ascii_lowercase)
                    .collect::<Vec<_>>(),
            ));
        }

        let pattern = queries
            .into_iter()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join("|");
        Ok(Self::Regex(
            regex::RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .build()?,
        ))
    }

    fn is_match(&self, text: &str) -> bool {
        match self {
            Self::Ascii(needles) => needles
                .iter()
                .any(|needle| contains_ascii_case_insensitive(text, needle)),
            Self::Regex(regex) => regex.is_match(text),
        }
    }

    fn match_count(&self, text: &str) -> usize {
        match self {
            Self::Ascii(needles) => needles
                .iter()
                .map(|needle| count_ascii_case_insensitive(text, needle))
                .sum(),
            Self::Regex(regex) => regex.find_iter(text).count(),
        }
    }

    fn specificity(&self, text: &str) -> Option<(usize, usize)> {
        match self {
            Self::Ascii(needles) => {
                let mut count = 0;
                let mut longest = 0;
                for needle in needles {
                    let matches = count_ascii_case_insensitive(text, needle);
                    if matches > 0 {
                        count += matches;
                        longest = longest.max(needle.len());
                    }
                }
                (count > 0).then_some((longest, count))
            }
            Self::Regex(regex) => {
                let mut count = 0;
                let mut longest = 0;
                for matched in regex.find_iter(text) {
                    count += 1;
                    longest = longest.max(matched.as_str().len());
                }
                (count > 0).then_some((longest, count))
            }
        }
    }
}

fn contains_ascii_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
    !needle_lower.is_empty()
        && haystack
            .as_bytes()
            .windows(needle_lower.len())
            .any(|window| {
                window
                    .iter()
                    .zip(needle_lower.bytes())
                    .all(|(left, right)| left.to_ascii_lowercase() == right)
            })
}

fn count_ascii_case_insensitive(haystack: &str, needle_lower: &str) -> usize {
    if needle_lower.is_empty() {
        return 0;
    }
    let needle_len = needle_lower.len();
    let mut count = 0;
    let mut cursor = 0;
    let bytes = haystack.as_bytes();
    while cursor + needle_len <= bytes.len() {
        let window = &bytes[cursor..cursor + needle_len];
        if window
            .iter()
            .zip(needle_lower.bytes())
            .all(|(left, right)| left.to_ascii_lowercase() == right)
        {
            count += 1;
            cursor += needle_len;
        } else {
            cursor += 1;
        }
    }
    count
}

pub fn hybrid_search(
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let fallback_model;
    let reconciliation_model = if let Some(model) = embedding_model {
        model
    } else {
        fallback_model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        &fallback_model
    };
    reconcile_worktree_overlay(workspace, reconciliation_model)?;
    let routing = QueryRouting::classify(query_text);
    let can_skip_neural_vectors = embedding_model.is_some()
        && !options.force_neural
        && !has_semantic_filters(options)
        && !routing.use_neural;
    let wants_semantic_vectors = embedding_model.is_some() && !can_skip_neural_vectors;
    let ctx = SearchContext::load(
        workspace,
        embedding_model
            .filter(|_| wants_semantic_vectors)
            .map(|model| model.dimensions()),
        wants_semantic_vectors
            && embedding_model.is_some_and(|model| model.model_identity().is_some()),
    )?;
    hybrid_search_with_context(&ctx, workspace, query_text, embedding_model, options)
}

pub(crate) fn query_uses_neural(query_text: &str, force_neural: bool) -> bool {
    force_neural || QueryRouting::classify(query_text.trim()).use_neural
}

pub fn hybrid_search_with_context(
    ctx: &SearchContext,
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    hybrid_search_with_context_and_neural_job(
        ctx,
        workspace,
        query_text,
        embedding_model,
        options,
        None,
    )
}

fn natural_language_path_recall_query(query_text: &str) -> Option<String> {
    natural_language_path_recall_terms(query_text).map(|terms| terms.join(" "))
}

fn natural_language_path_recall_terms(query_text: &str) -> Option<Vec<String>> {
    if raw_query_terms(query_text).len() <= 3 {
        return None;
    }

    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for raw in raw_query_terms(query_text) {
        if raw.len() < 3
            || is_query_stopword(&raw)
            || crate::chunking::resolve_type_alias(&raw).is_some()
        {
            continue;
        }

        let singular = singularize_token(&raw);
        for candidate in [singular, raw] {
            if candidate.len() >= 3 && seen.insert(candidate.clone()) {
                terms.push(candidate);
                if terms.len() == 12 {
                    break;
                }
            }
        }
        if terms.len() == 12 {
            break;
        }
    }
    (terms.len() >= 2).then_some(terms)
}

fn semantic_hash_weight(
    neural_available: bool,
    neural_vector_count: usize,
    hash_vector_count: usize,
) -> f32 {
    if !neural_available || hash_vector_count == 0 {
        return 1.0;
    }
    let coverage = (neural_vector_count as f32 / hash_vector_count as f32).clamp(0.0, 1.0);
    (1.0 - coverage).clamp(0.3, 1.0)
}

fn embed_hash_query(query_text: &str) -> Vec<f32> {
    static SEARCH_HASH_MODEL: std::sync::OnceLock<crate::embedding::HashEmbeddingModel> =
        std::sync::OnceLock::new();
    let hash_model =
        SEARCH_HASH_MODEL.get_or_init(|| crate::embedding::HashEmbeddingModel::new(256));
    hash_model.embed(&build_semantic_query_text(query_text))
}

fn build_lexical_queries(query_text: &str) -> Vec<String> {
    let query = query_text.trim();
    if query.is_empty() {
        return vec![];
    }

    let mut queries = vec![query.to_string()];
    let normalized_tokens = tokenize_query(query);
    if !normalized_tokens.is_empty() {
        let normalized = normalized_tokens.join(" ");
        if !normalized.eq_ignore_ascii_case(query) {
            queries.push(normalized);
        }

        let compact = normalized_tokens.join("");
        if compact.len() >= 4 && !compact.eq_ignore_ascii_case(query) {
            queries.push(compact);
        }

        // snake_case variant: "error handling" → "error_handling"
        if normalized_tokens.len() >= 2 {
            let snake = normalized_tokens.join("_");
            queries.push(snake);
        }

        // camelCase variant: "error handling" → "errorHandling"
        if normalized_tokens.len() >= 2 {
            let mut camel = normalized_tokens[0].clone();
            for token in &normalized_tokens[1..] {
                let mut chars = token.chars();
                if let Some(first) = chars.next() {
                    camel.push(first.to_ascii_uppercase());
                    camel.extend(chars);
                }
            }
            queries.push(camel);
        }
    }

    let mut token_aliases = Vec::new();
    for token in &normalized_tokens {
        token_aliases.extend(
            crate::query_aliases::token_aliases(token)
                .iter()
                .map(|alias| (*alias).to_string()),
        );
    }
    token_aliases.sort();
    token_aliases.dedup();
    if !token_aliases.is_empty() {
        queries.push(token_aliases.join(" "));
    }

    for alias in crate::query_aliases::phrase_aliases(&normalized_tokens) {
        queries.push(alias.to_string());
    }

    if normalized_tokens.len() > 1 {
        let mut derivational_roots = Vec::new();
        for token in &normalized_tokens {
            let (root, add_silent_e) = if let Some(root) = token.strip_suffix("ing") {
                (root, true)
            } else if let Some(root) = token.strip_suffix("ed") {
                (root, true)
            } else if let Some(root) = token.strip_suffix("or") {
                (root, false)
            } else {
                continue;
            };
            if root.len() >= 4 {
                derivational_roots.push(root.to_string());
                if add_silent_e {
                    derivational_roots.push(format!("{root}e"));
                }
            }
        }
        derivational_roots.sort();
        derivational_roots.dedup();
        if !derivational_roots.is_empty() {
            queries.push(derivational_roots.join(" "));
        }
    }

    let mut seen = HashSet::new();
    queries.retain(|query| seen.insert(query.clone()));
    queries
}

fn natural_language_symbol_queries(query_text: &str) -> Vec<String> {
    if raw_query_terms(query_text).len() <= 1 || !query_text.chars().any(char::is_whitespace) {
        return Vec::new();
    }

    const MAX_SYMBOL_QUERIES: usize = 16;

    let mut queries = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |candidate: String| {
        let normalized = candidate.to_ascii_lowercase();
        if candidate.len() >= 3
            && !is_query_stopword(&normalized)
            && seen.insert(normalized)
            && queries.len() < MAX_SYMBOL_QUERIES
        {
            queries.push(candidate);
        }
    };

    // Preserve source-style identifiers only near the subject of the query,
    // not a trailing language name or incidental type deep in the sentence.
    let meaningful = query_text
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
        .filter(|token| !token.is_empty())
        .filter_map(|raw| {
            let lower = raw.to_ascii_lowercase();
            (!is_query_stopword(&lower) && crate::chunking::resolve_type_alias(&lower).is_none())
                .then_some((raw, lower))
        })
        .collect::<Vec<_>>();
    let mut explicit = meaningful
        .iter()
        .take(2)
        .filter_map(|(raw, lower)| {
            let source_shaped = raw.contains(['_', '$'])
                || raw.chars().any(|ch| ch.is_ascii_uppercase())
                    && raw.chars().any(|ch| ch.is_ascii_lowercase());
            source_shaped.then_some(((*raw).to_string(), lower.clone()))
        })
        .collect::<Vec<_>>();
    let raw_terms = raw_query_terms(query_text);
    if raw_terms.iter().any(|term| term == "definitions") && explicit.len() >= 2 {
        explicit.clear();
    }
    for (name, _) in explicit {
        push(name);
    }

    // Bridge common prose operations to implementation-role definitions.
    for (index, (_, token)) in meaningful.iter().enumerate() {
        let role = match token.as_str() {
            "parse" | "parses" if index < 2 => Some("Parser"),
            "route" | "routed" | "routes" | "routing" | "router" => Some("Router"),
            _ => None,
        };
        if let Some(role) = role {
            push(role.to_string());
        }
    }

    // "X Y internals" often names a source symbol in separated prose, such
    // as "reflection equals internals" referring to `reflectionEquals`.
    if meaningful.iter().any(|(_, token)| token == "internals") {
        let subject = meaningful
            .iter()
            .take_while(|(_, token)| token != "internals")
            .take(4)
            .collect::<Vec<_>>();
        for pair in subject.windows(2) {
            let (_, left) = pair[0];
            let (_, right) = pair[1];
            if left.len() < 3 || right.len() < 3 {
                continue;
            }
            let mut compound = left.clone();
            let mut right_chars = right.chars();
            if let Some(first) = right_chars.next() {
                compound.push(first.to_ascii_uppercase());
                compound.extend(right_chars);
                push(compound);
            }
        }
    }

    queries
}

fn qualified_symbol_leaf_names(query_text: &str) -> Vec<String> {
    const MAX_QUALIFIED_NAMES: usize = 4;

    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for candidate in query_text
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$' && ch != '.')
        .map(|candidate| candidate.trim_matches('.'))
        .filter(|candidate| candidate.contains('.'))
    {
        let mut parts = candidate.split('.');
        let (Some(owner), Some(leaf), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let valid_owner = owner
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
            && owner
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
        let valid_leaf = leaf
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch == '$')
            && leaf
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
        let object_shaped = (2..=3).contains(&owner.len())
            || leaf.chars().skip(1).any(|ch| ch.is_ascii_uppercase());
        let normalized = leaf.to_ascii_lowercase();
        if valid_owner
            && valid_leaf
            && object_shaped
            && leaf.len() >= 2
            && crate::chunking::resolve_type_alias(&normalized).is_none()
            && seen.insert(normalized)
        {
            names.push(leaf.to_string());
            if names.len() == MAX_QUALIFIED_NAMES {
                break;
            }
        }
    }
    names
}

fn exact_symbol_query_names(query_text: &str) -> Vec<String> {
    let query = query_text.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let mut names = Vec::new();
    if !query.chars().any(char::is_whitespace) {
        names.push(query.to_string());
    }
    names.extend(qualified_symbol_leaf_names(query));
    if !query.chars().any(char::is_whitespace)
        && let Some(leaf) = query
            .rsplit([':', '\\', '.', '/', '#'])
            .find(|part| !part.is_empty())
        && leaf != query
    {
        names.push(leaf.to_string());
    }
    if raw_query_terms(query).len() > 1 {
        names.extend(
            query
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .filter(|term| {
                    term.len() >= 3
                        && term.chars().any(|ch| ch.is_ascii_lowercase())
                        && term.chars().skip(1).any(|ch| ch.is_ascii_uppercase())
                })
                .map(str::to_string),
        );
    }
    names.sort();
    names.dedup();
    names
}

fn lexical_query_candidate_limits(total: usize, query_count: usize) -> Vec<usize> {
    match query_count {
        0 => Vec::new(),
        1 => vec![total],
        _ => {
            let primary = total.saturating_mul(3) / 4;
            let expansion_total = total.saturating_sub(primary);
            let expansion_count = query_count - 1;
            let base = expansion_total / expansion_count;
            let remainder = expansion_total % expansion_count;

            std::iter::once(primary)
                .chain(
                    (0..expansion_count)
                        .map(|index| base.saturating_add(usize::from(index < remainder))),
                )
                .collect()
        }
    }
}

fn lexical_search_queries_for_routing(
    lexical_queries: &[String],
    routing: QueryRouting,
    conjunctive_numeric_query: bool,
) -> Vec<String> {
    if conjunctive_numeric_query {
        return lexical_queries.iter().take(1).cloned().collect();
    }
    if !routing.use_neural
        || matches!(
            routing.intent,
            QueryIntent::ExactIdentifier | QueryIntent::Path | QueryIntent::LiteralOrError
        )
        || lexical_queries.len() <= 2
    {
        return lexical_queries.to_vec();
    }

    let mut queries = Vec::with_capacity(2);
    queries.push(lexical_queries[0].clone());

    let mut expansion_terms = Vec::new();
    let mut seen = HashSet::new();
    for query in &lexical_queries[1..] {
        for term in query.split_whitespace() {
            if seen.insert(term.to_string()) {
                expansion_terms.push(term.to_string());
            }
        }
    }
    if !expansion_terms.is_empty() {
        queries.push(expansion_terms.join(" "));
    }
    queries
}

fn should_run_literal_pass(query_text: &str) -> bool {
    let query = query_text.trim();
    if query.is_empty() {
        return false;
    }
    if QueryRouting::classify(query).intent == QueryIntent::LiteralOrError {
        return true;
    }

    let tokens = tokenize_query(query);
    tokens.len() <= 2
        || (tokens.len() <= 3
            && query
                .chars()
                .any(|c| c == '_' || c == '-' || c == '/' || c == ':' || c.is_ascii_uppercase()))
}

fn should_use_conjunctive_numeric_query(query_text: &str) -> bool {
    let terms = raw_query_terms(query_text);
    (3..=10).contains(&terms.len())
        && query_text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
        && terms
            .last()
            .is_some_and(|term| term.len() >= 3 && term.chars().all(|ch| ch.is_ascii_digit()))
        && terms
            .iter()
            .filter(|term| term.chars().all(|ch| ch.is_ascii_digit()))
            .count()
            == 1
}

fn build_literal_queries(query_text: &str, lexical_queries: &[String]) -> Vec<String> {
    if should_run_literal_pass(query_text) {
        return lexical_queries.to_vec();
    }

    let primary = tokenize_query(query_text);
    let mut aliases = crate::query_aliases::literal_phrase_aliases(&primary)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    aliases.extend(
        aliases
            .iter()
            .filter(|alias| alias.contains('_'))
            .flat_map(|alias| {
                [
                    alias.replace('_', "-"),
                    alias.replace('_', " "),
                    snake_to_camel_case(alias),
                ]
            })
            .collect::<Vec<_>>(),
    );
    aliases.sort();
    aliases.dedup();
    aliases
}

fn snake_to_camel_case(value: &str) -> String {
    let mut words = value.split('_');
    let mut camel = words.next().unwrap_or_default().to_string();
    for word in words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            camel.push(first.to_ascii_uppercase());
            camel.extend(chars);
        }
    }
    camel
}

fn tokenize_query(query: &str) -> Vec<String> {
    const CACHE_CAPACITY: usize = 8;
    thread_local! {
        static CACHE: RefCell<VecDeque<(String, Vec<String>)>> =
            RefCell::new(VecDeque::with_capacity(CACHE_CAPACITY));
    }

    if let Some(tokens) = CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(cached_query, _)| cached_query == query)
            .map(|(_, tokens)| tokens.clone())
    }) {
        return tokens;
    }

    let mut tokens = Vec::new();
    let mut seen = HashSet::new();
    for raw in query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        for segment in split_identifier_segments(raw) {
            let lower = segment.to_ascii_lowercase();
            if is_query_stopword(&lower) {
                continue;
            }
            let normalized = singularize_token(&lower);
            if normalized.len() >= 2
                && !is_query_stopword(&normalized)
                && seen.insert(normalized.clone())
            {
                tokens.push(normalized);
            }
        }
    }

    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() == CACHE_CAPACITY {
            cache.pop_back();
        }
        cache.push_front((query.to_string(), tokens.clone()));
    });

    tokens
}

fn expanded_query_tokens(query: &str) -> Vec<String> {
    let primary = tokenize_query(query);
    let mut expanded = primary.clone();
    let mut seen = primary.iter().cloned().collect::<HashSet<_>>();

    for token in &primary {
        for alias in crate::query_aliases::token_aliases(token) {
            let alias = alias.to_string();
            if alias.len() >= 2 && seen.insert(alias.clone()) {
                expanded.push(alias);
            }
        }
    }

    for alias in crate::query_aliases::phrase_aliases(&primary) {
        let alias = alias.to_string();
        if alias.len() >= 2 && seen.insert(alias.clone()) {
            expanded.push(alias);
        }
    }

    expanded
}

fn build_semantic_query_text(query_text: &str) -> String {
    let query = query_text.trim();
    if query.is_empty() {
        return String::new();
    }

    let mut fragments = vec![query.to_string()];
    let mut seen = HashSet::new();
    let mut expanded = Vec::new();

    for token in raw_query_terms(query)
        .into_iter()
        .chain(tokenize_query(query))
    {
        if seen.insert(token.clone()) {
            expanded.push(token);
        }
    }

    if !expanded.is_empty() {
        fragments.push(expanded.join(" "));
    }

    fragments.join(" ")
}

fn is_query_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "been"
            | "being"
            | "by"
            | "can"
            | "could"
            | "did"
            | "do"
            | "does"
            | "done"
            | "file"
            | "files"
            | "find"
            | "for"
            | "from"
            | "how"
            | "i"
            | "in"
            | "into"
            | "is"
            | "it"
            | "locate"
            | "located"
            | "me"
            | "of"
            | "on"
            | "please"
            | "show"
            | "the"
            | "their"
            | "there"
            | "these"
            | "this"
            | "those"
            | "to"
            | "was"
            | "were"
            | "what"
            | "where"
            | "which"
            | "who"
            | "why"
            | "with"
            | "within"
            | "code"
    )
}

fn has_location_intent(query_text: &str) -> bool {
    raw_query_terms(query_text).into_iter().any(|term| {
        matches!(
            term.as_str(),
            "where"
                | "find"
                | "locate"
                | "located"
                | "implemented"
                | "implementation"
                | "defined"
                | "definition"
                | "done"
        )
    })
}

fn query_targets_secondary_sources(query_text: &str) -> bool {
    raw_query_terms(query_text).into_iter().any(|term| {
        matches!(
            term.as_str(),
            "doc"
                | "docs"
                | "documentation"
                | "example"
                | "examples"
                | "fixture"
                | "fixtures"
                | "guide"
                | "mock"
                | "mocks"
                | "readme"
                | "sample"
                | "samples"
                | "snapshot"
                | "snapshots"
                | "spec"
                | "specs"
                | "test"
                | "tests"
                | "tutorial"
        )
    })
}

fn truncate_for_reason(line: &str) -> String {
    const MAX_REASON_CHARS: usize = 120;
    if line.chars().count() <= MAX_REASON_CHARS {
        return line.to_string();
    }

    let truncated = line.chars().take(MAX_REASON_CHARS).collect::<String>();
    format!("{truncated}...")
}

fn type_matches(chunk: &IndexedChunk, type_filter: Option<&str>) -> bool {
    match type_filter {
        Some(filter) => {
            if chunk.language.eq_ignore_ascii_case(filter) {
                return true;
            }
            // Resolve aliases: "rs" → "rust", "py" → "python", etc.
            if let Some(canonical) = crate::chunking::resolve_type_alias(filter) {
                chunk.language.eq_ignore_ascii_case(canonical)
            } else {
                false
            }
        }
        None => true,
    }
}

fn scope_path_matches(rel_path: &Path, scope_filter: Option<&WorkspaceScope>) -> bool {
    scope_filter.is_none_or(|scope| scope.matches(rel_path))
}

fn scope_matches(chunk: &IndexedChunk, scope_filter: Option<&WorkspaceScope>) -> bool {
    scope_path_matches(&chunk.file_path, scope_filter)
}

fn path_matches(chunk: &IndexedChunk, path_matcher: &PathGlobMatcher) -> bool {
    path_matcher.matches(&chunk.file_path)
}

fn constrain_query_to_scope(
    query: Box<dyn Query>,
    fields: &TantivyFields,
    scope_filter: Option<&WorkspaceScope>,
) -> Result<Box<dyn Query>> {
    let Some(scope) = scope_filter else {
        return Ok(query);
    };

    let scope_path = index_path_string(&scope.rel_path);
    let path_query: Box<dyn Query> = if scope.is_file {
        Box::new(TermQuery::new(
            tantivy::Term::from_field_text(fields.file_path, &scope_path),
            IndexRecordOption::Basic,
        ))
    } else {
        let prefix = format!("{}/", regex::escape(&scope_path));
        Box::new(RegexQuery::from_pattern(
            &format!("{prefix}.*"),
            fields.file_path,
        )?)
    };

    Ok(Box::new(BooleanQuery::new(vec![
        (Occur::Must, query),
        (Occur::Must, path_query),
    ])))
}

const MAX_GLOB_PATH_TERMS: usize = 10_000;

#[derive(Debug, Clone, Default)]
struct GlobPathQueryFilter {
    included_paths: Option<Vec<String>>,
    excluded_paths: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GlobPathFilterCacheKey {
    include_globs: Vec<String>,
    exclude_globs: Vec<String>,
    scope_path: Option<String>,
    scope_is_file: bool,
}

/// Build an exact-path Tantivy filter for focused globs before TopDocs ranking.
///
/// Globset supports patterns richer than Tantivy regexes. Streaming distinct
/// indexed paths keeps matching semantics identical to the final Rust filter.
/// Broad globs fall back to post-filtering once the bounded term set overflows.
fn build_glob_path_query_filter(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Result<GlobPathQueryFilter> {
    let mut filter = GlobPathQueryFilter {
        included_paths: (!options.include_globs.is_empty()).then(Vec::new),
        excluded_paths: (!options.exclude_globs.is_empty()).then(Vec::new),
    };
    if filter.included_paths.is_none() && filter.excluded_paths.is_none() {
        return Ok(filter);
    }

    let cache_key = GlobPathFilterCacheKey {
        include_globs: options.include_globs.clone(),
        exclude_globs: options.exclude_globs.clone(),
        scope_path: options
            .scope_filter
            .as_ref()
            .map(|scope| index_path_string(&scope.rel_path)),
        scope_is_file: options
            .scope_filter
            .as_ref()
            .is_some_and(|scope| scope.is_file),
    };
    if let Some(cached) = ctx.glob_path_filters.borrow().get(&cache_key).cloned() {
        return Ok(cached);
    }

    let mut seen_paths = HashSet::new();
    let mut collect_path = |path: String, searcher_idx: usize| {
        if searcher_idx == 1 && ctx.is_shadowed_base_file(searcher_idx, Path::new(&path)) {
            return true;
        }
        if !seen_paths.insert(path.clone())
            || !scope_path_matches(Path::new(&path), options.scope_filter.as_ref())
        {
            return true;
        }

        if let Some(paths) = &mut filter.included_paths
            && path_matcher.matches(Path::new(&path))
        {
            if paths.len() == MAX_GLOB_PATH_TERMS {
                filter.included_paths = None;
            } else {
                paths.push(path.clone());
            }
        }

        if let Some(paths) = &mut filter.excluded_paths
            && path_matcher.is_excluded(Path::new(&path))
        {
            if paths.len() == MAX_GLOB_PATH_TERMS {
                filter.excluded_paths = None;
            } else {
                paths.push(path);
            }
        }

        filter.included_paths.is_some() || filter.excluded_paths.is_some()
    };

    let should_continue = visit_distinct_file_paths(&ctx.sqlite, |path| collect_path(path, 0))?;
    if should_continue && let Some(base_sqlite) = &ctx.base_sqlite {
        visit_distinct_file_paths(base_sqlite, |path| collect_path(path, 1))?;
    }

    let cached_terms = filter.included_paths.as_ref().map_or(0, Vec::len)
        + filter.excluded_paths.as_ref().map_or(0, Vec::len);
    if cached_terms <= 2_048 {
        let mut cache = ctx.glob_path_filters.borrow_mut();
        if cache.len() >= 16 {
            cache.clear();
        }
        cache.insert(cache_key, filter.clone());
    }

    Ok(filter)
}

fn visit_distinct_file_paths(
    conn: &Connection,
    mut visit: impl FnMut(String) -> bool,
) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT DISTINCT file_path FROM chunks")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if !visit(row.get(0)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn constrain_query_to_glob_paths(
    query: Box<dyn Query>,
    fields: &TantivyFields,
    filter: &GlobPathQueryFilter,
) -> Box<dyn Query> {
    let mut clauses = vec![(Occur::Must, query)];
    if let Some(paths) = &filter.included_paths {
        clauses.push((
            Occur::Must,
            Box::new(TermSetQuery::new(paths.iter().map(|path| {
                tantivy::Term::from_field_text(fields.file_path, path)
            }))),
        ));
    }
    if let Some(paths) = &filter.excluded_paths
        && !paths.is_empty()
    {
        clauses.push((
            Occur::MustNot,
            Box::new(TermSetQuery::new(paths.iter().map(|path| {
                tantivy::Term::from_field_text(fields.file_path, path)
            }))),
        ));
    }

    if clauses.len() == 1 {
        clauses.pop().unwrap().1
    } else {
        Box::new(BooleanQuery::new(clauses))
    }
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn is_definition_kind(kind: &str) -> bool {
    matches!(
        kind,
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
            | "Module"
            | "module"
    )
}

/// Pre-collect chunks from SQLite that match glob/scope/type filters.
/// Used to avoid full-corpus vector scan when targeted filters are set.
#[derive(Clone, Copy)]
struct FilteredChunkQuery<'a> {
    path_matcher: &'a PathGlobMatcher,
    scope_filter: Option<&'a WorkspaceScope>,
    type_filter: Option<&'a str>,
    include_globs: &'a [String],
    skip_gitignore: bool,
    max_results: usize,
}

fn collect_filtered_chunks(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    scope_filter: Option<&WorkspaceScope>,
    type_filter: Option<&str>,
    include_globs: &[String],
    skip_gitignore: bool,
    max_results: usize,
) -> Result<Vec<RawIndexedChunk>> {
    let query = FilteredChunkQuery {
        path_matcher,
        scope_filter,
        type_filter,
        include_globs,
        skip_gitignore,
        max_results,
    };
    let mut chunks = query_filtered_chunks(&ctx.sqlite, query, |_| true)?;
    if chunks.len() < max_results
        && let Some(base_sqlite) = &ctx.base_sqlite
    {
        let remaining = max_results - chunks.len();
        let base_chunks = query_filtered_chunks(
            base_sqlite,
            FilteredChunkQuery {
                max_results: remaining,
                ..query
            },
            |chunk| !ctx.is_shadowed_base_file(1, &chunk.file_path),
        )?;
        chunks.extend(base_chunks);
    }
    Ok(chunks)
}

fn query_filtered_chunks(
    conn: &Connection,
    query: FilteredChunkQuery<'_>,
    include_chunk: impl Fn(&RawIndexedChunk) -> bool,
) -> Result<Vec<RawIndexedChunk>> {
    // Build a SQL query that pushes as much filtering as possible into SQLite.
    let mut sql = String::from(
        "SELECT file_path, start_line, end_line,
                language, kind, x'', '', vector_key, is_ignored
         FROM chunks WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if !query.skip_gitignore {
        sql.push_str(" AND is_ignored = 0");
    }

    if let Some(tf) = query.type_filter {
        sql.push_str(" AND language = ?");
        params_vec.push(Box::new(tf.to_string()));
    }

    if let Some(scope) = query.scope_filter {
        let prefix = index_path_string(&scope.rel_path);
        if scope.is_file {
            sql.push_str(" AND file_path = ?");
            params_vec.push(Box::new(prefix));
        } else {
            let dir_prefix = format!("{prefix}/");
            sql.push_str(" AND file_path LIKE ? ESCAPE '\\'");
            params_vec.push(Box::new(format!("{}%", escape_like_pattern(&dir_prefix))));
        }
    }

    // Push exact paths and trailing-wildcard directory patterns into the
    // file_path index. Rust glob matching below remains the source of truth.
    let indexed_path_filters = query
        .include_globs
        .iter()
        .map(|glob| indexed_include_path_filter(glob))
        .collect::<Option<Vec<_>>>();
    if let Some(filters) = indexed_path_filters.filter(|filters| !filters.is_empty()) {
        let mut clauses = Vec::with_capacity(filters.len());
        for filter in filters {
            match filter {
                IndexedIncludePath::Exact(path) => {
                    clauses.push("file_path = ?");
                    params_vec.push(Box::new(path));
                }
                IndexedIncludePath::Prefix(prefix) => {
                    clauses.push("(file_path >= ? AND file_path < ?)");
                    params_vec.push(Box::new(format!("{prefix}/")));
                    params_vec.push(Box::new(format!("{prefix}0")));
                }
            }
        }
        sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
    }

    // Push simple extension globs into SQL for massive performance gains.
    // e.g., "*.yaml" -> language IN ('yaml') (Hits the SQLite index instantly!)
    // Instead of doing `file_path LIKE '%.yaml'` which triggers a full table scan.
    let mut sql_ext_filters: Vec<String> = Vec::new();
    for glob in query.include_globs {
        let trimmed = glob.trim();
        if trimmed.starts_with("*.") && !trimmed.contains('/') && !trimmed.contains('?') {
            // Simple extension glob: *.yaml, *.rs, *.py, etc.
            let ext = &trimmed[1..]; // ".yaml"
            if let Some(lang) =
                crate::chunking::language_for_path(&PathBuf::from(format!("dummy{}", ext)))
            {
                sql_ext_filters.push("language = ?".to_string());
                params_vec.push(Box::new(lang.to_string()));
            } else {
                // If we don't have a known language for this extension, we must fall back to LIKE
                sql_ext_filters.push("file_path LIKE ?".to_string());
                params_vec.push(Box::new(format!("%{}", ext)));
            }
        }
    }
    if !sql_ext_filters.is_empty() {
        sql.push_str(&format!(" AND ({})", sql_ext_filters.join(" OR ")));
    }

    let mut stmt = conn.prepare(&sql)?;

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        let file_path = PathBuf::from(row.get::<_, String>(0)?);
        let start_line = row.get::<_, i64>(1)? as usize;
        let end_line = row.get::<_, i64>(2)? as usize;
        let language = row.get::<_, String>(3)?;
        let kind = row.get::<_, String>(4)?;
        let vector_key = row.get::<_, i64>(7)? as u64;
        let raw_text: Vec<u8> = row.get(5)?;
        Ok(RawIndexedChunk {
            chunk_id: String::new(),
            file_path,
            start_line,
            end_line,
            language,
            kind,
            raw_text,
            content_hash: row.get(6)?,
            vector_key,
            is_ignored: row.get::<_, bool>(8)?,
        })
    })?;

    // Apply full glob filtering in Rust for complex patterns
    let mut chunks = Vec::new();
    for row in rows {
        let chunk = row?;
        if scope_path_matches(&chunk.file_path, query.scope_filter)
            && query.path_matcher.matches(&chunk.file_path)
            && include_chunk(&chunk)
        {
            chunks.push(chunk);
            if chunks.len() == query.max_results {
                break;
            }
        }
    }
    Ok(chunks)
}

enum IndexedIncludePath {
    Exact(String),
    Prefix(String),
}

const MAX_EXACT_FILTERED_CANDIDATES: usize = 50_000;
const MAX_FILTERED_ANN_MULTIPLIER: usize = 10;

fn broad_filter_ann_multiplier(total_docs: u64) -> usize {
    total_docs
        .saturating_mul(3)
        .div_ceil((MAX_EXACT_FILTERED_CANDIDATES as u64 + 1) * 2)
        .clamp(2, MAX_FILTERED_ANN_MULTIPLIER as u64) as usize
}

fn indexed_include_path_filter(glob: &str) -> Option<IndexedIncludePath> {
    let normalized = glob.trim().trim_start_matches("./").trim_end_matches('/');
    if normalized.is_empty() {
        return None;
    }
    let contains_meta = |value: &str| value.chars().any(|ch| matches!(ch, '*' | '?' | '[' | '{'));
    if !contains_meta(normalized) {
        return Some(IndexedIncludePath::Exact(normalized.to_string()));
    }
    for suffix in ["/**", "/*"] {
        if let Some(prefix) = normalized.strip_suffix(suffix)
            && !prefix.is_empty()
            && !contains_meta(prefix)
        {
            return Some(IndexedIncludePath::Prefix(prefix.to_string()));
        }
    }
    None
}

fn collect_semantic_candidates(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
    query_vector: &[f32],
    candidate_limit: usize,
    stores: (Option<&VectorStore>, Option<&VectorStore>),
    filter_plan: Option<&SemanticFilterPlan>,
) -> Result<Vec<(IndexedChunk, f32)>> {
    let (primary_store, base_store) = stores;
    let has_filters = has_semantic_filters(options);

    // ANN cannot push path filters into the graph. For focused searches, exact
    // scoring over the filtered subset is both complete and cheap. This avoids
    // losing the best scoped result behind globally-nearer out-of-scope chunks.
    if has_filters {
        let owned_plan;
        let plan = if let Some(plan) = filter_plan {
            plan
        } else {
            owned_plan = build_semantic_filter_plan(ctx, path_matcher, options)?;
            &owned_plan
        };
        if let SemanticFilterPlan::Exact(filtered) = plan {
            return score_filtered_semantic_candidates(
                ctx,
                filtered,
                query_vector,
                candidate_limit,
                primary_store,
                base_store,
            );
        }
    }

    let mut semantic_chunks = Vec::new();

    // Always use the ANN index for initial candidates — even when filters are
    // active. Over-fetch then post-filter is orders of magnitude faster than
    // loading all matching rows from SQLite (which could be millions for common
    // language filters like "Go" on large repos).
    let ann_limit = if has_filters {
        // Broad filters match more than 50K chunks. Size the ANN pool from that
        // guaranteed lower bound, with 1.5x safety and a bounded fallback.
        let total_docs = ctx
            .searchers
            .iter()
            .map(tantivy::Searcher::num_docs)
            .sum::<u64>();
        let multiplier = broad_filter_ann_multiplier(total_docs);
        candidate_limit.saturating_mul(multiplier).min(20_000)
    } else {
        candidate_limit
    };

    let matches =
        collect_semantic_vector_matches(query_vector, ann_limit, primary_store, base_store);

    // Batch-fetch all candidate chunks in one SQL round-trip.
    let keys: Vec<u64> = matches.iter().map(|m| m.key).collect();
    let mut batch_result = ctx.fetch_chunks_by_vector_keys_batch(&keys)?;

    for vector_match in matches {
        if let Some(chunk) = batch_result.remove(&vector_match.key)
            && (options.skip_gitignore || !chunk.is_ignored)
        {
            // Post-filter: apply type/scope/glob filters in Rust.
            if has_filters {
                if !type_matches(&chunk, options.type_filter.as_deref()) {
                    continue;
                }
                if !scope_matches(&chunk, options.scope_filter.as_ref()) {
                    continue;
                }
                if !path_matches(&chunk, path_matcher) {
                    continue;
                }
            }
            semantic_chunks.push((chunk, vector_match.score));
            if semantic_chunks.len() >= candidate_limit {
                break;
            }
        }
    }

    if has_filters && semantic_chunks.len() < candidate_limit && ann_limit < 20_000 {
        let fallback_limit = candidate_limit
            .saturating_mul(MAX_FILTERED_ANN_MULTIPLIER)
            .max(ann_limit.saturating_mul(2))
            .min(20_000);
        if fallback_limit > ann_limit {
            let fallback_matches = collect_semantic_vector_matches(
                query_vector,
                fallback_limit,
                primary_store,
                base_store,
            );
            let fallback_keys = fallback_matches
                .iter()
                .map(|vector_match| vector_match.key)
                .collect::<Vec<_>>();
            let mut fallback_chunks = ctx.fetch_chunks_by_vector_keys_batch(&fallback_keys)?;
            semantic_chunks.clear();
            for vector_match in fallback_matches {
                if let Some(chunk) = fallback_chunks.remove(&vector_match.key)
                    && (options.skip_gitignore || !chunk.is_ignored)
                    && type_matches(&chunk, options.type_filter.as_deref())
                    && scope_matches(&chunk, options.scope_filter.as_ref())
                    && path_matches(&chunk, path_matcher)
                {
                    semantic_chunks.push((chunk, vector_match.score));
                    if semantic_chunks.len() >= candidate_limit {
                        break;
                    }
                }
            }
        }
    }

    Ok(semantic_chunks)
}

/// Return true when SQLite can prove an index-backed filter exceeds the exact
/// scoring budget without materializing candidate metadata. Complex globs and
/// worktree overlays retain the existing collection path.
fn indexed_filter_exceeds_exact_limit(
    ctx: &SearchContext,
    options: &SearchOptions,
    limit: usize,
) -> Result<bool> {
    if ctx.base_sqlite.is_some()
        || !options.include_globs.is_empty()
        || !options.exclude_globs.is_empty()
    {
        return Ok(false);
    }

    let mut sql = String::from("SELECT 1 FROM chunks WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if !options.skip_gitignore {
        sql.push_str(" AND is_ignored = 0");
    }
    if let Some(type_filter) = options.canonical_type_filter() {
        sql.push_str(" AND language = ?");
        params_vec.push(Box::new(type_filter));
    }
    if let Some(scope) = options.scope_filter.as_ref() {
        let prefix = index_path_string(&scope.rel_path);
        if scope.is_file {
            sql.push_str(" AND file_path = ?");
            params_vec.push(Box::new(prefix));
        } else {
            let dir_prefix = format!("{prefix}/");
            sql.push_str(" AND file_path LIKE ? ESCAPE '\\'");
            params_vec.push(Box::new(format!("{}%", escape_like_pattern(&dir_prefix))));
        }
    }
    sql.push_str(" LIMIT 1 OFFSET ?");
    params_vec.push(Box::new(limit as i64));
    let params_refs = params_vec
        .iter()
        .map(|value| value.as_ref())
        .collect::<Vec<&dyn rusqlite::types::ToSql>>();
    match ctx
        .sqlite
        .query_row(&sql, params_refs.as_slice(), |_| Ok(()))
    {
        Ok(()) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

enum SemanticFilterPlan {
    Exact(Vec<RawIndexedChunk>),
    Broad,
}

fn build_semantic_filter_plan(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Result<SemanticFilterPlan> {
    if indexed_filter_exceeds_exact_limit(ctx, options, MAX_EXACT_FILTERED_CANDIDATES)? {
        return Ok(SemanticFilterPlan::Broad);
    }
    let type_filter = options.canonical_type_filter();
    let filtered = collect_filtered_chunks(
        ctx,
        path_matcher,
        options.scope_filter.as_ref(),
        type_filter.as_deref(),
        &options.include_globs,
        options.skip_gitignore,
        MAX_EXACT_FILTERED_CANDIDATES + 1,
    )?;
    if filtered.len() <= MAX_EXACT_FILTERED_CANDIDATES {
        Ok(SemanticFilterPlan::Exact(filtered))
    } else {
        Ok(SemanticFilterPlan::Broad)
    }
}

fn has_semantic_filters(options: &SearchOptions) -> bool {
    !options.include_globs.is_empty()
        || !options.exclude_globs.is_empty()
        || options.scope_filter.is_some()
        || options.type_filter.is_some()
}

fn collect_semantic_vector_matches(
    query_vector: &[f32],
    candidate_limit: usize,
    primary_store: Option<&VectorStore>,
    base_store: Option<&VectorStore>,
) -> Vec<VectorMatch> {
    let mut matches = Vec::new();
    if let Some(store) = primary_store {
        matches.extend(store.search(query_vector, candidate_limit));
    }
    if let Some(store) = base_store {
        matches.extend(store.search(query_vector, candidate_limit));
    }
    matches.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.key.cmp(&right.key))
    });
    let mut seen_keys = HashSet::new();
    matches.retain(|vector_match| seen_keys.insert(vector_match.key));
    matches.truncate(candidate_limit);
    matches
}

fn neural_query_vector(
    model: &dyn EmbeddingModel,
    query_text: &str,
    job: &mut Option<NeuralQueryVectorJob>,
) -> Vec<f32> {
    if let Some(mut job) = job.take() {
        let vector = job.finish();
        if let Some(vector) = vector {
            if vector.len() == model.dimensions() {
                return vector;
            }
            tracing::warn!("discarding precomputed neural query vector with wrong dimensions");
        }
    }
    model.embed(query_text)
}

fn collect_unfiltered_semantic_candidates(
    ctx: &SearchContext,
    options: &SearchOptions,
    sources: Vec<(Vec<VectorMatch>, f32, &'static str)>,
) -> Result<SemanticCandidatesById> {
    debug_assert!(!has_semantic_filters(options));

    let mut by_key = HashMap::<u64, (f32, HashSet<&'static str>)>::new();
    for (matches, multiplier, source) in sources {
        for vector_match in matches {
            let adjusted = vector_match.score * multiplier;
            by_key
                .entry(vector_match.key)
                .and_modify(|(score, source_set)| {
                    *score = score.max(adjusted);
                    source_set.insert(source);
                })
                .or_insert_with(|| (adjusted, HashSet::from([source])));
        }
    }

    let keys = by_key.keys().copied().collect::<Vec<_>>();
    let chunks = ctx.fetch_chunk_metadata_by_vector_keys_batch(&keys)?;
    Ok(by_key
        .into_iter()
        .filter_map(|(key, (score, sources))| {
            let chunk = chunks.get(&key)?.clone();
            (options.skip_gitignore || !chunk.is_ignored).then_some((key, (chunk, score, sources)))
        })
        .collect())
}

fn score_filtered_semantic_candidates(
    ctx: &SearchContext,
    filtered: &[RawIndexedChunk],
    query_vector: &[f32],
    candidate_limit: usize,
    primary_store: Option<&VectorStore>,
    base_store: Option<&VectorStore>,
) -> Result<Vec<(IndexedChunk, f32)>> {
    let keys = filtered
        .iter()
        .map(|chunk| chunk.vector_key)
        .collect::<Vec<_>>();
    let mut scores = HashMap::<u64, f32>::with_capacity(candidate_limit.saturating_mul(2));
    for store in [primary_store, base_store].into_iter().flatten() {
        for vector_match in store.score_many_top_k(&keys, query_vector, candidate_limit) {
            scores
                .entry(vector_match.key)
                .and_modify(|score| {
                    if vector_match.score.total_cmp(score).is_gt() {
                        *score = vector_match.score;
                    }
                })
                .or_insert(vector_match.score);
        }
    }
    let mut scored = filtered
        .iter()
        .filter_map(|chunk| {
            scores
                .get(&chunk.vector_key)
                .copied()
                .map(|score| (chunk.vector_key, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    scored.truncate(candidate_limit);

    let keys = scored.iter().map(|(key, _)| *key).collect::<Vec<_>>();
    let chunks = ctx.fetch_chunks_by_vector_keys_batch(&keys)?;
    Ok(scored
        .into_iter()
        .filter_map(|(key, score)| chunks.get(&key).cloned().map(|chunk| (chunk, score)))
        .collect())
}

fn merge_semantic_candidates(
    semantic_by_id: &mut SemanticCandidatesById,
    hits: Vec<(IndexedChunk, f32)>,
    score_multiplier: f32,
    source: &'static str,
) {
    for (chunk, score) in hits {
        let adjusted = score * score_multiplier;
        semantic_by_id
            .entry(chunk.vector_key)
            .and_modify(|(_, best_score, sources)| {
                *best_score = best_score.max(adjusted);
                sources.insert(source);
            })
            .or_insert_with(|| (chunk, adjusted, HashSet::from([source])));
    }
}

struct FusionQuery<'a> {
    text: &'a str,
    lower: String,
    compact: String,
    path_candidates: [String; 4],
    tokens: Vec<String>,
    primary_tokens: Vec<String>,
    primary_token_compacts: Vec<String>,
    token_compacts: Vec<String>,
    alias_token_compacts: Vec<String>,
    location_intent: bool,
    secondary_intent: bool,
    compact_candidate_text: bool,
}

impl<'a> FusionQuery<'a> {
    fn new(query_text: &'a str) -> Self {
        let text = query_text.trim();
        let lower = text.to_ascii_lowercase();
        let primary_tokens = tokenize_query(text);
        let primary_token_compacts = primary_tokens
            .iter()
            .map(|token| compact_identifier(token))
            .collect();
        let tokens = expanded_query_tokens(text);
        let token_compacts = tokens
            .iter()
            .map(|token| compact_identifier(token))
            .collect();
        let primary_token_set = primary_tokens.iter().collect::<HashSet<_>>();
        let alias_token_compacts = tokens
            .iter()
            .filter(|token| !primary_token_set.contains(token))
            .map(|token| compact_identifier(token))
            .collect();
        let compact_candidate_text = should_use_compact_identifier_matching(text, &primary_tokens);
        let path_candidates = [
            lower.clone(),
            lower.replace(' ', "-"),
            lower.replace(' ', "_"),
            lower.replace(' ', ""),
        ];
        Self {
            text,
            compact: compact_identifier(text),
            tokens,
            location_intent: has_location_intent(text),
            secondary_intent: query_targets_secondary_sources(text),
            lower,
            path_candidates,
            primary_tokens,
            primary_token_compacts,
            token_compacts,
            alias_token_compacts,
            compact_candidate_text,
        }
    }
}

struct FusionCandidates {
    lexical: Vec<(IndexedChunk, f32)>,
    semantic: Vec<(IndexedChunk, f32, HashSet<&'static str>)>,
    literal: Vec<(IndexedChunk, f32)>,
    path: Vec<(IndexedChunk, f32)>,
    path_weight: f32,
    symbols: Vec<(IndexedChunk, SymbolCandidateKind)>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SymbolCandidateKind {
    Exact,
    Inferred,
    Alias,
}

type SourceMask = u16;

const SOURCE_EXACT_SYMBOL: SourceMask = 1 << 0;
const SOURCE_HASH: SourceMask = 1 << 1;
const SOURCE_LEXICAL: SourceMask = 1 << 2;
const SOURCE_LITERAL: SourceMask = 1 << 3;
const SOURCE_NEURAL: SourceMask = 1 << 4;
const SOURCE_PATH: SourceMask = 1 << 5;
const SOURCE_SEMANTIC: SourceMask = 1 << 6;
const SOURCE_SYMBOL: SourceMask = 1 << 7;
const SOURCE_INFERRED_SYMBOL: SourceMask = 1 << 8;
const SOURCE_BACKFILL: SourceMask = 1 << 9;

#[derive(Clone)]
struct RankedCandidate {
    chunk: IndexedChunk,
    score: f32,
    sources: SourceMask,
}

impl RankedCandidate {
    fn into_tuple(self) -> (IndexedChunk, f32, Vec<String>) {
        (self.chunk, self.score, source_list(self.sources))
    }
}

fn source_bit(source: &str) -> SourceMask {
    match source {
        "exact-symbol" => SOURCE_EXACT_SYMBOL,
        "hash" => SOURCE_HASH,
        "lexical" => SOURCE_LEXICAL,
        "literal" => SOURCE_LITERAL,
        "neural" => SOURCE_NEURAL,
        "path" => SOURCE_PATH,
        "semantic" => SOURCE_SEMANTIC,
        "symbol" => SOURCE_SYMBOL,
        "inferred-symbol" => SOURCE_INFERRED_SYMBOL,
        "backfill" => SOURCE_BACKFILL,
        _ => 0,
    }
}

fn source_list(mask: SourceMask) -> Vec<String> {
    [
        ("exact-symbol", SOURCE_EXACT_SYMBOL),
        ("hash", SOURCE_HASH),
        ("lexical", SOURCE_LEXICAL),
        ("literal", SOURCE_LITERAL),
        ("neural", SOURCE_NEURAL),
        ("path", SOURCE_PATH),
        ("semantic", SOURCE_SEMANTIC),
        ("symbol", SOURCE_SYMBOL),
        ("inferred-symbol", SOURCE_INFERRED_SYMBOL),
        ("backfill", SOURCE_BACKFILL),
    ]
    .into_iter()
    .filter(|(_, bit)| mask & *bit != 0)
    .map(|(source, _)| source.to_string())
    .collect()
}

#[cfg(test)]
fn source_mask(sources: &[String]) -> SourceMask {
    sources
        .iter()
        .fold(0, |mask, source| mask | source_bit(source))
}

const MIN_FILTERED_RESULTS: usize = 10;
const REPRESENTATIVE_SPAN_MIN_COVERAGE: f32 = 0.75;
const FILE_COHERENCE_WEIGHT: f32 = 0.22;
const FILE_COHERENCE_CANDIDATES: usize = 50;
const NATURAL_LANGUAGE_PATH_FILE_LIMIT: usize = 20;
const NATURAL_LANGUAGE_PATH_DOCUMENT_OVERFETCH: usize = 4;

fn path_key(path: &Path) -> u64 {
    xxhash_rust::xxh3::xxh3_64(path.as_os_str().as_encoded_bytes())
}

fn backfill_enabled(ranked: &[RankedCandidate]) -> bool {
    let mut files = HashSet::with_capacity(MIN_FILTERED_RESULTS);
    ranked.iter().any(|item| {
        files.insert(path_key(&item.chunk.file_path)) && files.len() >= MIN_FILTERED_RESULTS
    })
}

fn apply_file_coherence_boost(ranked: &mut [RankedCandidate], weight: f32, secondary_intent: bool) {
    if weight <= 0.0 || ranked.is_empty() {
        return;
    }

    let max_score = ranked
        .iter()
        .take(FILE_COHERENCE_CANDIDATES)
        .map(|item| item.score)
        .reduce(f32::max)
        .unwrap_or(0.0);
    if max_score <= 0.0 {
        return;
    }

    // Corroborating chunks are file-level evidence. Boost only the best chunk
    // per file, and discount secondary sources unless the query requests them.
    let mut file_sums = HashMap::<u64, f32>::with_capacity(FILE_COHERENCE_CANDIDATES);
    let mut file_authorities = HashMap::<u64, f32>::with_capacity(FILE_COHERENCE_CANDIDATES);
    let mut best_indices = HashMap::<u64, usize>::with_capacity(FILE_COHERENCE_CANDIDATES);
    for (index, item) in ranked.iter().take(FILE_COHERENCE_CANDIDATES).enumerate() {
        let file = path_key(&item.chunk.file_path);
        *file_sums.entry(file).or_default() += item.score;
        file_authorities.entry(file).or_insert_with(|| {
            if secondary_intent {
                1.0
            } else {
                file_authority_score_for_path(lower_index_path(&item.chunk.file_path).as_ref())
            }
        });
        best_indices
            .entry(file)
            .and_modify(|best| {
                if item.score > ranked[*best].score {
                    *best = index;
                }
            })
            .or_insert(index);
    }

    let max_file_sum = file_sums.values().copied().reduce(f32::max).unwrap_or(0.0);
    if max_file_sum <= 0.0 {
        return;
    }

    let boost_unit = max_score * weight;
    for (file, index) in best_indices {
        ranked[index].score +=
            boost_unit * file_sums[&file] / max_file_sum * file_authorities[&file];
    }
}

fn local_term_coverage(query_tokens: &[String], text: &str) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let text = text.as_bytes();
    let matched = query_tokens
        .iter()
        .filter(|token| {
            let token = token.as_bytes();
            token.len() <= text.len()
                && text
                    .windows(token.len())
                    .any(|window| window.eq_ignore_ascii_case(token))
        })
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn promote_representative_span(
    ranked: &mut [RankedCandidate],
    query_tokens: &[String],
    min_coverage: f32,
) {
    const MIN_QUERY_TOKENS: usize = 4;
    const MIN_COVERAGE_GAIN: f32 = 0.25;
    const MAX_CANDIDATES_TO_SCAN: usize = 48;

    if query_tokens.len() < MIN_QUERY_TOKENS
        || ranked.is_empty()
        || !has_direct_source(ranked[0].sources)
    {
        return;
    }

    let first_path = path_key(&ranked[0].chunk.file_path);
    let first_coverage = local_term_coverage(query_tokens, &ranked[0].chunk.text);
    let mut best_index = 0;
    let mut best_coverage = first_coverage;
    for (index, item) in ranked.iter().take(MAX_CANDIDATES_TO_SCAN).enumerate() {
        if path_key(&item.chunk.file_path) != first_path || !has_direct_source(item.sources) {
            continue;
        }
        let coverage = local_term_coverage(query_tokens, &item.chunk.text);
        if coverage > best_coverage {
            best_index = index;
            best_coverage = coverage;
        }
    }

    if best_index == 0
        || best_coverage < min_coverage
        || best_coverage < first_coverage + MIN_COVERAGE_GAIN
    {
        return;
    }

    let (first, rest) = ranked.split_at_mut(best_index);
    std::mem::swap(&mut first[0].chunk, &mut rest[0].chunk);
}

fn promote_qualified_symbol_span(ranked: &mut [RankedCandidate], query_text: &str) {
    if ranked.is_empty() {
        return;
    }

    let names = qualified_symbol_leaf_names(query_text);
    if names.is_empty() {
        return;
    }

    let first_path = path_key(&ranked[0].chunk.file_path);
    for name in names {
        if crate::symbols::chunk_defines_exact_name(&ranked[0].chunk, &name) {
            return;
        }
        let Some(best_index) = ranked.iter().position(|item| {
            path_key(&item.chunk.file_path) == first_path
                && item.sources & SOURCE_EXACT_SYMBOL != 0
                && crate::symbols::chunk_defines_exact_name(&item.chunk, &name)
        }) else {
            continue;
        };

        let (first, rest) = ranked.split_at_mut(best_index);
        std::mem::swap(&mut first[0].chunk, &mut rest[0].chunk);
        return;
    }
}

#[cfg(test)]
fn fuse_rrf(
    candidates: FusionCandidates,
    semantic_direct_weight: f32,
    query_text: &str,
    limit: Option<usize>,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    let query = FusionQuery::new(query_text);
    let routing = QueryRouting::classify(query_text);
    fuse_rrf_with_context(
        None,
        candidates,
        None,
        semantic_direct_weight,
        &query,
        routing,
        limit,
    )
    .expect("fusion without a search context cannot perform fallible I/O")
}

pub fn rerank_candidate_limit() -> usize {
    configured_rerank_candidate_limit().unwrap_or_else(|| {
        if learned_reranker_enabled_by_env() {
            100
        } else {
            30
        }
    })
}

fn rerank_candidate_limit_for_routing(routing: QueryRouting) -> usize {
    if let Some(configured) = configured_rerank_candidate_limit() {
        return configured;
    }
    if !learned_reranker_enabled_by_env() {
        return 30;
    }
    match routing.intent {
        QueryIntent::ExactIdentifier | QueryIntent::Path | QueryIntent::LiteralOrError => 30,
        QueryIntent::NaturalLanguage | QueryIntent::DocsTestsExamples | QueryIntent::Mixed => 100,
    }
}

fn configured_rerank_candidate_limit() -> Option<usize> {
    std::env::var("IVYGREP_RERANK_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn learned_reranker_enabled_by_env() -> bool {
    match std::env::var("IVYGREP_RERANKER") {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "deterministic" | "disabled" | "off"
        ),
        Err(_) => true,
    }
}

#[cfg(test)]
fn filter_meaningful_scores(
    ranked: Vec<(IndexedChunk, f32, Vec<String>)>,
    query_text: &str,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    let query = FusionQuery::new(query_text);
    let ranked = ranked
        .into_iter()
        .map(|(chunk, score, sources)| RankedCandidate {
            chunk,
            score,
            sources: source_mask(&sources),
        })
        .collect::<Vec<_>>();
    let enable_backfill = backfill_enabled(&ranked);
    filter_meaningful_scores_with_query(ranked, &query, enable_backfill)
        .into_iter()
        .map(RankedCandidate::into_tuple)
        .collect()
}

fn filter_meaningful_scores_with_query(
    ranked: Vec<RankedCandidate>,
    query: &FusionQuery<'_>,
    enable_backfill: bool,
) -> Vec<RankedCandidate> {
    let precise_query = is_precise_lookup_query_with_tokens(query.text, &query.primary_tokens);
    let query_tokens = query.tokens.as_slice();
    let secondary_intent = query.secondary_intent;
    let implementation_intent = query_targets_implementation(query_tokens);
    let raw_terms = raw_query_terms(query.text);
    let short_literal_lookup =
        !raw_terms.is_empty() && raw_terms.len() <= 2 && !query.location_intent;
    if ranked.is_empty() {
        return vec![];
    }

    let best_score = ranked[0].score;
    let has_direct_candidate = ranked.iter().any(|item| has_direct_source(item.sources));
    if !has_direct_candidate {
        return filter_semantic_only_scores(ranked, query, precise_query);
    }

    if ranked.len() == 1 {
        let item = &ranked[0];
        if direct_candidate_has_enough_authority(
            &item.chunk,
            item.sources,
            query_tokens,
            precise_query,
            secondary_intent,
            implementation_intent,
            short_literal_lookup,
        ) {
            return ranked;
        }
        return vec![];
    }

    // Adaptive threshold: start from score distribution, then clamp against
    // the best result. Low-authority files are suppressed unless the query is
    // an exact identifier/path-style lookup with a verified literal hit; this
    // avoids fixture/data/vendor junk leaking into high-confidence advice.
    let mean = ranked.iter().map(|item| item.score).sum::<f32>() / ranked.len() as f32;
    let variance = ranked
        .iter()
        .map(|item| (item.score - mean).powi(2))
        .sum::<f32>()
        / ranked.len() as f32;
    let stddev = variance.sqrt();
    let adaptive_threshold = (mean - stddev).max(best_score * 0.35).max(0.010);

    let candidate_authority = |chunk: &IndexedChunk, sources: SourceMask| -> (f32, f32) {
        let path_lower = lower_index_path(&chunk.file_path);
        let authority =
            effective_authority_score_for_path(query_tokens, path_lower.as_ref(), secondary_intent);
        let authority_floor = recommendation_authority_floor(
            sources,
            precise_query,
            secondary_intent,
            implementation_intent,
            short_literal_lookup,
        );
        (authority, authority_floor)
    };
    let mut fallback = None;
    let mut filtered = Vec::new();
    let mut backfill = Vec::new();
    for (index, item) in ranked.into_iter().enumerate() {
        let (authority, authority_floor) = candidate_authority(&item.chunk, item.sources);
        let enough_authority = authority >= authority_floor;
        let strong_path_match = has_path_source(item.sources)
            && file_stem_boost(query_tokens, &ChunkBoostContext::new(&item.chunk)) >= 0.5;
        let meaningful = if strong_path_match {
            enough_authority
        } else if has_literal_source(item.sources) {
            enough_authority && (precise_query || item.score >= adaptive_threshold * 0.7)
        } else {
            item.score >= adaptive_threshold && enough_authority
        };

        if meaningful {
            filtered.push(item);
        } else if enable_backfill && enough_authority && backfill.len() < MIN_FILTERED_RESULTS {
            backfill.push(item);
        } else if index == 0 && has_direct_source(item.sources) && enough_authority {
            fallback = Some(item);
        }
    }
    if filtered.is_empty()
        && let Some(best) = fallback
    {
        filtered.push(best);
    }
    if enable_backfill && filtered.len() < MIN_FILTERED_RESULTS {
        let mut selected = filtered
            .iter()
            .map(|item| item.chunk.vector_key)
            .collect::<HashSet<_>>();
        let mut next_backfill_score = filtered
            .last()
            .map(|item| item.score * 0.99)
            .unwrap_or(f32::MAX);
        for mut item in backfill {
            if selected.insert(item.chunk.vector_key) {
                item.score = item.score.min(next_backfill_score);
                next_backfill_score = item.score * 0.99;
                item.sources |= SOURCE_BACKFILL;
                filtered.push(item);
                if filtered.len() == MIN_FILTERED_RESULTS {
                    break;
                }
            }
        }
    }

    filtered
}

fn filter_semantic_only_scores(
    ranked: Vec<RankedCandidate>,
    query: &FusionQuery<'_>,
    precise_query: bool,
) -> Vec<RankedCandidate> {
    let Some(best) = ranked.first() else {
        return vec![];
    };

    let bctx = ChunkBoostContext::new(&best.chunk);
    let support = support_signals(query.text, &query.tokens, &bctx);
    let authority =
        effective_authority_score_with_intent(&query.tokens, &bctx, query.secondary_intent);
    let second_score = ranked.get(1).map(|item| item.score).unwrap_or(0.0);
    let authority_floor = if query.secondary_intent || precise_query {
        0.5
    } else {
        0.65
    };
    let score_floor = if support.corroboration_count() >= 2 {
        0.035
    } else {
        0.05
    };
    let decisive_ratio = if support.corroboration_count() >= 2 {
        1.15
    } else {
        1.30
    };
    let decisive = best.score >= score_floor
        || (best.score >= score_floor * 0.8
            && second_score > f32::EPSILON
            && best.score / second_score >= decisive_ratio);

    if authority < authority_floor
        || !decisive
        || !support.is_enough_for_semantic_only(precise_query)
    {
        return vec![];
    }

    // Exact-looking queries stay single-result to avoid turning embedding
    // collisions into identifier answers. Natural-language discovery may have
    // several equally strong implementation files, so retain a small cluster
    // when every candidate independently clears authority and support gates.
    let max_results = if precise_query { 1 } else { 3 };
    let score_threshold = (best.score * 0.85).max(score_floor * 0.8);
    ranked
        .into_iter()
        .filter(|item| {
            if item.score < score_threshold {
                return false;
            }
            let bctx = ChunkBoostContext::new(&item.chunk);
            let support = support_signals(query.text, &query.tokens, &bctx);
            let authority =
                effective_authority_score_with_intent(&query.tokens, &bctx, query.secondary_intent);
            authority >= authority_floor && support.is_enough_for_semantic_only(precise_query)
        })
        .take(max_results)
        .collect()
}

fn has_direct_source(sources: SourceMask) -> bool {
    has_literal_source(sources)
        || sources & (SOURCE_LEXICAL | SOURCE_PATH | SOURCE_EXACT_SYMBOL | SOURCE_INFERRED_SYMBOL)
            != 0
}

fn has_literal_source(sources: SourceMask) -> bool {
    sources & SOURCE_LITERAL != 0
}

fn has_path_source(sources: SourceMask) -> bool {
    sources & SOURCE_PATH != 0
}

fn direct_candidate_has_enough_authority(
    chunk: &IndexedChunk,
    sources: SourceMask,
    query_tokens: &[String],
    precise_query: bool,
    secondary_intent: bool,
    implementation_intent: bool,
    short_literal_lookup: bool,
) -> bool {
    let path_lower = lower_index_path(&chunk.file_path);
    let authority =
        effective_authority_score_for_path(query_tokens, path_lower.as_ref(), secondary_intent);
    authority
        >= recommendation_authority_floor(
            sources,
            precise_query,
            secondary_intent,
            implementation_intent,
            short_literal_lookup,
        )
}

fn recommendation_authority_floor(
    sources: SourceMask,
    precise_query: bool,
    secondary_intent: bool,
    implementation_intent: bool,
    short_literal_lookup: bool,
) -> f32 {
    if secondary_intent {
        return 0.30;
    }
    if precise_query || short_literal_lookup {
        return 0.35;
    }
    if implementation_intent {
        return 0.72;
    }
    if has_literal_source(sources) {
        0.75
    } else if has_direct_source(sources) {
        0.65
    } else {
        0.70
    }
}

fn support_signals(
    query_text: &str,
    query_tokens: &[String],
    bctx: &ChunkBoostContext,
) -> SupportSignals {
    SupportSignals {
        coverage: term_coverage_boost(query_tokens, bctx),
        path_segments: path_segment_boost(query_tokens, bctx),
        file_stem: file_stem_boost(query_tokens, bctx),
        definition_name: definition_name_boost(query_tokens, bctx),
        exact_path: path_exact_match_boost(query_text, bctx),
        literal: literal_match_boost(query_text, bctx),
    }
}

fn is_precise_lookup_query_with_tokens(query: &str, tokens: &[String]) -> bool {
    !query.is_empty()
        && (tokens.len() == 1
            || query.chars().any(|ch| {
                ch == '_' || ch == '-' || ch == '/' || ch == ':' || ch.is_ascii_uppercase()
            }))
}

fn normalize_lexical_score(raw_score: f32) -> f32 {
    if raw_score.is_finite() && raw_score > 0.0 {
        (raw_score + 1.0).ln()
    } else {
        0.0
    }
}

fn normalize_semantic_score(raw_score: f32) -> f32 {
    // Cosine similarity is already in [-1, 1]; clamp to [0, 1] and apply
    // a gentle log curve to spread out the high-similarity range.
    let clamped = raw_score.clamp(0.0, 1.0);
    if clamped > 0.0 {
        (clamped * 2.0 + 1.0).ln() // maps 0→0, 0.5→0.69, 1.0→1.10
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SupportSignals {
    coverage: f32,
    path_segments: f32,
    file_stem: f32,
    definition_name: f32,
    exact_path: f32,
    literal: f32,
}

impl SupportSignals {
    fn corroboration_count(self) -> usize {
        [
            self.coverage >= 0.34,
            self.path_segments >= 0.34,
            self.file_stem >= 0.5,
            self.definition_name >= 0.34,
            self.exact_path >= 0.4,
            self.literal > 0.0,
        ]
        .into_iter()
        .filter(|matched| *matched)
        .count()
    }

    fn is_enough_for_semantic_only(self, precise_query: bool) -> bool {
        if self.exact_path >= 0.7 {
            return true;
        }
        if precise_query {
            return self.literal > 0.0
                || self.file_stem > 0.0
                || self.path_segments >= 0.34
                || self.definition_name >= 0.34;
        }
        self.corroboration_count() >= 2
            || self.coverage >= 0.5
            || self.definition_name >= 0.5
            || self.path_segments >= 0.5
    }
}

/// Precomputed lowercase text/path data for a candidate chunk.
/// Built once per candidate in `fuse_rrf` and passed to all boost functions,
/// eliminating ~10 redundant `.to_ascii_lowercase()` allocations per candidate.
struct ChunkBoostContext {
    text_lower: String,
    path_lower: String,
    /// Identifier-aware path terms used for conservative morphology matching.
    path_terms: Vec<String>,
    /// Lowercased file stem (e.g. "search" from "search.rs").
    file_stem: Option<String>,
    /// Byte range of the first meaningful line within `text_lower`.
    first_line_range: Option<std::ops::Range<usize>>,
    /// compact_identifier of the full chunk text, needed only for
    /// identifier-shaped literal matching.
    text_compact: Option<String>,
    /// compact_identifier of the file path, needed only for
    /// identifier-shaped literal matching.
    path_compact: Option<String>,
}

impl ChunkBoostContext {
    fn new(chunk: &IndexedChunk) -> Self {
        Self::new_with_compact(chunk, true)
    }

    fn new_with_compact(chunk: &IndexedChunk, include_compact: bool) -> Self {
        let text_lower = chunk.text.to_ascii_lowercase();
        let path_string = index_path_string(&chunk.file_path);
        let path_lower = path_string.to_ascii_lowercase();
        let path_terms = path_string
            .split('/')
            .flat_map(split_identifier_segments)
            .collect::<Vec<_>>();
        let file_stem = chunk
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        let mut offset = 0usize;
        let first_line_range = text_lower.split_inclusive('\n').find_map(|line| {
            let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
            let line_without_newline = line_without_newline
                .strip_suffix('\r')
                .unwrap_or(line_without_newline);
            let trimmed = line_without_newline.trim();
            let range =
                (!trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#'))
                    .then_some(offset..offset.saturating_add(line_without_newline.len()));
            offset = offset.saturating_add(line.len());
            range
        });

        let (text_compact, path_compact) = if include_compact {
            (
                Some(compact_identifier(&chunk.text)),
                Some(compact_identifier(&path_string)),
            )
        } else {
            (None, None)
        };

        Self {
            text_lower,
            path_lower,
            path_terms,
            file_stem,
            first_line_range,
            text_compact,
            path_compact,
        }
    }

    fn first_line(&self) -> &str {
        self.first_line_range
            .as_ref()
            .map_or("", |range| &self.text_lower[range.clone()])
    }
}

/// Fraction of query tokens that appear (case-insensitive) in the chunk text.
fn term_coverage_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let matched = query_tokens
        .iter()
        .filter(|t| bctx.text_lower.contains(t.as_str()))
        .count();
    matched as f32 / query_tokens.len() as f32
}

/// Boost when query tokens match file-path segments (directory/filename).
fn path_segment_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let matched = query_tokens
        .iter()
        .filter(|t| {
            bctx.path_terms
                .iter()
                .any(|term| code_term_matches(t, term))
        })
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn code_term_matches(left: &str, right: &str) -> bool {
    if left == right || left.contains(right) || right.contains(left) {
        return true;
    }
    let common = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    let shorter = left.len().min(right.len());
    common >= 5 && common.saturating_mul(3) >= shorter.saturating_mul(2)
}

/// Massive boost when the full query appears as a path segment (directory or
/// file name). Searching "my-service" should rank files under a directory
/// literally named "my-service/" far above random code mentions.
fn path_exact_match_boost(query: &str, bctx: &ChunkBoostContext) -> f32 {
    let query_context = FusionQuery::new(query);
    path_exact_match_boost_with_query(&query_context, bctx)
}

fn path_exact_match_boost_with_query(query: &FusionQuery<'_>, bctx: &ChunkBoostContext) -> f32 {
    if query.lower.is_empty() {
        return 0.0;
    }

    for seg in bctx.path_lower.split('/') {
        for candidate in &query.path_candidates {
            // Exact segment match: dir name IS the query
            if seg == candidate {
                return 1.0;
            }
            // Segment starts/ends with query (e.g. "my-service-v2")
            if seg.len() > candidate.len()
                && (seg.starts_with(candidate.as_str()) || seg.ends_with(candidate.as_str()))
            {
                return 0.7;
            }
        }
    }

    // Check if the full path contains the query as a substring
    // (e.g. path has "my-service" embedded in a longer segment)
    for candidate in &query.path_candidates {
        if candidate.len() >= 4 && bctx.path_lower.contains(candidate.as_str()) {
            return 0.4;
        }
    }

    0.0
}

fn file_stem_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    let compact_tokens = query_tokens
        .iter()
        .map(|token| compact_identifier(token))
        .collect::<Vec<_>>();
    file_stem_boost_with_compact_tokens(query_tokens, &compact_tokens, bctx)
}

fn file_stem_boost_with_compact_tokens(
    query_tokens: &[String],
    compact_tokens: &[String],
    bctx: &ChunkBoostContext,
) -> f32 {
    file_stem_signals(query_tokens, compact_tokens, &[], &[], bctx).0
}

fn file_stem_signals(
    query_tokens: &[String],
    compact_tokens: &[String],
    primary_tokens: &[String],
    primary_compact_tokens: &[String],
    bctx: &ChunkBoostContext,
) -> (f32, bool) {
    if query_tokens.is_empty() {
        return (0.0, false);
    }

    let Some(ref stem) = bctx.file_stem else {
        return (0.0, false);
    };

    let compact_stem = compact_identifier(stem);
    let exact_match = query_tokens
        .iter()
        .zip(compact_tokens)
        .any(|(token, compact_token)| *stem == *token || compact_stem == *compact_token);
    let primary_exact_match = primary_tokens
        .iter()
        .zip(primary_compact_tokens)
        .any(|(token, compact_token)| *stem == *token || compact_stem == *compact_token);
    let stem_terms = split_identifier_segments(stem);
    let partial_match = query_tokens.iter().any(|token| {
        stem_terms
            .iter()
            .any(|stem_term| code_term_matches(token, stem_term))
    });
    let primary_derivational_match = !primary_exact_match
        && primary_tokens.iter().any(|token| {
            stem_terms
                .iter()
                .any(|stem_term| token != stem_term && code_term_matches(token, stem_term))
        });

    let boost = if exact_match {
        1.0
    } else if partial_match {
        0.5
    } else {
        0.0
    };
    (boost, primary_derivational_match)
}

fn alias_file_stem_multiplier(alias_token_compacts: &[String], bctx: &ChunkBoostContext) -> f32 {
    if alias_token_compacts.is_empty() || path_role(&bctx.path_lower) != PathRole::PrimarySource {
        return 1.0;
    }

    let Some(ref stem) = bctx.file_stem else {
        return 1.0;
    };
    let compact_stem = compact_identifier(stem);
    if compact_stem.len() < 3
        || is_generic_process_alias(compact_stem.as_str())
        || !alias_token_compacts
            .iter()
            .any(|alias| alias == &compact_stem)
    {
        return 1.0;
    }

    if stem.contains('_') || stem.contains('-') {
        2.7
    } else {
        2.0
    }
}

fn primary_file_stem_multiplier(
    query_tokens: &[String],
    derivational_match: bool,
    bctx: &ChunkBoostContext,
) -> f32 {
    if query_tokens.len() < 3
        || !derivational_match
        || path_role(&bctx.path_lower) != PathRole::PrimarySource
    {
        return 1.0;
    }
    1.25
}

fn is_generic_process_alias(alias: &str) -> bool {
    matches!(alias, "daemon" | "service" | "worker")
}

fn location_intent_boost(chunk: &IndexedChunk, bctx: &ChunkBoostContext) -> f32 {
    let mut boost: f32 = 0.0;

    if is_definition_kind(&chunk.kind) {
        boost += 0.7;
    }
    if matches!(chunk.kind.as_str(), "Module" | "module") {
        boost += 0.5;
    }
    if bctx.path_lower.starts_with("src/")
        || bctx.path_lower.starts_with("app/")
        || bctx.path_lower.starts_with("lib/")
        || bctx.path_lower.starts_with("pkg/")
    {
        boost += 0.35;
    }
    if is_test_path(&bctx.path_lower) {
        boost -= 0.35;
    }

    boost.max(0.0)
}

/// Boosts definitions whose first non-blank line contains query tokens.
fn definition_name_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    let first_line = bctx.first_line();
    if query_tokens.is_empty() || first_line.is_empty() {
        return 0.0;
    }

    let matched = query_tokens
        .iter()
        .filter(|t| first_line.contains(t.as_str()))
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn literal_match_boost(query_text: &str, bctx: &ChunkBoostContext) -> f32 {
    let query = FusionQuery::new(query_text);
    literal_match_boost_with_query(&query, bctx)
}

fn literal_match_boost_with_query(query: &FusionQuery<'_>, bctx: &ChunkBoostContext) -> f32 {
    const LITERAL_MATCH_BOOST: f32 = 0.20;
    const NORMALIZED_IDENTIFIER_BOOST: f32 = 0.10;

    if query.text.is_empty() {
        return 0.0;
    }

    if bctx.text_lower.contains(&query.lower) || bctx.path_lower.contains(&query.lower) {
        return LITERAL_MATCH_BOOST;
    }

    if query.compact.is_empty() {
        return 0.0;
    }

    let text_matches = bctx
        .text_compact
        .as_deref()
        .is_some_and(|text| text.contains(&query.compact));
    let path_matches = bctx
        .path_compact
        .as_deref()
        .is_some_and(|path| path.contains(&query.compact));
    if text_matches || path_matches {
        NORMALIZED_IDENTIFIER_BOOST
    } else {
        0.0
    }
}

fn compact_identifier(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect::<String>()
}

fn chunk_kind_boost(chunk: &IndexedChunk) -> f32 {
    match chunk.kind.as_str() {
        // Definitions usually carry more authority than mentions.
        "Function" | "function" => 1.35,
        "Class" | "class" | "Struct" | "struct" | "Trait" | "trait" | "Interface" | "interface" => {
            1.4
        }
        "Impl" | "impl" | "Enum" | "enum" => 1.25,
        // Parser-derived module documentation is authoritative for conceptual
        // and architecture queries, while remaining below definition sites.
        "Documentation" | "documentation" => 1.2,

        // Imports and comments are usually supporting context.
        "Comment" | "comment" => 0.6,
        "Import" | "import" | "Use" | "use" => 0.65,

        // Generic blocks match many terms but rarely own a concept.
        "Block" | "block" => 0.75,

        _ => 1.0,
    }
}

/// File authority scoring inspired by PageRank: implementation code is usually
/// more authoritative than support files, tests, fixtures, docs, data files, and
/// vendored dependencies.
#[cfg(test)]
fn file_authority_score(bctx: &ChunkBoostContext) -> f32 {
    file_authority_score_for_path(&bctx.path_lower)
}

fn file_authority_score_for_path(path: &str) -> f32 {
    // Prefer project code over vendored and generated trees.
    if path.contains("vendor/")
        || path.contains("node_modules/")
        || path.contains("__pycache__/")
        || path.contains(".git/")
        || path.contains("target/")
        || path.contains("dist/")
        || path.contains("build/")
    {
        return 0.2;
    }

    // Generated files rarely answer code-ownership queries.
    if path.ends_with(".lock")
        || path.ends_with(".min.js")
        || path.ends_with(".min.css")
        || path.ends_with(".map")
        || path.ends_with(".sum")
    {
        return 0.2;
    }

    match path_role(path) {
        PathRole::Generated => 0.35,
        PathRole::Data => 0.4,
        PathRole::Support => 0.45,
        PathRole::Documentation => 0.5,
        PathRole::Test => 0.6,
        PathRole::PrimarySource => 1.0,
    }
}

#[cfg(test)]
fn effective_authority_score(
    query_text: &str,
    query_tokens: &[String],
    bctx: &ChunkBoostContext,
) -> f32 {
    effective_authority_score_with_intent(
        query_tokens,
        bctx,
        query_targets_secondary_sources(query_text),
    )
}

fn effective_authority_score_with_intent(
    query_tokens: &[String],
    bctx: &ChunkBoostContext,
    secondary_intent: bool,
) -> f32 {
    effective_authority_score_for_path(query_tokens, &bctx.path_lower, secondary_intent)
}

fn effective_authority_score_for_path(
    query_tokens: &[String],
    path_lower: &str,
    secondary_intent: bool,
) -> f32 {
    let mut score = file_authority_score_for_path(path_lower);

    if !secondary_intent {
        if path_depth(path_lower) <= 3 && path_role(path_lower) == PathRole::PrimarySource {
            score *= 1.08;
        }
        if path_depth(path_lower) >= 6 {
            score *= match path_query_overlap_for_path(query_tokens, path_lower) {
                0 => 0.74,
                1 => 0.86,
                _ => 0.95,
            };
        }
    }

    score
}

fn lower_index_path(path: &Path) -> Cow<'_, str> {
    let raw = path.to_string_lossy();
    let needs_separator_normalization =
        std::path::MAIN_SEPARATOR != '/' && raw.contains(std::path::MAIN_SEPARATOR);
    let needs_lowercase = raw.bytes().any(|byte| byte.is_ascii_uppercase());
    if !needs_separator_normalization && !needs_lowercase {
        return raw;
    }

    let mut normalized = raw.into_owned();
    if needs_separator_normalization {
        normalized = normalized.replace(std::path::MAIN_SEPARATOR, "/");
    }
    normalized.make_ascii_lowercase();
    Cow::Owned(normalized)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathRole {
    PrimarySource,
    Test,
    Documentation,
    Generated,
    Data,
    Support,
}

fn path_role(path: &str) -> PathRole {
    if path.contains("generated/")
        || path.contains("__snapshots__/")
        || path.contains("fixtures/")
        || path.contains("testdata/")
        || path.contains("test_data/")
    {
        return PathRole::Generated;
    }
    if is_test_path(path) {
        return PathRole::Test;
    }
    if path.ends_with(".md") || path.ends_with(".txt") || path.ends_with(".rst") {
        return PathRole::Documentation;
    }
    if is_data_or_config_path(path) {
        return PathRole::Data;
    }
    if is_support_path(path) {
        return PathRole::Support;
    }
    PathRole::PrimarySource
}

fn is_data_or_config_path(path: &str) -> bool {
    path.ends_with(".json")
        || path.ends_with(".csv")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".xml")
        || path.ends_with(".toml")
        || path.ends_with(".ini")
        || path.ends_with(".env")
        || path.ends_with(".sql")
}

fn is_support_path(path: &str) -> bool {
    has_path_segment(path, "tools")
        || has_path_segment(path, "tooling")
        || has_path_segment(path, "scripts")
        || has_path_segment(path, "script")
        || has_path_segment(path, "examples")
        || has_path_segment(path, "example")
        || has_path_segment(path, "samples")
        || has_path_segment(path, "sample")
        || has_path_segment(path, "demos")
        || has_path_segment(path, "demo")
        || has_path_segment(path, "bench")
        || has_path_segment(path, "benches")
        || has_path_segment(path, "benchmarks")
}

fn has_path_segment(path: &str, needle: &str) -> bool {
    path.split('/').any(|segment| segment == needle)
}

fn path_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

#[cfg(test)]
fn path_query_overlap(query_tokens: &[String], bctx: &ChunkBoostContext) -> usize {
    path_query_overlap_for_path(query_tokens, &bctx.path_lower)
}

fn path_query_overlap_for_path(query_tokens: &[String], path_lower: &str) -> usize {
    let file_stem = Path::new(path_lower)
        .file_stem()
        .and_then(|stem| stem.to_str());
    let mut matched_tokens = Vec::<&str>::new();
    for token in query_tokens {
        let matches_path = path_lower
            .split('/')
            .any(|segment| segment.contains(token.as_str()))
            || file_stem.is_some_and(|stem| stem.contains(token.as_str()));
        if matches_path
            && !matched_tokens
                .iter()
                .any(|matched| matched.contains(token.as_str()) || token.contains(matched))
        {
            matched_tokens.push(token);
        }
    }
    matched_tokens.len()
}

fn chunk_density_exponent(bctx: &ChunkBoostContext) -> f32 {
    if path_role(&bctx.path_lower) == PathRole::PrimarySource {
        0.0
    } else {
        0.3
    }
}

fn query_targets_implementation(query_tokens: &[String]) -> bool {
    query_tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "implement"
                | "implementation"
                | "implemented"
                | "defined"
                | "definition"
                | "dispatch"
                | "handler"
                | "loader"
                | "parser"
                | "ranking"
                | "score"
                | "calculate"
                | "refresh"
                | "detect"
        )
    })
}

fn is_test_path(path: &str) -> bool {
    // Directory-level signals (path segments that are test directories)
    path.contains("/tests/")
        || path.contains("/test/")
        || path.contains("/__tests__/")
        || path.contains("/spec/")
        || path.contains("/specs/")
        || path.contains("/mocks/")
        || path.contains("/mock/")
        || path.contains("/selftests/")
        || path.contains("/__mocks__/")
        || path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("spec/")
        || path.starts_with("selftests/")
        // File-level signals (naming conventions across languages)
        || path.contains("_test.")    // Go, Rust: foo_test.go, foo_test.rs
        || path.contains(".test.")    // JS/TS: foo.test.ts, foo.test.js
        || path.contains("_spec.")    // Ruby, JS: foo_spec.rb, foo.spec.ts
        || path.contains(".spec.")    // JS/TS: foo.spec.ts
        || path.contains("_mock.")    // foo_mock.go, foo_mock.rs
        || path.contains(".mock.")    // foo.mock.ts
        || path.ends_with("_test.rs")
        || path.ends_with("_test.go")
        // Filename-prefix conventions
        || path.contains("/test_")    // Python: test_handler.py
        || path.starts_with("test_") // Python: test_handler.py (at root)
}

pub fn workspace_has_results(workspace: &Workspace) -> Result<bool> {
    let conn: Connection = open_sqlite_readonly(&workspace.sqlite_path())?;
    // Check cached stats first (O(1)), fall back to EXISTS which stops at first row
    let count: i64 = conn
        .query_row(
            "SELECT value FROM _stats WHERE key = 'chunk_count'",
            [],
            |row| row.get(0),
        )
        .or_else(|_| conn.query_row("SELECT 1 FROM chunks LIMIT 1", [], |row| row.get(0)))
        .unwrap_or(0);
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serial_test::serial;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
    use crate::indexer::{enhance_workspace_hash, index_workspace};
    use crate::workspace::{Workspace, WorkspaceScope};

    use super::*;

    #[test]
    fn snippet_bounds_saturate_extreme_context() {
        assert_eq!(snippet_bounds(1, usize::MAX, 3), (1, 3));
        assert_eq!(snippet_bounds(usize::MAX, usize::MAX, 3), (1, 3));
    }

    #[test]
    fn explicit_limits_are_bounded_without_changing_no_limit_sentinel() {
        let mut options = SearchOptions {
            limit: Some(MAX_SEARCH_RESULT_LIMIT + 1),
            ..Default::default()
        };
        assert_eq!(options.bounded_limit(), Some(MAX_SEARCH_RESULT_LIMIT));
        options.limit = Some(usize::MAX);
        assert_eq!(options.bounded_limit(), Some(usize::MAX));
    }

    #[test]
    fn query_routing_covers_search_intents_without_corpus_rules() {
        let cases = [
            ("parse_request", QueryIntent::ExactIdentifier, false),
            ("src/search.rs", QueryIntent::Path, false),
            (
                "error: connection refused",
                QueryIntent::LiteralOrError,
                false,
            ),
            (
                "def parse_request(payload):\n    value = payload.get(\"value\")\n    return normalize and validate the incoming request before dispatching it",
                QueryIntent::LiteralOrError,
                true,
            ),
            (
                "Using the given code, format the number 7.321 to contain two decimal points and return the transformed value without changing unrelated behavior",
                QueryIntent::NaturalLanguage,
                true,
            ),
            (
                "where in the code is the request authentication policy evaluated before a handler is dispatched",
                QueryIntent::NaturalLanguage,
                true,
            ),
            (
                "show an example test for retry behavior",
                QueryIntent::DocsTestsExamples,
                true,
            ),
            ("python sort list descending", QueryIntent::Mixed, true),
        ];
        for (query, expected_intent, expected_neural) in cases {
            let routing = QueryRouting::classify(query);
            assert_eq!(routing.intent, expected_intent, "{query}");
            assert_eq!(routing.use_neural, expected_neural, "{query}");
        }
    }

    #[test]
    fn neural_query_precompute_follows_search_routing() {
        assert!(!query_uses_neural("NeuralQueryVectorJob", false));
        assert!(query_uses_neural("where is query caching handled", false));
        assert!(query_uses_neural("NeuralQueryVectorJob", true));
    }

    #[test]
    fn corpus_candidate_budgets_scale_at_stable_boundaries() {
        assert_eq!(corpus_candidate_multiplier(50_000), 1);
        assert_eq!(corpus_candidate_multiplier(50_001), 2);
        assert_eq!(corpus_candidate_multiplier(500_000), 2);
        assert_eq!(corpus_candidate_multiplier(500_001), 3);
    }

    #[test]
    fn broad_filter_ann_budget_scales_from_exact_filter_threshold() {
        assert_eq!(broad_filter_ann_multiplier(50_001), 2);
        assert_eq!(broad_filter_ann_multiplier(100_000), 3);
        assert_eq!(broad_filter_ann_multiplier(200_000), 6);
        assert_eq!(broad_filter_ann_multiplier(400_000), 10);
    }

    #[test]
    fn query_routing_p95_is_below_two_milliseconds() {
        let queries = [
            "parse_request",
            "src/search.rs",
            "error: connection refused",
            "where in the code is the request authentication policy evaluated before a handler is dispatched",
            "show an example test for retry behavior",
            "def parse_request(payload):\n    value = payload.get(\"value\")\n    return normalize and validate the incoming request before dispatching it",
        ];
        let mut per_query_ns = Vec::with_capacity(200);
        for _ in 0..200 {
            let started = std::time::Instant::now();
            for _ in 0..100 {
                for query in queries {
                    std::hint::black_box(QueryRouting::classify(query));
                }
            }
            per_query_ns.push(started.elapsed().as_nanos() / (100 * queries.len()) as u128);
        }
        per_query_ns.sort_unstable();
        let p95_ns = per_query_ns[189];
        eprintln!("query routing p95: {:.3} ms", p95_ns as f64 / 1_000_000.0);
        assert!(p95_ns < 2_000_000, "routing p95 was {p95_ns} ns");
    }

    #[test]
    fn hash_weight_tracks_partial_neural_coverage() {
        assert_eq!(semantic_hash_weight(false, 0, 100), 1.0);
        assert_eq!(semantic_hash_weight(true, 0, 100), 1.0);
        assert!((semantic_hash_weight(true, 1, 100) - 0.99).abs() < 0.001);
        assert!((semantic_hash_weight(true, 50, 100) - 0.5).abs() < 0.001);
        assert_eq!(semantic_hash_weight(true, 100, 100), 0.3);
    }

    #[test]
    fn neural_fallback_uses_lexical_confidence() {
        let routed = QueryRouting::classify("where is query caching handled");
        assert!(neural_fallback_needed(routed, false, None, None));
        assert!(neural_fallback_needed(routed, false, Some(1.99), Some(1.0)));
        assert!(neural_fallback_needed(routed, false, Some(5.0), Some(4.8)));
        assert!(!neural_fallback_needed(routed, false, Some(5.0), Some(4.5)));

        let exact = QueryRouting::classify("SearchContext");
        assert!(!neural_fallback_needed(exact, false, None, None));
        assert!(neural_fallback_needed(exact, true, Some(5.0), Some(4.5)));
    }

    #[test]
    fn fusion_query_compacts_only_identifier_shaped_candidates() {
        assert!(FusionQuery::new("Router").compact_candidate_text);
        assert!(FusionQuery::new("path_router").compact_candidate_text);
        assert!(FusionQuery::new("error handling").compact_candidate_text);
        assert!(FusionQuery::new("how Router stores matching routes").compact_candidate_text);
        assert!(!FusionQuery::new("how routes are stored and matched").compact_candidate_text);
        assert!(PresentationQuery::new("apply filters").compact_matching);
        assert!(!PresentationQuery::new("how routes are stored and matched").compact_matching);
    }

    struct CountingEmbeddingModel {
        dimensions: usize,
        calls: AtomicUsize,
    }

    impl CountingEmbeddingModel {
        fn new(dimensions: usize) -> Self {
            Self {
                dimensions,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl EmbeddingModel for CountingEmbeddingModel {
        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn embed(&self, _text: &str) -> Vec<f32> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            vec![1.0; self.dimensions]
        }
    }

    #[test]
    fn neural_query_vector_uses_precomputed_job() {
        let model = CountingEmbeddingModel::new(3);
        let mut job = Some(NeuralQueryVectorJob::pending(std::thread::spawn(|| {
            vec![0.25, 0.5, 0.75]
        })));

        let vector = neural_query_vector(&model, "ignored", &mut job);

        assert_eq!(vector, vec![0.25, 0.5, 0.75]);
        assert_eq!(model.calls.load(Ordering::Relaxed), 0);
        assert!(job.is_none());
    }

    #[test]
    fn dropping_rapid_neural_precompute_jobs_leaves_no_detached_workers() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        for _ in 0..32 {
            let worker_active = active.clone();
            let worker_max = max_active.clone();
            let job = NeuralQueryVectorJob::pending(std::thread::spawn(move || {
                let current = worker_active.fetch_add(1, Ordering::SeqCst) + 1;
                worker_max.fetch_max(current, Ordering::SeqCst);
                worker_active.fetch_sub(1, Ordering::SeqCst);
                vec![1.0; 3]
            }));
            drop(job);
            assert_eq!(active.load(Ordering::SeqCst), 0);
        }

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn neural_query_vector_uses_cached_vector() {
        let model = CountingEmbeddingModel::new(3);
        let mut job = Some(NeuralQueryVectorJob::Ready(vec![0.25, 0.5, 0.75]));

        let vector = neural_query_vector(&model, "ignored", &mut job);

        assert_eq!(vector, vec![0.25, 0.5, 0.75]);
        assert_eq!(model.calls.load(Ordering::Relaxed), 0);
        assert!(job.is_none());
    }

    #[test]
    fn neural_query_vector_falls_back_on_wrong_dimensions() {
        let model = CountingEmbeddingModel::new(3);
        let mut job = Some(NeuralQueryVectorJob::pending(std::thread::spawn(|| {
            vec![0.25, 0.5]
        })));

        let vector = neural_query_vector(&model, "fallback", &mut job);

        assert_eq!(vector, vec![1.0, 1.0, 1.0]);
        assert_eq!(model.calls.load(Ordering::Relaxed), 1);
        assert!(job.is_none());
    }

    #[test]
    fn requested_corrupt_vector_store_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.usearch");
        std::fs::write(&path, b"not a vector store").unwrap();

        let err = open_optional_vector_store(true, &path, 256, HASH_VECTOR_QUANTIZATION)
            .err()
            .unwrap();
        assert!(err.to_string().contains("open vector store"));
    }

    #[test]
    fn unused_vector_store_is_not_opened() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.usearch");
        std::fs::write(&path, b"not a vector store").unwrap();

        assert!(
            open_optional_vector_store(false, &path, 256, HASH_VECTOR_QUANTIZATION)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn filtered_candidate_storage_errors_are_not_empty_results() {
        let connection = Connection::open_in_memory().unwrap();
        let matcher = PathGlobMatcher::new(&[], &[]).unwrap();
        let error = query_filtered_chunks(
            &connection,
            FilteredChunkQuery {
                path_matcher: &matcher,
                scope_filter: None,
                type_filter: None,
                include_globs: &[],
                skip_gitignore: false,
                max_results: 10,
            },
            |_| true,
        )
        .unwrap_err();

        assert!(error.to_string().contains("no such table"));
    }

    #[test]
    #[serial]
    fn semantic_type_alias_filter_matches_canonical_language() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let ctx = SearchContext::load(&workspace, None, false).unwrap();
        let matcher = PathGlobMatcher::new(&[], &[]).unwrap();

        let exact_paths = |type_filter: &str| {
            let options = SearchOptions {
                type_filter: Some(type_filter.to_string()),
                ..Default::default()
            };
            match build_semantic_filter_plan(&ctx, &matcher, &options).unwrap() {
                SemanticFilterPlan::Exact(chunks) => chunks
                    .into_iter()
                    .map(|chunk| chunk.file_path)
                    .collect::<Vec<_>>(),
                SemanticFilterPlan::Broad => panic!("small fixture should use exact filtering"),
            }
        };

        assert_eq!(exact_paths("rs"), exact_paths("rust"));
        assert_eq!(exact_paths("rs"), vec![PathBuf::from("lib.rs")]);
        assert!(exact_paths("unknown-language").is_empty());
    }

    struct TestEmbeddingModel384;

    impl EmbeddingModel for TestEmbeddingModel384 {
        fn dimensions(&self) -> usize {
            384
        }

        fn embed(&self, text: &str) -> Vec<f32> {
            let mut vector = vec![0.0; 384];
            for token in tokenize_query(text) {
                let idx = token.bytes().fold(0usize, |acc, b| acc + b as usize) % 384;
                vector[idx] += 1.0;
            }
            vector
        }

        fn profile_info(&self) -> Option<&'static str> {
            Some("general")
        }

        fn model_identity(&self) -> Option<&crate::embedding::NeuralModelIdentity> {
            static IDENTITY: std::sync::OnceLock<crate::embedding::NeuralModelIdentity> =
                std::sync::OnceLock::new();
            Some(IDENTITY.get_or_init(|| crate::embedding::NeuralProfile::General.identity()))
        }
    }

    fn assert_hybrid_search_scope_filter(scope_dir: &str, out_of_scope_dirs: &[&str]) {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join(scope_dir)).unwrap();
        std::fs::write(
            tmp.path().join(scope_dir).join("match.rs"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();

        for dir in out_of_scope_dirs {
            std::fs::create_dir_all(tmp.path().join(dir)).unwrap();
            std::fs::write(
                tmp.path().join(dir).join("match.rs"),
                "pub fn applyFilter() -> bool { true }\n",
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        enhance_workspace_hash(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "applyFilter",
            Some(&model),
            &SearchOptions {
                limit: None,
                context: 2,
                type_filter: None,
                include_globs: vec![],
                exclude_globs: vec![],
                scope_filter: Some(WorkspaceScope {
                    rel_path: PathBuf::from(scope_dir),
                    is_file: false,
                }),
                skip_gitignore: false,
                force_neural: false,
                progress_tx: None,
                cancel_token: None,
            },
        )
        .unwrap();

        assert!(!hits.is_empty());
        assert!(hits[0].sources.iter().any(|source| source == "lexical"));
        assert!(hits[0].sources.iter().any(|source| source == "semantic"));
        assert!(hits[0].sources.iter().any(|source| source == "hash"));

        let files = hits
            .iter()
            .map(|hit| hit.file_path.clone())
            .collect::<HashSet<_>>();
        assert_eq!(
            files,
            HashSet::from([PathBuf::from(format!("{scope_dir}/match.rs"))])
        );
        assert!(
            hits.iter()
                .all(|hit| hit.file_path.starts_with(Path::new(scope_dir)))
        );
    }

    #[test]
    fn escape_like_pattern_escapes_sql_wildcards() {
        assert_eq!(
            escape_like_pattern(r"test_utils\match%"),
            r"test\_utils\\match\%"
        );
    }

    #[test]
    #[serial]
    fn hybrid_search_returns_hits() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("tax.rs"),
            "pub fn calculate_sales_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        enhance_workspace_hash(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "where is tax calculated",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();

        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("calculate_sales_tax"));
    }

    #[test]
    #[serial]
    fn hybrid_search_prefers_exact_lexical_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("exact.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("semantic.rs"),
            "pub fn process_rules(items: &[i32]) -> Vec<i32> { items.to_vec() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "applyFilter",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();

        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("applyFilter"));
        assert!(hits[0].sources.iter().any(|source| source == "lexical"));
    }

    #[test]
    #[serial]
    fn default_hit_context_is_compact() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let mut content = String::new();
        for i in 0..30 {
            if i == 19 {
                content.push_str(
                    "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
                );
            } else {
                content.push_str(&format!("// filler line {}\n", i + 1));
            }
        }

        std::fs::write(tmp.path().join("sample.rs"), content).unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "applyFilter",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());

        let top = &hits[0];
        assert!(top.end_line >= top.start_line);
        assert!(top.end_line - top.start_line <= 4);
        assert!(top.preview.lines().count() <= 5);
        assert!(!top.reason.is_empty());
    }

    #[test]
    #[serial]
    fn hybrid_search_matches_phrase_to_camel_case_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let mut noisy = String::new();
        for _ in 0..200 {
            noisy.push_str("void enforceLimits() {}\n");
        }

        std::fs::write(tmp.path().join("noisy.java"), noisy).unwrap();
        std::fs::write(
            tmp.path().join("exact.java"),
            "class Filters {\n    void applyLimit() {}\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "apply limits",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|hit| hit.preview.contains("applyLimit")));
        assert!(hits[0].preview.contains("applyLimit"));
    }

    #[test]
    #[serial]
    fn hybrid_search_respects_scope_filter() {
        assert_hybrid_search_scope_filter("scoped", &["other"]);
    }

    #[test]
    #[serial]
    fn hybrid_search_respects_scope_filter_with_underscore_in_directory_name() {
        assert_hybrid_search_scope_filter(
            "test_utils",
            &["testXutils", "test.utils", "test_utils_extra"],
        );
    }

    #[test]
    #[serial]
    fn hybrid_search_respects_scope_filter_with_percent_in_directory_name() {
        assert_hybrid_search_scope_filter("test%utils", &["testXutils", "testXXutils"]);
    }

    #[test]
    #[serial]
    fn search_works_with_hash_only_no_neural() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("payments.rs"),
            "pub fn process_payment(amount: f64, method: &str) -> bool { amount > 0.0 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        enhance_workspace_hash(&workspace, &model).unwrap();

        // No neural store — should fall back to hash vectors
        assert!(!workspace.vector_neural_path().exists());

        let hits = hybrid_search(
            &workspace,
            "process payment",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("process_payment"));
        assert!(
            hits.iter()
                .any(|hit| hit.sources.iter().any(|source| source == "semantic")),
            "hash vector search should contribute before neural vectors exist"
        );
    }

    #[test]
    #[serial]
    fn hybrid_search_bounds_near_maximum_explicit_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("limits.rs"),
            "pub fn overflow_boundary_marker() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "overflow_boundary_marker",
            Some(&model),
            &SearchOptions {
                limit: Some(usize::MAX - 1),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    #[serial]
    fn search_uses_neural_vectors_only_for_low_confidence_queries() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("service.rs"),
            "pub fn authenticate_service(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let neural_model = TestEmbeddingModel384;
        index_workspace(&workspace, &hash_model).unwrap();
        enhance_workspace_hash(&workspace, &hash_model).unwrap();
        assert!(!workspace.vector_neural_path().exists());

        let hits_before = hybrid_search(
            &workspace,
            "authenticate user",
            Some(&neural_model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits_before.is_empty());
        assert!(
            hits_before
                .iter()
                .any(|hit| hit.sources.iter().any(|source| source == "hash")),
            "384-dim search should still use hash vectors before neural vectors exist"
        );

        crate::indexer::enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert!(workspace.vector_neural_path().exists());

        let hits_after = hybrid_search(
            &workspace,
            "authenticate user",
            Some(&neural_model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits_after.is_empty());
        assert!(hits_after[0].preview.contains("authenticate_user"));
        assert!(
            hits_after
                .iter()
                .all(|hit| hit.sources.iter().all(|source| source != "neural")),
            "confident lexical evidence should skip neural retrieval"
        );

        let ambiguous = hybrid_search(
            &workspace,
            "authenticate account",
            Some(&neural_model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(
            ambiguous
                .iter()
                .any(|hit| hit.sources.iter().any(|source| source == "neural")),
            "ambiguous lexical evidence should execute neural retrieval"
        );
        let exact_default = hybrid_search(
            &workspace,
            "authenticate_user",
            Some(&neural_model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(
            exact_default
                .iter()
                .all(|hit| hit.sources.iter().all(|source| source != "neural")),
            "exact identifiers should keep neural disabled by default"
        );

        let exact_forced = hybrid_search(
            &workspace,
            "authenticate_user",
            Some(&neural_model),
            &SearchOptions {
                force_neural: true,
                ..SearchOptions::default()
            },
        )
        .unwrap();
        assert!(
            exact_forced
                .iter()
                .any(|hit| hit.sources.iter().any(|source| source == "neural")),
            "forced neural routing must execute neural retrieval"
        );
    }

    #[test]
    #[serial]
    fn shared_semantic_hydration_matches_separate_unfiltered_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("payment.rs"),
            "pub fn process_payment(amount: u64) -> u64 { amount }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let neural_model = TestEmbeddingModel384;
        index_workspace(&workspace, &hash_model).unwrap();
        enhance_workspace_hash(&workspace, &hash_model).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &neural_model).unwrap();

        let context =
            SearchContext::load(&workspace, Some(neural_model.dimensions()), true).unwrap();
        let options = SearchOptions::default();
        let path_matcher =
            PathGlobMatcher::new(&options.include_globs, &options.exclude_globs).unwrap();
        let query = "authenticate user";
        let candidate_limit = 50;
        let hash_query_model = HashEmbeddingModel::new(256);
        let hash_query_vector = hash_query_model.embed(&build_semantic_query_text(query));
        let neural_query_vector = neural_model.embed(query);
        let hash_count = context.hash_vectors.as_ref().map_or(0, VectorStore::size);
        let neural_count = context.neural_vectors.as_ref().map_or(0, VectorStore::size);
        let hash_weight = semantic_hash_weight(true, neural_count, hash_count);

        let mut separate = HashMap::new();
        let hash_hits = collect_semantic_candidates(
            &context,
            &path_matcher,
            &options,
            &hash_query_vector,
            candidate_limit,
            (
                context.hash_vectors.as_ref(),
                context.base_hash_vectors.as_ref(),
            ),
            None,
        )
        .unwrap();
        merge_semantic_candidates(&mut separate, hash_hits, hash_weight, "hash");
        let neural_hits = collect_semantic_candidates(
            &context,
            &path_matcher,
            &options,
            &neural_query_vector,
            candidate_limit,
            (
                context.neural_vectors.as_ref(),
                context.base_neural_vectors.as_ref(),
            ),
            None,
        )
        .unwrap();
        merge_semantic_candidates(&mut separate, neural_hits, 1.08, "neural");

        let shared = collect_unfiltered_semantic_candidates(
            &context,
            &options,
            vec![
                (
                    collect_semantic_vector_matches(
                        &hash_query_vector,
                        candidate_limit,
                        context.hash_vectors.as_ref(),
                        context.base_hash_vectors.as_ref(),
                    ),
                    hash_weight,
                    "hash",
                ),
                (
                    collect_semantic_vector_matches(
                        &neural_query_vector,
                        candidate_limit,
                        context.neural_vectors.as_ref(),
                        context.base_neural_vectors.as_ref(),
                    ),
                    1.08,
                    "neural",
                ),
            ],
        )
        .unwrap();

        assert_eq!(shared.len(), separate.len());
        for (key, (separate_chunk, separate_score, separate_sources)) in separate {
            let (shared_chunk, shared_score, shared_sources) = shared
                .get(&key)
                .expect("shared hydration must preserve every key");
            assert_eq!(shared_chunk.chunk_id, separate_chunk.chunk_id);
            assert!(
                !separate_chunk.text.is_empty(),
                "the full-fetch control should include stored chunk text"
            );
            assert!(
                shared_chunk.text.is_empty(),
                "shared ANN discovery should defer text hydration to fusion"
            );
            assert_eq!(*shared_score, separate_score);
            assert_eq!(shared_sources, &separate_sources);
        }
    }

    #[test]
    #[serial]
    fn search_uses_neural_vectors_when_available() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        // Search before neural enhancement
        let hits_before = hybrid_search(
            &workspace,
            "authenticate user",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits_before.is_empty());

        // Run neural enhancement (using hash model as stand-in)
        crate::indexer::enhance_workspace_neural(&workspace, &model).unwrap();
        assert!(workspace.vector_neural_path().exists());

        // Search after neural enhancement — should still work
        let hits_after = hybrid_search(
            &workspace,
            "authenticate user",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits_after.is_empty());
        assert!(hits_after[0].preview.contains("authenticate_user"));
    }

    #[test]
    #[serial]
    fn hash_fallback_covers_chunks_without_neural_vectors() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        // File A: indexed and neural-enhanced (gets a neural vector).
        std::fs::write(
            tmp.path().join("auth.rs"),
            "pub fn authenticate_user(token: &str) -> bool { !token.is_empty() }\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let neural_model = TestEmbeddingModel384;
        index_workspace(&workspace, &hash_model).unwrap();
        enhance_workspace_hash(&workspace, &hash_model).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &neural_model).unwrap();
        assert!(workspace.vector_neural_path().exists());

        // File B added AFTER enhancement: it has hash vectors but no neural
        // vector yet — a normal partial-coverage state during incremental
        // background enhancement.
        std::fs::write(
            tmp.path().join("payment.rs"),
            "pub fn process_payment_refund(amount: u64) -> u64 { amount }\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        index_workspace(&workspace, &hash_model).unwrap();
        enhance_workspace_hash(&workspace, &hash_model).unwrap();

        // Searching B's content with the neural model must still yield a
        // semantic candidate for B via the hash fallback, even though neural
        // coverage is partial. Without the fallback, B would have no semantic
        // source until enhancement completes.
        let hits = hybrid_search(
            &workspace,
            "process payment refund",
            Some(&neural_model),
            &SearchOptions::default(),
        )
        .unwrap();
        let b_hit = hits
            .iter()
            .find(|h| h.preview.contains("process_payment_refund"));
        assert!(b_hit.is_some(), "payment.rs should be found");
        assert!(
            b_hit.unwrap().sources.iter().any(|s| s == "hash"),
            "a chunk without a neural vector should still get a semantic candidate via the hash fallback"
        );
    }

    #[test]
    #[serial]
    fn search_after_reindex_and_enhance_returns_new_content() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("v1.rs"),
            "pub fn original_func() -> i32 { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &model).unwrap();

        // Add new file, re-index, re-enhance
        std::fs::write(
            tmp.path().join("v2.rs"),
            "pub fn payment_gateway(amount: f64) -> bool { amount > 0.0 }\n",
        )
        .unwrap();
        index_workspace(&workspace, &model).unwrap();
        crate::indexer::enhance_workspace_neural(&workspace, &model).unwrap();

        // Should find the new content
        let hits = hybrid_search(
            &workspace,
            "payment gateway",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("payment_gateway"));
    }

    #[test]
    #[serial]
    fn workspace_has_results_after_indexing() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("lib.rs"),
            "pub fn has_results() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert!(workspace_has_results(&workspace).unwrap());
    }

    #[test]
    #[serial]
    fn workspace_has_no_results_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert!(!workspace_has_results(&workspace).unwrap());
    }

    #[test]
    #[serial]
    fn literal_search_finds_exact_match() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("tax.rs"),
            "pub fn calculate_sales_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits =
            literal_search(&workspace, "calculate_sales_tax", &SearchOptions::default()).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("calculate_sales_tax"));
    }

    #[test]
    #[serial]
    fn literal_search_discards_results_when_pre_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join("match.rs"), "fn cancelled_match() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let options = SearchOptions {
            cancel_token: Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                true,
            ))),
            ..SearchOptions::default()
        };

        let hits = literal_search(&workspace, "cancelled_match", &options).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    #[serial]
    fn literal_search_finds_substring_inside_identifier_token() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("filter.rs"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = literal_search(&workspace, "ppl", &SearchOptions::default()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("filter.rs"));
    }

    #[test]
    #[serial]
    fn literal_search_deduplicates_overlapping_structural_chunks() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("filter.rs"),
            "struct Filter;\nimpl Filter {\n    fn apply_filter(&self) -> bool { true }\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = literal_search(&workspace, "apply_filter", &SearchOptions::default()).unwrap();
        assert_eq!(hits.len(), 1, "same source line must appear once: {hits:?}");
        assert!(hits[0].preview.contains("fn apply_filter"));
    }

    #[test]
    #[serial]
    fn literal_search_limit_returns_deterministic_equal_score_subset() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        for i in 0..40 {
            std::fs::write(
                tmp.path().join(format!("file_{i:03}.rs")),
                "pub fn common_literal_marker() -> bool { true }\n",
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let options = SearchOptions {
            limit: Some(5),
            context: 0,
            ..SearchOptions::default()
        };

        let first = literal_search(&workspace, "common_literal_marker", &options).unwrap();
        let first_paths: Vec<_> = first.iter().map(|hit| hit.file_path.clone()).collect();
        let mut sorted_paths = first_paths.clone();
        sorted_paths.sort();
        assert_eq!(first_paths, sorted_paths);

        for _ in 0..5 {
            let repeated = literal_search(&workspace, "common_literal_marker", &options).unwrap();
            let repeated_paths: Vec<_> = repeated.iter().map(|hit| hit.file_path.clone()).collect();
            assert_eq!(repeated_paths, first_paths);
        }
    }

    #[test]
    #[serial]
    fn glob_filters_survive_global_candidate_caps() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let scoped = tmp.path().join("scoped");
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&scoped).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        for i in 0..700 {
            std::fs::write(
                other.join(format!("targettoken_noise_{i:03}.rs")),
                format!(
                    "pub fn noisy_{i}() {{\n    // {}\n}}\n",
                    "targettoken ".repeat(80)
                ),
            )
            .unwrap();
        }
        std::fs::write(
            scoped.join("match.rs"),
            "pub fn scoped_match() -> &'static str { \"targettoken\" }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        for options in [
            SearchOptions {
                limit: Some(1),
                include_globs: vec!["scoped/**".to_string()],
                ..SearchOptions::default()
            },
            SearchOptions {
                limit: Some(1),
                include_globs: vec!["scoped/match.rs".to_string()],
                ..SearchOptions::default()
            },
            SearchOptions {
                limit: Some(1),
                exclude_globs: vec!["other/**".to_string()],
                ..SearchOptions::default()
            },
        ] {
            let literal_hits = literal_search(&workspace, "targettoken", &options).unwrap();
            assert_eq!(literal_hits.len(), 1);
            assert_eq!(literal_hits[0].file_path, PathBuf::from("scoped/match.rs"));

            let hybrid_hits = hybrid_search(&workspace, "targettoken", None, &options).unwrap();
            assert_eq!(hybrid_hits.len(), 1);
            assert_eq!(hybrid_hits[0].file_path, PathBuf::from("scoped/match.rs"));
        }
    }

    #[test]
    #[serial]
    fn literal_search_does_not_panic_when_file_truncated_after_indexing() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        // Index a file whose chunk spans many lines.
        let mut big = String::new();
        for i in 0..60 {
            big.push_str(&format!("fn line_{i}() {{ let needle_marker = {i}; }}\n"));
        }
        std::fs::write(tmp.path().join("big.rs"), &big).unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        // Truncate the file on disk WITHOUT reindexing, so the stored chunk
        // bounds (start_line ~1..60) exceed the live line count.
        std::fs::write(tmp.path().join("big.rs"), "fn only() {}\n").unwrap();

        // Must not panic on the out-of-range chunk bounds.
        let hits = literal_search(&workspace, "needle_marker", &SearchOptions::default()).unwrap();
        assert!(
            hits.iter().all(|h| !h.preview.contains("needle_marker")),
            "truncated file no longer contains the literal"
        );
    }

    #[test]
    #[serial]
    fn search_context_loads_vectors_only_when_needed() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("tax.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let lexical_context = SearchContext::load(&workspace, None, false).unwrap();
        assert!(lexical_context.hash_vectors.is_none());
        assert!(lexical_context.neural_vectors.is_none());

        let hash_context = SearchContext::load(&workspace, Some(256), false).unwrap();
        assert!(hash_context.hash_vectors.is_some());
        assert!(hash_context.neural_vectors.is_none());
    }

    #[test]
    #[serial]
    fn search_context_file_cache_tracks_live_file_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let path = tmp.path().join("cached.rs");
        std::fs::write(&path, "fn first() {}\n").unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let context = SearchContext::load(&workspace, None, false).unwrap();

        let first = context.read_file_content(&path).unwrap();
        assert_eq!(&*first.content, "fn first() {}\n");
        assert_eq!(first.lines.len(), 1);
        assert_eq!(line_at(&first.content, &first.lines, 1), "fn first() {}");

        std::fs::write(&path, "fn second_version() {}\n").unwrap();
        let second = context.read_file_content(&path).unwrap();
        assert_eq!(&*second.content, "fn second_version() {}\n");
        assert_eq!(
            line_at(&second.content, &second.lines, 1),
            "fn second_version() {}"
        );

        std::fs::remove_file(&path).unwrap();
        assert!(context.read_file_content(&path).is_none());
    }

    #[test]
    fn cached_line_spans_match_str_lines() {
        for content in [
            "",
            "one",
            "one\n",
            "one\ntwo\n",
            "one\r\ntwo\r\n",
            "\n",
            "\r\n",
            "one\n\ntwo\r\nthree\r",
            "first\nmultibyte: cafe\u{301}\nlast",
        ] {
            let spans = line_spans(content);
            let actual = spans
                .iter()
                .map(|span| &content[span.start..span.end])
                .collect::<Vec<_>>();
            assert_eq!(actual, content.lines().collect::<Vec<_>>(), "{content:?}");
        }
    }

    #[test]
    fn literal_pass_runs_only_for_exactish_queries() {
        assert!(should_run_literal_pass("calculate tax"));
        assert!(should_run_literal_pass("calculate_tax_for_region"));
        assert!(should_run_literal_pass("KernelMemoryAllocation"));
        assert!(!should_run_literal_pass("kernel memory allocation"));
        assert!(!should_run_literal_pass(
            "how are nested and sub-dependencies resolved"
        ));
        assert!(!should_run_literal_pass(
            "how IntoResponse converts handler return values"
        ));
        assert!(should_run_literal_pass(
            "Error: failed to open database connection after retrying the primary endpoint"
        ));
        assert!(should_run_literal_pass(
            "Traceback (most recent call last):\nconnection pool initialization failed"
        ));
    }

    #[test]
    fn structured_numeric_queries_use_one_conjunctive_lexical_pass() {
        assert!(should_use_conjunctive_numeric_query(
            "retry request after status 503"
        ));
        assert!(should_use_conjunctive_numeric_query(
            "find generated operation 498650"
        ));
        assert!(!should_use_conjunctive_numeric_query(
            "retry request after failure"
        ));
        assert!(!should_use_conjunctive_numeric_query("status 5"));
        assert!(!should_use_conjunctive_numeric_query(
            "for slot in range 16 if slot in 6 7 12"
        ));
        assert!(!should_use_conjunctive_numeric_query(
            "calculate payload checksum bits for binary value 1001001."
        ));
    }

    #[test]
    #[serial]
    fn literal_search_returns_empty_for_blank_query() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(tmp.path().join("lib.rs"), "pub fn something() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        assert!(
            literal_search(&workspace, "   ", &SearchOptions::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    #[serial]
    fn hybrid_search_handles_blank_query_gracefully() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(tmp.path().join("lib.rs"), "pub fn something() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let result = hybrid_search(&workspace, "", Some(&model), &SearchOptions::default());
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "empty query must return no results, not semantic noise"
        );
        for blank in ["   ", "\t\n", "  \t "] {
            let r = hybrid_search(&workspace, blank, Some(&model), &SearchOptions::default());
            assert!(
                r.is_ok() && r.unwrap().is_empty(),
                "whitespace query must be empty"
            );
        }
    }

    #[test]
    #[serial]
    fn hybrid_search_ignores_surrounding_query_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("extract.rs"),
            "pub trait FromRequest { fn from_request(); }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("handler.rs"),
            "fn handler<T: FromRequest>() {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let options = SearchOptions {
            limit: Some(10),
            ..Default::default()
        };

        let query = "where is FromRequest implemented";
        let plain = hybrid_search(&workspace, query, Some(&model), &options).unwrap();
        let padded = hybrid_search(
            &workspace,
            &format!(" \t{query}  \n"),
            Some(&model),
            &options,
        )
        .unwrap();
        let locations = |hits: &[SearchHit]| {
            hits.iter()
                .map(|hit| (hit.file_path.clone(), hit.start_line, hit.end_line))
                .collect::<Vec<_>>()
        };
        assert_eq!(locations(&plain), locations(&padded));
    }

    #[test]
    #[serial]
    fn hybrid_search_centers_qualified_prose_queries_on_the_member_definition() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("response.js"),
            r#"
res.sendFile = function sendFile(path, options, callback) {
  return sendfile(this, path, options, callback);
};

function sendfile(res, path, options, callback) {
  return streamFile(res, path, options, callback);
}
"#,
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let hits = hybrid_search(
            &workspace,
            "static file serving with res.sendFile",
            Some(&model),
            &SearchOptions {
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(hits[0].file_path, PathBuf::from("response.js"));
        assert!(
            hits[0].preview.contains("res.sendFile = function sendFile"),
            "qualified prose query should focus the public member, got: {}",
            hits[0].preview
        );
    }

    #[test]
    fn query_tokenization_filters_stopwords_before_singularizing() {
        let tokens = tokenize_query("where does packet processing enter the stack");
        assert!(!tokens.iter().any(|token| token == "doe"));
        assert_eq!(tokens, ["packet", "processing", "enter", "stack"]);
    }

    #[test]
    fn natural_language_literal_queries_use_canonical_phrase_aliases() {
        let query = "where does packet receive processing enter network stack";
        let literals = build_literal_queries(query, &build_lexical_queries(query));
        assert!(literals.iter().any(|literal| literal == "ingress"));
        assert!(!literals.iter().any(|literal| literal == "rx"));

        let query = "parsing uploaded binary file parts from HTTP requests";
        let literals = build_literal_queries(query, &build_lexical_queries(query));
        assert!(literals.iter().any(|literal| literal == "multipart"));

        let query = "how server-sent events are streamed to the client";
        let lexical = build_lexical_queries(query);
        assert!(lexical.iter().any(|variant| variant == "sse"));
        let literals = build_literal_queries(query, &lexical);
        assert!(literals.iter().any(|literal| literal == "sse"));
        assert!(
            literals
                .iter()
                .any(|literal| literal == "server-sent-events")
        );
        assert!(
            literals
                .iter()
                .any(|literal| literal == "server_sent_events")
        );
        assert!(
            literals
                .iter()
                .any(|literal| literal == "server sent events")
        );
    }

    #[test]
    fn mixed_long_alias_and_short_acronym_use_specificity_ranking() {
        assert!(literal_queries_need_specificity_ranking(&[
            "sse".to_string(),
            "server-sent-events".to_string(),
        ]));
        assert!(!literal_queries_need_specificity_ranking(&[
            "multipart".to_string(),
        ]));

        let matcher = LiteralMatcher::from_queries(["sse", "server-sent-events"], true).unwrap();
        assert!(
            literal_match_specificity(&matcher, "server-sent-events")
                > literal_match_specificity(&matcher, "SSE SSE")
        );
    }

    #[test]
    fn ascii_literal_matcher_is_case_insensitive_and_non_overlapping() {
        let matcher = LiteralMatcher::from_queries(["calculate_tax"], false).unwrap();
        assert!(matcher.is_match("fn CALCULATE_TAX() {}"));
        assert_eq!(matcher.match_count("calculate_tax CALCULATE_TAX"), 2);
        assert_eq!(matcher.match_count("aaaa"), 0);

        let matcher = LiteralMatcher::from_queries(["aa"], false).unwrap();
        assert_eq!(matcher.match_count("aaaa"), 2);
    }

    #[test]
    fn literal_matcher_uses_regex_for_multi_query_specificity() {
        let matcher = LiteralMatcher::from_queries(["sse", "server-sent-events"], true).unwrap();
        assert_eq!(literal_match_specificity(&matcher, "SSE SSE"), Some((3, 2)));
        assert_eq!(
            literal_match_specificity(&matcher, "server-sent-events"),
            Some((18, 1))
        );
    }

    #[test]
    #[serial]
    fn multipart_vocabulary_finds_the_canonical_implementation() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("multipart.rs"),
            "pub struct Multipart;\nimpl Multipart { pub fn next_field(&mut self) {} }\n",
        )
        .unwrap();
        for index in 0..12 {
            std::fs::write(
                tmp.path().join(format!("request_parts_{index}.rs")),
                format!("pub fn parse_http_request_parts_{index}() {{}}\n"),
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "parsing uploaded binary file parts from HTTP requests",
            Some(&model),
            &SearchOptions {
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            hits.iter()
                .any(|hit| hit.file_path == Path::new("multipart.rs")),
            "canonical multipart implementation should survive the top-10 candidate set"
        );
    }

    #[test]
    #[serial]
    fn query_expansion_matches_snake_case_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("errors.rs"),
            "pub fn handle_error(code: i32) -> String { format!(\"Error: {}\", code) }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("noise.rs"),
            "pub fn compute_value(x: f64) -> f64 { x * 2.0 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "handle error",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].preview.contains("handle_error"),
            "Expected handle_error as #1, got: {}",
            hits[0].preview.lines().next().unwrap_or("")
        );
    }

    #[test]
    #[serial]
    fn query_expansion_matches_camel_case_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("validator.java"),
            "class Validator {\n    void validateInput(String data) { }\n}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("noise.java"),
            "class Formatter {\n    void formatOutput() { }\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "validate input",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits.iter().any(|h| h.preview.contains("validateInput")),
            "Should find validateInput in results"
        );
    }

    #[test]
    fn query_expansion_adds_cli_aliases_for_command_line_flags() {
        let expanded = expanded_query_tokens("command line flags");
        assert!(expanded.iter().any(|token| token == "cli"));
        assert!(!expanded.iter().any(|token| token == "arg"));
        assert!(!expanded.iter().any(|token| token == "option"));

        let lexical = build_lexical_queries("command line flags");
        assert!(lexical.iter().any(|query| query == "cli"));

        let semantic = build_semantic_query_text("command line flags");
        assert!(!semantic.split_whitespace().any(|token| token == "cli"));
    }

    #[test]
    fn lexical_query_expansion_adds_code_word_roots() {
        let queries = build_lexical_queries("extractors resolved and streamed");
        assert_eq!(queries[0], "extractors resolved and streamed");
        let roots = queries
            .iter()
            .flat_map(|query| query.split_whitespace())
            .collect::<HashSet<_>>();
        assert!(roots.contains("extract"));
        assert!(roots.contains("resolve"));
        assert!(roots.contains("stream"));
    }

    #[test]
    fn neural_prose_lexical_search_condenses_expansion_passes() {
        let routing = QueryRouting::classify("calculate invoice tax region");
        assert!(routing.use_neural);

        let lexical = build_lexical_queries("calculate invoice tax region");
        let search_queries = lexical_search_queries_for_routing(&lexical, routing, false);

        assert_eq!(search_queries.len(), 2);
        assert_eq!(search_queries[0], "calculate invoice tax region");
        assert!(search_queries[1].contains("calculate_invoice_tax_region"));
        assert!(search_queries[1].contains("calculateInvoiceTaxRegion"));
    }

    #[test]
    fn exact_identifier_lexical_search_keeps_full_expansion_passes() {
        let routing = QueryRouting::classify("handle_error");
        let lexical = build_lexical_queries("handle_error");
        let search_queries = lexical_search_queries_for_routing(&lexical, routing, false);

        assert_eq!(search_queries, lexical);
    }

    #[test]
    fn simple_lexical_query_defers_path_and_parser_syntax() {
        let mut schema = tantivy::schema::Schema::builder();
        let fields = TantivyFields {
            vector_key: schema.add_u64_field("vector_key", tantivy::schema::STORED),
            file_path: schema.add_text_field("file_path", tantivy::schema::STRING),
            start_line: schema.add_u64_field("start_line", tantivy::schema::STORED),
            end_line: schema.add_u64_field("end_line", tantivy::schema::STORED),
            language: schema.add_text_field("language", tantivy::schema::STRING),
            kind: schema.add_text_field("kind", tantivy::schema::STRING),
            text: schema.add_text_field("text", tantivy::schema::TEXT),
            text_trigrams: None,
            is_ignored: None,
            file_path_text: Some(schema.add_text_field("file_path_text", tantivy::schema::TEXT)),
            signature: Some(schema.add_text_field("signature", tantivy::schema::TEXT)),
        };

        assert!(simple_lexical_query(&fields, "calculate invoice_tax 42", false).is_some());
        assert!(simple_lexical_query(&fields, "src/search.rs", false).is_none());
        assert!(simple_lexical_query(&fields, "\"exact phrase\"", false).is_none());
        assert!(simple_lexical_query(&fields, "alpha OR beta", false).is_none());
    }

    #[test]
    fn natural_language_symbol_queries_are_bounded_and_definition_shaped() {
        let visitor =
            natural_language_symbol_queries("Visitor pattern for deserializer implementations");
        assert_eq!(visitor.first().map(String::as_str), Some("Visitor"));
        assert!(visitor.len() <= 16);

        let parser = natural_language_symbol_queries("parse a JSON string into a value");
        assert!(parser.contains(&"Parser".to_string()));

        let router = natural_language_symbol_queries("routing HTTP requests to controllers");
        assert!(router.contains(&"Router".to_string()));

        let internals = natural_language_symbol_queries("reflection equals builder internals");
        assert!(internals.contains(&"reflectionEquals".to_string()));
        assert!(internals.contains(&"equalsBuilder".to_string()));

        assert!(
            natural_language_symbol_queries(
                "how serde_derive generates Serialize impl with serialize_field"
            )
            .iter()
            .all(|query| query != "Serialize")
        );
        assert!(
            natural_language_symbol_queries(
                "how the mapper resolves a deserializer for a given Java type"
            )
            .iter()
            .all(|query| query != "Java")
        );
        assert!(
            natural_language_symbol_queries("Deserialize and Deserializer trait definitions")
                .is_empty()
        );
        assert!(
            natural_language_symbol_queries(
                "deserialization context and error accumulation during parsing"
            )
            .iter()
            .all(|query| query != "Parser")
        );
        assert!(
            natural_language_symbol_queries("exception types for parse and type errors")
                .iter()
                .all(|query| query != "Parser")
        );

        assert!(natural_language_symbol_queries("Visitor").is_empty());
        assert!(natural_language_symbol_queries("Sinatra::Helpers").is_empty());
        assert!(
            natural_language_symbol_queries(
                "HTTP TLS JSON request handling implementation details"
            )
            .iter()
            .all(|query| !matches!(query.as_str(), "HTTP" | "TLS" | "JSON"))
        );
    }

    #[test]
    fn exact_symbol_queries_include_qualified_leaf_names() {
        assert_eq!(
            exact_symbol_query_names("Rack::Response"),
            ["Rack::Response", "Response"]
        );
        assert_eq!(exact_symbol_query_names("Visitor"), ["Visitor"]);
        assert_eq!(
            qualified_symbol_leaf_names(
                "how app.handle dispatches requests and res.sendFile() serves content"
            ),
            ["handle", "sendFile"]
        );
        assert!(
            exact_symbol_query_names("how app.handle dispatches requests")
                .contains(&"handle".to_string())
        );
        assert_eq!(
            qualified_symbol_leaf_names("client.sendRequest dispatch"),
            ["sendRequest"]
        );
        assert!(qualified_symbol_leaf_names("z.record and z.map schemas").is_empty());
        assert!(qualified_symbol_leaf_names("how mini.files opens buffers").is_empty());
        assert!(qualified_symbol_leaf_names("Plug.Session cookie store").is_empty());
        assert!(qualified_symbol_leaf_names("absl::Mutex locking").is_empty());
        assert!(qualified_symbol_leaf_names("read config.toml").is_empty());
    }

    #[test]
    fn lexical_candidate_budget_prioritizes_the_original_query() {
        let limits = lexical_query_candidate_limits(250, 6);
        assert_eq!(limits.iter().sum::<usize>(), 250);
        assert_eq!(limits[0], 187);
        assert!(limits[1..].iter().all(|limit| *limit >= 12));
        assert!(limits[1..].iter().all(|limit| *limit < limits[0]));
    }

    #[test]
    fn query_expansion_does_not_add_repo_specific_aliases() {
        let expanded = expanded_query_tokens("search results output format");
        assert!(!expanded.iter().any(|token| token == "printer"));
        assert!(
            !expanded_query_tokens("parallel directory walker")
                .iter()
                .any(|token| token == "walk")
        );

        let lexical = build_lexical_queries("search results output format");
        assert!(!lexical.iter().any(|query| query == "printer"));
    }

    #[test]
    #[serial]
    fn definition_site_ranks_above_usage() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("definition.rs"),
            "pub fn process_payment(amount: f64) -> bool {\n    amount > 0.0\n}\n",
        )
        .unwrap();
        // A usage site: the function name appears but this is a caller, not the definition
        std::fs::write(
            tmp.path().join("caller.rs"),
            "pub fn run_billing() {\n    let ok = process_payment(100.0);\n    println!(\"payment processed: {ok}\");\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "process payment",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].preview.contains("pub fn process_payment"),
            "Definition site should rank first, got: {}",
            hits[0].preview.lines().next().unwrap_or("")
        );
    }

    #[test]
    #[serial]
    fn file_path_boosts_relevant_results() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("auth.rs"),
            "pub fn login(user: &str) -> bool { true }\npub fn logout() {}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("utils.rs"),
            "// auth redirect helper\npub fn redirect(url: &str) {}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "auth login",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].file_path.to_string_lossy().contains("auth"),
            "auth.rs should rank first due to path boost, got: {}",
            hits[0].file_path.display()
        );
    }

    #[test]
    fn natural_language_path_recall_is_bounded_and_identifier_aware() {
        assert_eq!(
            natural_language_path_recall_query(
                "how code traverses the Zig AST to resolve symbol scopes"
            )
            .as_deref(),
            Some("traverse traverses ast resolve symbol scope scopes")
        );
        assert!(natural_language_path_recall_query("auth login").is_none());
    }

    #[test]
    #[serial]
    fn natural_language_path_recall_survives_content_heavy_lexical_competition() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/ast.zig"),
            "pub fn inspect(value: usize) void { _ = value; }\n",
        )
        .unwrap();
        for index in 0..120 {
            std::fs::write(
                tmp.path().join(format!("src/distractor_{index}.zig")),
                format!(
                    "pub fn distractor_{index}() void {{ // code traverses zig to resolve symbol scopes\n}}\n"
                ),
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "how code traverses the Zig AST to resolve symbol scopes",
            Some(&model),
            &SearchOptions {
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            hits.iter()
                .any(|hit| hit.file_path == Path::new("src/ast.zig")),
            "path-matched module should survive content-heavy lexical competition: {hits:#?}"
        );
        assert!(
            hits.iter()
                .find(|hit| hit.file_path == Path::new("src/ast.zig"))
                .is_some_and(|hit| hit.sources.iter().any(|source| source == "path")),
            "ast.zig should carry path evidence: {hits:#?}"
        );
    }

    #[test]
    #[serial]
    fn natural_language_path_recall_fills_the_distinct_file_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let distractor = (0..48)
            .map(|index| format!("def helper_{index}():\n    return {index}\n"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(tmp.path().join("src/model_fields_helpers.py"), distractor).unwrap();
        std::fs::write(
            tmp.path().join("src/fields.py"),
            "def construct_value():\n    return None\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "how are model fields declared and constrained",
            Some(&model),
            &SearchOptions {
                limit: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            hits.iter()
                .any(|hit| hit.file_path == Path::new("src/fields.py")),
            "a chunk-heavy matching file must not exhaust the path-file budget: {hits:#?}"
        );
        assert!(
            hits.iter()
                .find(|hit| hit.file_path == Path::new("src/fields.py"))
                .is_some_and(|hit| hit.sources.iter().any(|source| source == "path")),
            "fields.py should carry path evidence: {hits:#?}"
        );
    }

    #[test]
    #[serial]
    fn natural_language_query_prefers_chunking_source_file() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("benches")).unwrap();
        std::fs::write(
            tmp.path().join("src/chunking.rs"),
            "pub fn chunk_source(input: &str) -> usize {\n    input.lines().count()\n}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("src/text.rs"),
            "fn is_code_separator(ch: char) -> bool {\n    matches!(ch, '_' | '-')\n}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("benches/indexer_bench.rs"),
            "fn bench_chunking() {\n    assert_eq!(2, 1 + 1);\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "where is code chunking done",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(
            hits[0].file_path,
            std::path::PathBuf::from("src/chunking.rs")
        );
    }

    #[test]
    #[serial]
    fn implementation_query_prefers_source_over_tests() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tests")).unwrap();
        std::fs::write(
            tmp.path().join("src/mcp.rs"),
            "pub fn serve_stdio() {\n    println!(\"mcp server ready\");\n}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("tests/mcp_e2e.rs"),
            "#[test]\nfn e2e_mcp_initialize() {\n    assert!(true);\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "where is mcp implemented",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].file_path, std::path::PathBuf::from("src/mcp.rs"));
    }

    #[test]
    #[serial]
    fn semantic_only_results_penalized_below_lexical() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            tmp.path().join("exact.rs"),
            "pub fn calculate_discount(price: f64, rate: f64) -> f64 { price * rate }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("vague.rs"),
            "pub fn apply_reduction(value: f64) -> f64 { value * 0.9 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "calculate discount",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].preview.contains("calculate_discount"),
            "Exact lexical match should rank #1, got: {}",
            hits[0].preview.lines().next().unwrap_or("")
        );
    }

    #[test]
    #[serial]
    fn literal_search_finds_string_constants() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        // The term "gquota" appears ONLY inside a string literal and as part
        // of a constant name. Tantivy's tokenizer may or may not produce a
        // matching token — the SQLite fallback must catch it either way.
        std::fs::write(
            tmp.path().join("plugin.ts"),
            r#"import { Plugin } from "sdk";

const GEMINI_QUOTA_COMMAND = "gquota";

export function registerCommands(p: Plugin) {
    p.registerCommand(GEMINI_QUOTA_COMMAND, () => {
        console.log("checking quota...");
    });
}
"#,
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("README.md"),
            "# Plugin\n\nRun `/gquota` to check your quota.\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        // --literal mode must find both files
        let literal_hits = literal_search(&workspace, "gquota", &SearchOptions::default()).unwrap();

        let literal_files: HashSet<String> = literal_hits
            .iter()
            .map(|h| h.file_path.to_string_lossy().to_string())
            .collect();
        assert!(
            literal_files.contains("plugin.ts"),
            "literal search must find gquota in plugin.ts, got files: {:?}",
            literal_files
        );
        assert!(
            literal_files.contains("README.md"),
            "literal search must find gquota in README.md, got files: {:?}",
            literal_files
        );

        // hybrid mode must also surface plugin.ts
        let hybrid_hits = hybrid_search(
            &workspace,
            "gquota",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();
        let hybrid_files: HashSet<String> = hybrid_hits
            .iter()
            .map(|h| h.file_path.to_string_lossy().to_string())
            .collect();
        assert!(
            hybrid_files.contains("plugin.ts"),
            "hybrid search must find gquota in plugin.ts, got files: {:?}",
            hybrid_files
        );
    }

    #[test]
    #[serial]
    fn bm25f_signature_boost_ranks_definitions_first() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        // Definition site: the function signature should be indexed in the
        // `signature` field with 5× boost via code tokenizer.
        std::fs::write(
            tmp.path().join("handler.rs"),
            r#"pub fn handleError(code: i32) -> Result<(), Error> {
    log::error!("error code: {}", code);
    Err(Error::new(code))
}
"#,
        )
        .unwrap();
        // Usage site: mentions handleError but is not the definition
        std::fs::write(
            tmp.path().join("main.rs"),
            r#"fn main() {
    let result = handler::handleError(404);
    match result {
        Ok(()) => println!("ok"),
        Err(e) => println!("failed: {}", e),
    }
}
"#,
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "handle error",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();

        assert!(
            !hits.is_empty(),
            "BM25F should find results for 'handle error'"
        );

        // Both files should appear in results
        let files: Vec<String> = hits
            .iter()
            .map(|h| {
                h.file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert!(
            files.contains(&"handler.rs".to_string()),
            "definition file must appear in results, got: {:?}",
            files
        );

        // Definition should rank first thanks to signature field boost
        assert_eq!(
            files[0], "handler.rs",
            "definition site should rank #1 thanks to signature boost, got order: {:?}",
            files
        );
    }

    #[test]
    fn is_test_path_true_positives() {
        // Directory conventions
        assert!(is_test_path("tests/unit/handler.rs"));
        assert!(is_test_path("test/integration/db.go"));
        assert!(is_test_path("src/__tests__/Button.test.tsx"));
        assert!(is_test_path("spec/models/user_spec.rb"));
        assert!(is_test_path("src/__mocks__/api.ts"));

        // Filename conventions
        assert!(is_test_path("src/handler_test.go"));
        assert!(is_test_path("src/handler_test.rs"));
        assert!(is_test_path("src/Button.test.tsx"));
        assert!(is_test_path("src/user_spec.rb"));
        assert!(is_test_path("src/handler.spec.ts"));
        assert!(is_test_path("src/handler_mock.go"));
        assert!(is_test_path("src/handler.mock.ts"));
        assert!(is_test_path("test_handler.py"));
        assert!(is_test_path("lib/test_utils.py"));
    }

    #[test]
    fn is_test_path_false_positives_avoided() {
        // These contain "test" as a substring but are NOT test files
        assert!(!is_test_path("src/attestation.rs"));
        assert!(!is_test_path("src/contest.rs"));
        assert!(!is_test_path("src/fastest.go"));
        assert!(!is_test_path("src/detest.py"));
        assert!(!is_test_path("src/latest_handler.rs"));
        assert!(!is_test_path("src/protest.go"));

        // These contain "spec" as a substring but are NOT spec files
        assert!(!is_test_path("src/inspect.rs"));
        assert!(!is_test_path("src/specification.py"));
        assert!(!is_test_path("src/respect.go"));

        // Core source files
        assert!(!is_test_path("src/search.rs"));
        assert!(!is_test_path("src/handler.rs"));
        assert!(!is_test_path("lib/utils.py"));
    }

    // -----------------------------------------------------------------------
    // filter_meaningful_scores unit tests
    // -----------------------------------------------------------------------

    fn make_chunk(id: &str) -> IndexedChunk {
        crate::indexer::IndexedChunk {
            chunk_id: id.to_string(),
            file_path: PathBuf::from(format!("{id}.rs")),
            start_line: 1,
            end_line: 10,
            language: "rust".to_string(),
            kind: "function".to_string(),
            text: format!("fn {id}() {{}}"),
            content_hash: id.to_string(),
            vector_key: xxhash_rust::xxh3::xxh3_64(id.as_bytes()),
            is_ignored: false,
        }
    }

    fn make_chunk_with_path(id: &str, path: &str, text: &str) -> IndexedChunk {
        crate::indexer::IndexedChunk {
            chunk_id: id.to_string(),
            file_path: PathBuf::from(path),
            start_line: 1,
            end_line: 10,
            language: "rust".to_string(),
            kind: "function".to_string(),
            text: text.to_string(),
            content_hash: format!("hash-{id}"),
            vector_key: xxhash_rust::xxh3::xxh3_64(id.as_bytes()),
            is_ignored: false,
        }
    }

    #[test]
    #[serial]
    fn exact_symbol_signal_can_promote_the_canonical_definition() {
        unsafe { std::env::set_var("IVYGREP_RERANK_LIMIT", "1") };
        let usage = make_chunk_with_path(
            "usage",
            "src/wrapper.rs",
            "pub fn wrapper() { handle_error(); handle_error(); }",
        );
        let definition = make_chunk_with_path(
            "definition",
            "src/error.rs",
            "pub fn handle_error(code: i32) { log(code); }",
        );
        let mut reexport = make_chunk_with_path(
            "reexport",
            "src/lib.rs",
            "pub use crate::error::handle_error;",
        );
        reexport.kind = "Text".to_string();
        let without_symbols = fuse_rrf(
            FusionCandidates {
                lexical: vec![(usage.clone(), 1.0), (definition.clone(), 1.0)],
                semantic: vec![],
                literal: vec![],
                path: vec![],
                path_weight: 1.5,
                symbols: vec![],
            },
            1.0,
            "handle_error",
            Some(10),
        );
        let with_symbols = fuse_rrf(
            FusionCandidates {
                lexical: vec![(usage, 1.0), (definition.clone(), 1.0)],
                semantic: vec![],
                literal: vec![],
                path: vec![],
                path_weight: 1.5,
                symbols: vec![
                    (reexport, SymbolCandidateKind::Exact),
                    (definition, SymbolCandidateKind::Exact),
                ],
            },
            1.0,
            "handle_error",
            Some(10),
        );

        assert_eq!(without_symbols[0].0.chunk_id, "usage");
        assert_eq!(with_symbols[0].0.chunk_id, "definition");
        assert!(with_symbols[0].2.contains(&"exact-symbol".to_string()));

        let mut lowercase_function = make_chunk_with_path(
            "lowercase-function",
            "lib/ecto/query/planner.ex",
            "def query(meta, prepared, cache, params) do",
        );
        lowercase_function.language = "elixir".to_string();
        let mut exact_module = make_chunk_with_path(
            "exact-module",
            "lib/ecto/query.ex",
            "defmodule Ecto.Query do\n  defstruct [:from, :joins]\nend",
        );
        exact_module.language = "elixir".to_string();
        exact_module.kind = "Module".to_string();
        let mut nested_module = make_chunk_with_path(
            "nested-module",
            "lib/ecto/repo/query.ex",
            "defmodule Ecto.Repo.Query do\n  def load(meta), do: meta\nend",
        );
        nested_module.language = "elixir".to_string();
        nested_module.kind = "Module".to_string();
        let exact_case = fuse_rrf(
            FusionCandidates {
                lexical: vec![
                    (lowercase_function.clone(), 1.0),
                    (exact_module.clone(), 0.5),
                    (nested_module.clone(), 2.0),
                ],
                semantic: vec![],
                literal: vec![],
                path: vec![],
                path_weight: 1.5,
                symbols: vec![
                    (lowercase_function, SymbolCandidateKind::Exact),
                    (nested_module, SymbolCandidateKind::Exact),
                    (exact_module, SymbolCandidateKind::Exact),
                ],
            },
            1.0,
            "Query",
            Some(10),
        );
        unsafe { std::env::remove_var("IVYGREP_RERANK_LIMIT") };

        assert_eq!(exact_case[0].0.chunk_id, "exact-module");
        assert!(exact_case[0].2.contains(&"exact-symbol".to_string()));
    }

    #[test]
    fn inferred_symbol_survives_architecture_distractors() {
        let mut router = make_chunk_with_path(
            "router",
            "src/Routing/Router.php",
            "class Router { public function dispatch(Request $request) { return $this->dispatchToRoute($request); } }",
        );
        router.language = "php".to_string();
        router.kind = "Class".to_string();

        let lexical = (0..12)
            .map(|index| {
                (
                    make_chunk_with_path(
                        &format!("controller-{index}"),
                        &format!("src/Routing/Controller{index}.php"),
                        "class Controller { public function handle(Request $request) {} }",
                    ),
                    20.0 - index as f32,
                )
            })
            .collect();
        let ranked = fuse_rrf(
            FusionCandidates {
                lexical,
                semantic: vec![],
                literal: vec![],
                path: vec![],
                path_weight: 1.5,
                symbols: vec![(router, SymbolCandidateKind::Inferred)],
            },
            1.0,
            "routing HTTP requests to controllers",
            Some(20),
        );

        let router_rank = ranked.iter().position(|(chunk, _, sources)| {
            chunk.chunk_id == "router" && sources.contains(&"inferred-symbol".to_string())
        });
        assert!(
            router_rank.is_some_and(|rank| rank < 10),
            "inferred Router definition should survive lexical distractors: {ranked:#?}"
        );
    }

    #[test]
    fn deep_pool_backfill_survives_file_diversity() {
        let top = make_chunk_with_path(
            "top",
            "src/top.rs",
            "binary file detection handles binary files",
        );
        let low = make_chunk_with_path("low", "src/low.rs", "supporting implementation");
        let query = FusionQuery::new("binary file detection");
        let ranked = vec![
            (top, 2.0, vec!["lexical".to_string()]),
            (low, 0.1, vec!["lexical".to_string()]),
        ];

        let filtered = ranked_candidates_to_tuples(filter_meaningful_scores_with_query(
            ranked_candidates_from_tuples(ranked),
            &query,
            true,
        ));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[1].0.chunk_id, "low");
        assert!(filtered[1].2.contains(&"backfill".to_string()));
    }

    #[test]
    fn representative_span_promotion_preserves_file_rank() {
        let top = make_chunk_with_path("top", "src/shared.rs", "JSON serialization");
        let evidence = make_chunk_with_path(
            "evidence",
            "src/shared.rs",
            "JSON response JSONP callback support",
        );
        let other = make_chunk_with_path("other", "src/other.rs", "JSON response");
        let ranked = vec![
            (top, 5.0, vec!["lexical".to_string()]),
            (evidence, 4.0, vec!["path".to_string()]),
            (other, 3.0, vec!["lexical".to_string()]),
        ];
        let query_tokens = tokenize_query("JSON response serialization and JSONP callback support");

        let mut ranked = ranked_candidates_from_tuples(ranked);
        promote_representative_span(&mut ranked, &query_tokens, 0.67);
        let ranked = ranked_candidates_to_tuples(ranked);

        assert_eq!(ranked[0].0.chunk_id, "evidence");
        assert_eq!(ranked[0].1, 5.0);
        assert_eq!(ranked[0].2, vec!["lexical".to_string()]);
        assert_eq!(ranked[1].0.chunk_id, "top");
        assert_eq!(ranked[1].1, 4.0);
        assert_eq!(ranked[1].2, vec!["path".to_string()]);
        assert_eq!(ranked[2].0.chunk_id, "other");
        assert_eq!(ranked[2].1, 3.0);
    }

    #[test]
    fn qualified_symbol_span_promotion_prefers_the_exact_cased_member() {
        let mut helper = make_chunk_with_path(
            "helper",
            "lib/response.js",
            "function sendfile(res, file, options, callback) {}",
        );
        helper.language = "javascript".to_string();
        helper.kind = "Function".to_string();
        let mut member = make_chunk_with_path(
            "member",
            "lib/response.js",
            "res.sendFile = function sendFile(path, options, callback) {}",
        );
        member.language = "javascript".to_string();
        member.kind = "Module".to_string();
        let other = make_chunk_with_path("other", "lib/static.js", "function serveStatic(path) {}");
        let ranked = vec![
            (
                helper,
                5.0,
                vec!["exact-symbol".to_string(), "lexical".to_string()],
            ),
            (member, 4.0, vec!["exact-symbol".to_string()]),
            (other, 3.0, vec!["lexical".to_string()]),
        ];

        let mut ranked = ranked_candidates_from_tuples(ranked);
        promote_qualified_symbol_span(&mut ranked, "static file serving with res.sendFile()");
        let ranked = ranked_candidates_to_tuples(ranked);

        assert_eq!(ranked[0].0.chunk_id, "member");
        assert_eq!(ranked[0].1, 5.0);
        assert_eq!(
            ranked[0].2,
            vec!["exact-symbol".to_string(), "lexical".to_string()]
        );
        assert_eq!(ranked[1].0.chunk_id, "helper");
        assert_eq!(ranked[1].1, 4.0);
        assert_eq!(ranked[1].2, vec!["exact-symbol".to_string()]);
        assert_eq!(ranked[2].0.chunk_id, "other");
    }

    #[test]
    fn local_term_coverage_is_ascii_case_insensitive() {
        let tokens = vec!["json".to_string(), "callback".to_string()];
        assert_eq!(local_term_coverage(&tokens, "JSON Callback"), 1.0);
    }

    #[test]
    fn file_coherence_promotes_the_best_chunk_from_supported_files() {
        let primary = make_chunk_with_path("primary", "src/primary.rs", "primary");
        let support = make_chunk_with_path("support", "src/primary.rs", "support");
        let alternate = make_chunk_with_path("alternate", "src/alternate.rs", "alternate");
        let ranked = vec![
            (alternate, 1.0, vec![]),
            (primary, 0.95, vec![]),
            (support, 0.90, vec![]),
        ];

        let mut ranked = ranked_candidates_from_tuples(ranked);
        apply_file_coherence_boost(&mut ranked, 0.2, false);
        let ranked = ranked_candidates_to_tuples(ranked);

        assert_eq!(ranked[0].1, 1.0 + 0.2 / 1.85);
        assert_eq!(ranked[1].1, 0.95 + 0.2);
        assert_eq!(ranked[2].1, 0.90);
    }

    #[test]
    fn file_coherence_respects_source_authority() {
        let primary = make_chunk_with_path("primary", "src/primary_test.rs", "primary");
        let support = make_chunk_with_path("support", "src/primary_test.rs", "support");
        let alternate = make_chunk_with_path("alternate", "src/alternate.rs", "alternate");
        let ranked = vec![
            (alternate, 1.0, vec![]),
            (primary, 0.95, vec![]),
            (support, 0.90, vec![]),
        ];

        let mut ranked = ranked_candidates_from_tuples(ranked);
        apply_file_coherence_boost(&mut ranked, 0.2, false);
        let ranked = ranked_candidates_to_tuples(ranked);

        assert_eq!(ranked[0].1, 1.0 + 0.2 / 1.85);
        assert_eq!(ranked[1].1, 0.95 + 0.2 * 0.6);
        assert_eq!(ranked[2].1, 0.90);
    }

    #[test]
    #[serial]
    fn reranker_candidate_limit_is_configurable_and_bounded() {
        unsafe { std::env::set_var("IVYGREP_RERANK_LIMIT", "7") };
        assert_eq!(rerank_candidate_limit(), 7);
        assert_eq!(
            rerank_candidate_limit_for_routing(QueryRouting::classify(
                "how to route learned query"
            )),
            7
        );
        unsafe { std::env::set_var("IVYGREP_RERANK_LIMIT", "0") };
        assert_eq!(rerank_candidate_limit(), 100);
        assert_eq!(
            rerank_candidate_limit_for_routing(QueryRouting::classify("exact_symbol")),
            30
        );
        assert_eq!(
            rerank_candidate_limit_for_routing(QueryRouting::classify(
                "how to route learned query"
            )),
            100
        );
        unsafe { std::env::set_var("IVYGREP_RERANKER", "deterministic") };
        assert_eq!(rerank_candidate_limit(), 30);
        assert_eq!(
            rerank_candidate_limit_for_routing(QueryRouting::classify(
                "how to route learned query"
            )),
            30
        );
        unsafe { std::env::remove_var("IVYGREP_RERANKER") };
        unsafe { std::env::remove_var("IVYGREP_RERANK_LIMIT") };
    }

    fn make_ranked(entries: &[(&str, f32, &[&str])]) -> Vec<(IndexedChunk, f32, Vec<String>)> {
        entries
            .iter()
            .map(|(id, score, sources)| {
                (
                    make_chunk(id),
                    *score,
                    sources.iter().map(|s| s.to_string()).collect(),
                )
            })
            .collect()
    }

    fn ranked_candidates_from_tuples(
        ranked: Vec<(IndexedChunk, f32, Vec<String>)>,
    ) -> Vec<RankedCandidate> {
        ranked
            .into_iter()
            .map(|(chunk, score, sources)| RankedCandidate {
                chunk,
                score,
                sources: source_mask(&sources),
            })
            .collect()
    }

    fn ranked_candidates_to_tuples(
        ranked: Vec<RankedCandidate>,
    ) -> Vec<(IndexedChunk, f32, Vec<String>)> {
        ranked
            .into_iter()
            .map(RankedCandidate::into_tuple)
            .collect()
    }

    fn make_ranked_with_chunks(
        entries: &[(IndexedChunk, f32, &[&str])],
    ) -> Vec<(IndexedChunk, f32, Vec<String>)> {
        entries
            .iter()
            .map(|(chunk, score, sources)| {
                (
                    chunk.clone(),
                    *score,
                    sources.iter().map(|s| s.to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn filter_single_result_returns_it() {
        let ranked = make_ranked(&[("a", 0.5, &["lexical"])]);
        let filtered = filter_meaningful_scores(ranked, "a");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_empty_input_returns_empty() {
        let ranked: Vec<(IndexedChunk, f32, Vec<String>)> = vec![];
        let filtered = filter_meaningful_scores(ranked, "anything");
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_keeps_single_high_confidence_semantic_result() {
        let chunk = make_chunk_with_path(
            "binary_detector",
            "src/search/binary_detector.rs",
            "pub fn detect_binary_content(bytes: &[u8]) -> bool { bytes.contains(&0) }",
        );
        let ranked = make_ranked_with_chunks(&[(chunk, 0.06, &["semantic"])]);
        let filtered = filter_meaningful_scores(ranked, "binary file detection");
        assert_eq!(
            filtered.len(),
            1,
            "strong single semantic hit should survive"
        );
        assert_eq!(filtered[0].0.chunk_id, "binary_detector");
    }

    #[test]
    fn filter_drops_doc_literal_without_doc_intent() {
        let doc_chunk = make_chunk_with_path(
            "search_docs",
            "docs/search.md",
            "Search guide: where is tax calculated?",
        );
        let ranked = make_ranked_with_chunks(&[(doc_chunk, 0.9, &["literal", "lexical"])]);
        let filtered = filter_meaningful_scores(ranked, "where is tax calculated");
        assert!(
            filtered.is_empty(),
            "docs example should not become high-confidence recommendation"
        );
    }

    #[test]
    fn filter_keeps_doc_literal_when_query_targets_docs() {
        let doc_chunk = make_chunk_with_path(
            "search_docs",
            "docs/search.md",
            "Search guide: ranking thresholds and examples",
        );
        let ranked = make_ranked_with_chunks(&[(doc_chunk, 0.9, &["literal", "lexical"])]);
        let filtered = filter_meaningful_scores(ranked, "search guide docs");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0.chunk_id, "search_docs");
    }

    #[test]
    fn filter_uniform_scores_keeps_all() {
        // All scores are identical — stddev is 0, so threshold = max(mean, 0.35*best, 0.01)
        // All entries equal that threshold so all should pass
        let ranked = make_ranked(&[
            ("a", 0.5, &["lexical"]),
            ("b", 0.5, &["lexical"]),
            ("c", 0.5, &["lexical"]),
            ("d", 0.5, &["lexical"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "a");
        assert_eq!(filtered.len(), 4, "all uniform scores should be kept");
    }

    #[test]
    fn filter_drops_low_outliers() {
        // One strong hit, several very weak ones — the weak ones should be filtered
        let ranked = make_ranked(&[
            ("strong", 1.0, &["lexical", "semantic"]),
            ("ok", 0.5, &["lexical"]),
            ("weak1", 0.02, &["semantic"]),
            ("weak2", 0.01, &["semantic"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "strong");
        // The threshold should be high enough to drop weak1 and weak2
        // (mean - stddev with a 0.35*best clamp of 0.35 means entries below 0.35 are cut)
        assert!(
            filtered.len() <= 3,
            "very low scores should be filtered out, got {}",
            filtered.len()
        );
        assert_eq!(filtered[0].0.chunk_id, "strong");
    }

    #[test]
    fn filter_backfills_authoritative_candidates_to_top_ten() {
        let ranked = make_ranked(&[
            ("a", 1.0, &["lexical"]),
            ("b", 0.09, &["semantic"]),
            ("c", 0.08, &["semantic"]),
            ("d", 0.07, &["semantic"]),
            ("e", 0.06, &["semantic"]),
            ("f", 0.05, &["semantic"]),
            ("g", 0.04, &["semantic"]),
            ("h", 0.03, &["semantic"]),
            ("i", 0.02, &["semantic"]),
            ("j", 0.01, &["semantic"]),
            ("k", 0.009, &["semantic"]),
            ("l", 0.008, &["semantic"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "search ranking");
        assert_eq!(filtered.len(), 10);
        assert_eq!(filtered[0].0.chunk_id, "a");
    }

    #[test]
    fn backfill_requires_ten_distinct_candidate_files() {
        let ranked = (0..12)
            .map(|index| {
                (
                    make_chunk_with_path(
                        &format!("chunk-{index}"),
                        &format!("src/file-{}.rs", index % 5),
                        "pub fn candidate() {}",
                    ),
                    1.0 - index as f32 * 0.01,
                    vec!["semantic".to_string()],
                )
            })
            .collect::<Vec<_>>();

        let ranked = ranked_candidates_from_tuples(ranked);
        assert!(!backfill_enabled(&ranked));
    }

    #[test]
    fn filter_keeps_literal_sources_even_below_threshold() {
        // Even a below-threshold result should be kept if it has a "literal" source
        let ranked = make_ranked(&[
            ("strong", 1.0, &["lexical"]),
            ("literal_hit", 0.001, &["literal"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "literal_hit");
        assert_eq!(filtered.len(), 2, "literal source should bypass threshold");
        let has_literal = filtered.iter().any(|(c, _, _)| c.chunk_id == "literal_hit");
        assert!(has_literal, "literal_hit must be preserved");
    }

    #[test]
    fn filter_drops_low_confidence_semantic_only_results() {
        let ranked = make_ranked(&[
            ("barely", 0.001, &["semantic"]),
            ("worse", 0.0001, &["semantic"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "unrelated natural language");
        assert!(
            filtered.is_empty(),
            "low-confidence semantic-only results should be suppressed"
        );
    }

    #[test]
    fn filter_keeps_decisive_semantic_only_result() {
        let strong = make_chunk_with_path(
            "strong",
            "src/search/binary_detector.rs",
            "pub fn detect_binary_content(bytes: &[u8]) -> bool { bytes.contains(&0) }",
        );
        let weak = make_chunk_with_path(
            "weak",
            "src/search/notes.rs",
            "pub fn unrelated_feature() {}",
        );
        let ranked =
            make_ranked_with_chunks(&[(strong, 0.12, &["semantic"]), (weak, 0.02, &["semantic"])]);
        let filtered = filter_meaningful_scores(ranked, "binary file detection");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0.chunk_id, "strong");
    }

    #[test]
    fn filter_keeps_strong_semantic_only_result_cluster_for_discovery() {
        let detect = make_chunk_with_path(
            "detect",
            "src/binary_file_detection/detect.rs",
            "pub fn detect_binary_file(bytes: &[u8]) -> bool { bytes.contains(&0) }",
        );
        let classify = make_chunk_with_path(
            "classify",
            "src/binary_file_detection/classify.rs",
            "pub fn classify_binary_file_detection(bytes: &[u8]) -> FileKind { inspect(bytes) }",
        );
        let inspect = make_chunk_with_path(
            "inspect",
            "src/binary_file_detection/inspect.rs",
            "pub fn inspect_binary_file_detection_header(bytes: &[u8]) -> Header { parse(bytes) }",
        );
        let unrelated = make_chunk_with_path(
            "unrelated",
            "src/runtime/metrics.rs",
            "pub fn record_process_metrics() {}",
        );
        let ranked = make_ranked_with_chunks(&[
            (detect, 0.12, &["semantic"]),
            (classify, 0.115, &["semantic"]),
            (inspect, 0.11, &["semantic"]),
            (unrelated, 0.09, &["semantic"]),
        ]);

        let filtered = filter_meaningful_scores(ranked, "binary file detection");
        assert_eq!(
            filtered
                .iter()
                .map(|item| item.0.chunk_id.as_str())
                .collect::<Vec<_>>(),
            ["detect", "classify", "inspect"]
        );
    }

    #[test]
    fn filter_tight_cluster_keeps_all() {
        // Scores in a tight cluster should all survive (small stddev, mean ≈ values)
        // 0.35*best(0.50) = 0.175, so all values are well above the clamp floor.
        // mean=0.49, stddev≈0.007, threshold=max(0.483, 0.175, 0.01)=0.483
        // All four values ≥ 0.483 ✓
        let ranked = make_ranked(&[
            ("a", 0.50, &["lexical"]),
            ("b", 0.50, &["lexical"]),
            ("c", 0.49, &["semantic"]),
            ("d", 0.49, &["lexical"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "a");
        assert_eq!(
            filtered.len(),
            4,
            "tight cluster should keep all results, got {}",
            filtered.len()
        );
    }

    #[test]
    fn filter_wide_spread_keeps_top_drops_bottom() {
        // Wide spread: top is very high, bottom is very low
        let ranked = make_ranked(&[
            ("top", 2.0, &["lexical", "semantic"]),
            ("mid", 1.0, &["lexical"]),
            ("low", 0.3, &["semantic"]),
            ("noise", 0.05, &["semantic"]),
        ]);
        let filtered = filter_meaningful_scores(ranked, "top");
        // With best=2.0, the 0.35*best clamp = 0.70, so "noise" (0.05) and "low" (0.3) drop
        assert!(
            filtered.len() >= 2,
            "should keep at least top and mid, got {}",
            filtered.len()
        );
        assert!(
            filtered.len() <= 3,
            "should drop the noise, got {}",
            filtered.len()
        );
        assert_eq!(filtered[0].0.chunk_id, "top");
    }

    // ── ChunkBoostContext tests ──────────────────────────────────────────

    fn make_test_chunk(
        id: &str,
        path: &str,
        text: &str,
        kind: &str,
    ) -> crate::indexer::IndexedChunk {
        crate::indexer::IndexedChunk {
            chunk_id: id.to_string(),
            file_path: PathBuf::from(path),
            start_line: 1,
            end_line: 10,
            language: "rust".to_string(),
            kind: kind.to_string(),
            text: text.to_string(),
            content_hash: format!("hash-{id}"),
            vector_key: 42,
            is_ignored: false,
        }
    }

    #[test]
    fn boost_context_precomputes_text_lower() {
        let chunk = make_test_chunk("a", "src/Foo.rs", "pub fn CalcTax() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.text_lower, "pub fn calctax() {}");
    }

    #[test]
    fn boost_context_precomputes_path_lower() {
        let chunk = make_test_chunk("a", "SRC/MyService/Handler.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.path_lower, "src/myservice/handler.rs");
    }

    #[test]
    fn boost_context_matches_path_segments_without_storing_copies() {
        let chunk = make_test_chunk("a", "src/my-service/handler.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(path_exact_match_boost("my-service", &bctx), 1.0);
    }

    #[test]
    fn boost_context_extracts_file_stem() {
        let chunk = make_test_chunk("a", "pkg/search.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.file_stem.as_deref(), Some("search"));
    }

    #[test]
    fn boost_context_extracts_first_meaningful_line() {
        let chunk = make_test_chunk(
            "a",
            "src/lib.rs",
            "// copyright header\n# attribute\npub fn handle_error() {}",
            "Function",
        );
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.first_line(), "pub fn handle_error() {}");
    }

    #[test]
    fn boost_context_computes_compact_identifiers() {
        let chunk = make_test_chunk("a", "src/my-service.rs", "fn foo_bar() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.text_compact.as_deref(), Some("fnfoobar"));
        assert_eq!(bctx.path_compact.as_deref(), Some("srcmyservicers"));
    }

    #[test]
    fn boost_context_handles_empty_text() {
        let chunk = make_test_chunk("a", "src/empty.rs", "", "Block");
        let bctx = ChunkBoostContext::new(&chunk);
        assert!(bctx.text_lower.is_empty());
        assert!(bctx.first_line().is_empty());
        assert_eq!(bctx.text_compact.as_deref(), Some(""));
    }

    #[test]
    fn term_coverage_boost_uses_precomputed_text() {
        let chunk = make_test_chunk("a", "src/lib.rs", "fn calculate_TAX() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["calculate".to_string(), "tax".to_string()];
        let coverage = term_coverage_boost(&tokens, &bctx);
        assert!(
            (coverage - 1.0).abs() < f32::EPSILON,
            "both tokens should match case-insensitively, got {coverage}"
        );
    }

    #[test]
    fn term_coverage_boost_partial_match() {
        let chunk = make_test_chunk("a", "src/lib.rs", "fn calculate() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["calculate".to_string(), "tax".to_string()];
        let coverage = term_coverage_boost(&tokens, &bctx);
        assert!(
            (coverage - 0.5).abs() < f32::EPSILON,
            "one of two tokens matched, got {coverage}"
        );
    }

    #[test]
    fn path_segment_boost_matches_directory() {
        let chunk = make_test_chunk("a", "src/tax/calculator.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["tax".to_string()];
        let boost = path_segment_boost(&tokens, &bctx);
        assert!(
            (boost - 1.0).abs() < f32::EPSILON,
            "token 'tax' matches path segment 'tax', got {boost}"
        );
    }

    #[test]
    fn path_exact_match_boost_full_segment() {
        let chunk = make_test_chunk("a", "services/my-service/handler.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let boost = path_exact_match_boost("my service", &bctx);
        assert!(
            (boost - 1.0).abs() < f32::EPSILON,
            "query 'my service' → 'my-service' should match path segment exactly, got {boost}"
        );
    }

    #[test]
    fn path_exact_match_boost_no_match() {
        let chunk = make_test_chunk("a", "src/unrelated.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let boost = path_exact_match_boost("my service", &bctx);
        assert!(boost < f32::EPSILON, "no path match expected, got {boost}");
    }

    #[test]
    fn file_stem_boost_exact_match() {
        let chunk = make_test_chunk("a", "src/search.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["search".to_string()];
        let boost = file_stem_boost(&tokens, &bctx);
        assert!(
            (boost - 1.0).abs() < f32::EPSILON,
            "exact stem match should return 1.0, got {boost}"
        );
    }

    #[test]
    fn file_stem_boost_partial_match() {
        let chunk = make_test_chunk("a", "src/search_engine.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["search".to_string()];
        let boost = file_stem_boost(&tokens, &bctx);
        assert!(
            (boost - 0.5).abs() < f32::EPSILON,
            "partial stem match should return 0.5, got {boost}"
        );
        let (_, derivational) = file_stem_signals(&tokens, &tokens, &tokens, &tokens, &bctx);
        assert!(
            !derivational,
            "exact stem terms are not derivational matches"
        );

        let walker = make_test_chunk("b", "src/walker.rs", "code", "Module");
        let walk = vec!["walk".to_string()];
        let (_, derivational) =
            file_stem_signals(&walk, &walk, &walk, &walk, &ChunkBoostContext::new(&walker));
        assert!(derivational, "walk should derivationally match walker");
    }

    #[test]
    fn alias_file_stem_multiplier_promotes_exact_alias_stems() {
        let root = make_test_chunk("root", "src/vector_store.rs", "code", "Module");
        let root_ctx = ChunkBoostContext::new(&root);
        assert_eq!(
            alias_file_stem_multiplier(&["vectorstore".to_string()], &root_ctx),
            2.7
        );

        let child = make_test_chunk("child", "src/vector_store/optimized.rs", "code", "Module");
        let child_ctx = ChunkBoostContext::new(&child);
        assert_eq!(
            alias_file_stem_multiplier(&["vectorstore".to_string()], &child_ctx),
            1.0
        );

        let ipc = make_test_chunk("ipc", "src/ipc.rs", "code", "Module");
        let ipc_ctx = ChunkBoostContext::new(&ipc);
        assert_eq!(
            alias_file_stem_multiplier(&["ipc".to_string()], &ipc_ctx),
            2.0
        );

        let daemon = make_test_chunk("daemon", "src/daemon.rs", "code", "Module");
        let daemon_ctx = ChunkBoostContext::new(&daemon);
        assert_eq!(
            alias_file_stem_multiplier(&["daemon".to_string()], &daemon_ctx),
            1.0
        );
    }

    #[test]
    fn primary_file_stem_multiplier_promotes_natural_language_subjects() {
        let walker = make_test_chunk("walker", "src/walker.rs", "code", "Module");
        assert_eq!(
            primary_file_stem_multiplier(
                &[
                    "walk".to_string(),
                    "directory".to_string(),
                    "tree".to_string(),
                ],
                true,
                &ChunkBoostContext::new(&walker),
            ),
            1.25
        );

        let jobs = make_test_chunk("jobs", "src/jobs.rs", "code", "Module");
        assert_eq!(
            primary_file_stem_multiplier(
                &[
                    "track".to_string(),
                    "progress".to_string(),
                    "job".to_string()
                ],
                true,
                &ChunkBoostContext::new(&jobs),
            ),
            1.25
        );
    }

    #[test]
    fn primary_file_stem_multiplier_stays_bounded_to_prose_and_primary_source() {
        let source = make_test_chunk("search", "src/search.rs", "code", "Module");
        assert_eq!(
            primary_file_stem_multiplier(
                &["find".to_string(), "code".to_string(), "search".to_string(),],
                false,
                &ChunkBoostContext::new(&source),
            ),
            1.0
        );

        let fixture = make_test_chunk("walker-test", "tests/walker.rs", "code", "Module");
        assert_eq!(
            primary_file_stem_multiplier(
                &[
                    "walk".to_string(),
                    "directory".to_string(),
                    "tree".to_string(),
                ],
                true,
                &ChunkBoostContext::new(&fixture),
            ),
            1.0
        );
    }

    #[test]
    fn path_boost_matches_conservative_derivational_roots() {
        let validate = make_test_chunk("a", "src/Validate.java", "code", "Class");
        let reflective = make_test_chunk(
            "b",
            "internal/ReflectiveTypeAdapterFactory.java",
            "code",
            "Class",
        );

        assert!(
            path_segment_boost(
                &["validation".to_string()],
                &ChunkBoostContext::new(&validate)
            ) > 0.0
        );
        assert!(
            file_stem_boost(
                &["reflection".to_string()],
                &ChunkBoostContext::new(&reflective)
            ) > 0.0
        );
        assert!(!code_term_matches("request", "response"));
    }

    #[test]
    fn definition_name_boost_matches_function_signature() {
        let chunk = make_test_chunk(
            "a",
            "src/lib.rs",
            "pub fn handle_error(err: Error) -> Result<()> {}\n    // body",
            "Function",
        );
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["handle".to_string(), "error".to_string()];
        let boost = definition_name_boost(&tokens, &bctx);
        assert!(
            (boost - 1.0).abs() < f32::EPSILON,
            "both tokens should match the definition name, got {boost}"
        );
    }

    #[test]
    fn definition_name_boost_skips_comments_and_attributes() {
        let chunk = make_test_chunk(
            "a",
            "src/lib.rs",
            "// handle error here\n#[derive(Debug)]\npub fn unrelated() {}",
            "Function",
        );
        let bctx = ChunkBoostContext::new(&chunk);
        let tokens = vec!["handle".to_string(), "error".to_string()];
        let boost = definition_name_boost(&tokens, &bctx);
        assert!(
            boost < f32::EPSILON,
            "definition name is 'pub fn unrelated()', should not match, got {boost}"
        );
    }

    #[test]
    fn parser_derived_documentation_gets_bounded_authority_boost() {
        let documentation = make_test_chunk(
            "docs",
            "src/middleware/mod.rs",
            "axum integrates Tower middleware with Router layers",
            "Documentation",
        );
        let function = make_test_chunk("function", "src/lib.rs", "fn router() {}", "Function");

        assert_eq!(chunk_kind_boost(&documentation), 1.2);
        assert!(
            chunk_kind_boost(&documentation) < chunk_kind_boost(&function),
            "module documentation should remain below definition sites"
        );
    }

    #[test]
    fn literal_match_boost_case_insensitive_text() {
        let chunk = make_test_chunk("a", "src/lib.rs", "fn CalcTax() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let boost = literal_match_boost("calctax", &bctx);
        assert!(
            boost > 0.0,
            "case-insensitive match should boost, got {boost}"
        );
    }

    #[test]
    fn literal_match_boost_compact_identifier_fallback() {
        let chunk = make_test_chunk("a", "src/lib.rs", "fn calc_tax() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        let boost = literal_match_boost("CalcTax", &bctx);
        assert!(
            boost > 0.0,
            "compact identifier should match calc_tax ↔ CalcTax, got {boost}"
        );
    }

    #[test]
    fn location_intent_boost_prefers_src_definitions() {
        let src_chunk = make_test_chunk("a", "src/handler.rs", "code", "Function");
        let test_chunk = make_test_chunk("b", "tests/handler_test.rs", "code", "Function");
        let src_bctx = ChunkBoostContext::new(&src_chunk);
        let test_bctx = ChunkBoostContext::new(&test_chunk);
        let src_boost = location_intent_boost(&src_chunk, &src_bctx);
        let test_boost = location_intent_boost(&test_chunk, &test_bctx);
        assert!(
            src_boost > test_boost,
            "src/ definition should rank above tests/ definition: {src_boost} vs {test_boost}"
        );
    }

    #[test]
    fn file_authority_score_penalizes_vendor() {
        let vendor_chunk = make_test_chunk("a", "vendor/dep/lib.rs", "code", "Function");
        let src_chunk = make_test_chunk("b", "src/handler.rs", "code", "Function");
        let vendor_bctx = ChunkBoostContext::new(&vendor_chunk);
        let src_bctx = ChunkBoostContext::new(&src_chunk);
        assert!(
            file_authority_score(&src_bctx) > file_authority_score(&vendor_bctx),
            "src should rank above vendor"
        );
    }

    #[test]
    fn file_authority_score_penalizes_test_files() {
        let test_chunk = make_test_chunk("a", "tests/integration_test.rs", "code", "Function");
        let src_chunk = make_test_chunk("b", "src/core.rs", "code", "Function");
        let test_bctx = ChunkBoostContext::new(&test_chunk);
        let src_bctx = ChunkBoostContext::new(&src_chunk);
        assert!(
            file_authority_score(&src_bctx) > file_authority_score(&test_bctx),
            "src should rank above tests"
        );
    }

    #[test]
    fn path_role_demotes_generic_support_paths() {
        assert_eq!(path_role("tools/debug_probe.rs"), PathRole::Support);
        assert_eq!(path_role("scripts/reindex.rs"), PathRole::Support);
        assert_eq!(path_role("examples/search_demo.rs"), PathRole::Support);
        assert_eq!(path_role("src/search.rs"), PathRole::PrimarySource);
    }

    #[test]
    fn selftests_count_as_tests_without_false_positive_substrings() {
        assert!(is_test_path(
            "tools/testing/selftests/bpf/prog_tests/verifier.c"
        ));
        assert!(!is_test_path("src/attestation.rs"));
    }

    #[test]
    fn effective_authority_penalizes_deep_unsupported_paths() {
        let shallow = make_test_chunk("a", "src/scheduler.rs", "code", "Function");
        let deep = make_test_chunk(
            "b",
            "plugins/vendor/wrappers/generated/scheduler.rs",
            "code",
            "Function",
        );
        let tokens = expanded_query_tokens("background job scheduler");
        let shallow_score = effective_authority_score(
            "background job scheduler",
            &tokens,
            &ChunkBoostContext::new(&shallow),
        );
        let deep_score = effective_authority_score(
            "background job scheduler",
            &tokens,
            &ChunkBoostContext::new(&deep),
        );

        assert!(shallow_score > deep_score);
    }

    #[test]
    fn deep_path_single_token_overlap_still_gets_small_penalty() {
        let deep = make_test_chunk(
            "a",
            "plugins/gpu/wrappers/examples/nested/scheduler.rs",
            "code",
            "Function",
        );
        let tokens = expanded_query_tokens("background job scheduler");
        let bctx = ChunkBoostContext::new(&deep);

        assert_eq!(path_query_overlap(&tokens, &bctx), 1);
        assert!(
            effective_authority_score("background job scheduler", &tokens, &bctx)
                < file_authority_score(&bctx)
        );
    }

    #[test]
    fn chunk_density_penalty_does_not_demote_primary_headers() {
        let source = make_test_chunk("a", "src/search.rs", "code", "Function");
        let header = make_test_chunk("b", "include/search.h", "code", "Function");
        let benchmark = make_test_chunk("c", "benchmarks/search_benchmark.cc", "code", "Function");

        assert_eq!(
            chunk_density_exponent(&ChunkBoostContext::new(&source)),
            chunk_density_exponent(&ChunkBoostContext::new(&header))
        );
        assert!(
            chunk_density_exponent(&ChunkBoostContext::new(&header))
                < chunk_density_exponent(&ChunkBoostContext::new(&benchmark))
        );
    }

    #[test]
    fn exact_symbol_preview_centers_the_definition_after_documentation() {
        let content =
            "/**\n * Create an Axios instance.\n */\nclass Axios {\n  constructor() {}\n}\n";
        let lines = line_spans(content);
        let chunk = make_test_chunk("axios", "core/Axios.js", content, "Class");
        let query = PresentationQuery::new("Axios");

        assert_eq!(find_focus_line(&chunk, &query, content, &lines), 4);
    }

    #[test]
    #[serial]
    fn hybrid_search_e2e_with_boost_context_refactor() {
        // Full E2E: create a workspace with source + test files, search, and verify
        // that the refactored boost pipeline ranks the source definition above the test.
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tests")).unwrap();

        std::fs::write(
            tmp.path().join("src/calculator.rs"),
            "pub fn calculate_total(items: &[f64]) -> f64 {\n    items.iter().sum()\n}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("tests/calculator_test.rs"),
            "#[test]\nfn test_calculate_total() {\n    assert_eq!(calculate_total(&[1.0, 2.0]), 3.0);\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "calculate total",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();

        assert!(
            !hits.is_empty(),
            "search should return results for 'calculate total'"
        );

        // The source definition should rank first (src/ boost + definition kind boost)
        let first_path = &hits[0].file_path;
        assert!(
            first_path.to_string_lossy().contains("src/calculator.rs"),
            "definition in src/ should rank above test file, got {:?}",
            first_path
        );
    }

    #[test]
    #[serial]
    fn hybrid_search_e2e_path_exact_match() {
        // E2E: a query matching a directory name should surface files from that dir
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("tax-engine/src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("utils")).unwrap();

        std::fs::write(
            tmp.path().join("tax-engine/src/calc.rs"),
            "pub fn apply() { /* tax calculation logic */ }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("utils/helper.rs"),
            "pub fn apply() { /* generic helper */ }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = hybrid_search(
            &workspace,
            "tax engine",
            Some(&model),
            &SearchOptions::default(),
        )
        .unwrap();

        assert!(!hits.is_empty());
        // The file under tax-engine/ should rank first due to path exact match boost
        let first_path = hits[0].file_path.to_string_lossy();
        assert!(
            first_path.contains("tax-engine"),
            "path exact match should rank tax-engine/ first, got {first_path}"
        );
    }
}
