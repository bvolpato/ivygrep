use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::Connection;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::{
    BooleanQuery, Occur, Query, QueryParser, RegexQuery, TermQuery, TermSetQuery,
};
use tantivy::schema::IndexRecordOption;

use crate::embedding::EmbeddingModel;
use crate::indexer::{
    IndexedChunk, fetch_chunk_by_id, fetch_chunk_by_vector_key, fetch_chunks_by_vector_keys_batch,
    open_sqlite_readonly, open_tantivy_index,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::text::{singularize_token, split_identifier_segments};
use crate::vector_store::{HASH_VECTOR_QUANTIZATION, NEURAL_VECTOR_QUANTIZATION, VectorStore};
use crate::workspace::{Workspace, WorkspaceScope, index_path_string};

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

impl RawIndexedChunk {
    #[allow(dead_code)]
    fn decompress(self) -> IndexedChunk {
        IndexedChunk {
            chunk_id: self.chunk_id,
            file_path: self.file_path,
            start_line: self.start_line,
            end_line: self.end_line,
            language: self.language,
            kind: self.kind,
            text: crate::indexer::decompress_text(self.raw_text),
            content_hash: self.content_hash,
            vector_key: self.vector_key,
            is_ignored: self.is_ignored,
        }
    }
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
    pub progress_tx: Option<std::sync::mpsc::Sender<(String, usize, usize)>>,
    /// When set to `true`, the search should bail out as soon as possible.
    pub cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum QueryIntent {
    ExactIdentifier,
    Path,
    LiteralOrError,
    NaturalLanguage,
    DocsTestsExamples,
    Mixed,
}

impl QueryIntent {
    fn name(self) -> &'static str {
        match self {
            Self::ExactIdentifier => "exact-identifier",
            Self::Path => "path-file",
            Self::LiteralOrError => "literal-error",
            Self::NaturalLanguage => "natural-language-implementation",
            Self::DocsTestsExamples => "docs-tests-examples",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct QueryRouting {
    intent: QueryIntent,
    use_neural: bool,
    lexical_multiplier: usize,
    literal_multiplier: usize,
    semantic_multiplier: usize,
    symbol_limit: usize,
}

impl QueryRouting {
    fn classify(query: &str) -> Self {
        let trimmed = query.trim();
        let terms = raw_query_terms(trimmed);
        let lower = trimmed.to_ascii_lowercase();
        let concise_path_query = !trimmed.contains('\n') && terms.len() <= 8;
        let has_path_shape = concise_path_query
            && (trimmed.contains('/')
                || trimmed.contains('\\')
                || trimmed.split_whitespace().any(|term| {
                    Path::new(term.trim_matches(|ch: char| {
                        matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
                    }))
                    .extension()
                    .is_some_and(|extension| {
                        let length = extension.to_string_lossy().len();
                        (1..=8).contains(&length)
                    })
                }));
        let has_literal_shape = trimmed.contains('\n')
            || trimmed.contains('"')
            || trimmed.contains('\'')
            || lower.contains("error:")
            || lower.contains("exception")
            || lower.contains("traceback")
            || lower.contains("failed to");
        let targets_support = terms.iter().any(|term| {
            matches!(
                term.as_str(),
                "doc"
                    | "docs"
                    | "documentation"
                    | "readme"
                    | "test"
                    | "tests"
                    | "testing"
                    | "example"
                    | "examples"
                    | "sample"
                    | "samples"
            )
        });
        let exact_identifier = !trimmed.is_empty()
            && !trimmed.contains(char::is_whitespace)
            && trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.' | '$'));

        let intent = if has_path_shape {
            QueryIntent::Path
        } else if has_literal_shape {
            QueryIntent::LiteralOrError
        } else if exact_identifier {
            QueryIntent::ExactIdentifier
        } else if targets_support {
            QueryIntent::DocsTestsExamples
        } else if terms.len() >= 13 {
            QueryIntent::NaturalLanguage
        } else {
            QueryIntent::Mixed
        };
        match intent {
            QueryIntent::ExactIdentifier => Self {
                intent,
                use_neural: false,
                lexical_multiplier: 8,
                literal_multiplier: 6,
                semantic_multiplier: 1,
                symbol_limit: 100,
            },
            QueryIntent::Path => Self {
                intent,
                use_neural: false,
                lexical_multiplier: 8,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::LiteralOrError => Self {
                intent,
                // Long literals are usually pasted code or detailed prompts.
                // Keep exact retrieval dominant, but let semantic retrieval
                // contribute instead of treating every quote/newline as a
                // short error lookup.
                use_neural: terms.len() >= 13,
                lexical_multiplier: 10,
                literal_multiplier: 8,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::NaturalLanguage => Self {
                intent,
                use_neural: true,
                lexical_multiplier: 5,
                literal_multiplier: 4,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::DocsTestsExamples => Self {
                intent,
                use_neural: true,
                lexical_multiplier: 5,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::Mixed => Self {
                intent,
                use_neural: false,
                lexical_multiplier: 5,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 100,
            },
        }
    }
}

fn corpus_candidate_multiplier(document_count: u64) -> usize {
    match document_count {
        0..=50_000 => 1,
        50_001..=500_000 => 2,
        _ => 3,
    }
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
            progress_tx: None,
            cancel_token: None,
        }
    }
}

impl SearchOptions {
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
}

impl SearchContext {
    pub fn load(
        workspace: &Workspace,
        emb_dim: Option<usize>,
        wants_neural_vectors: bool,
    ) -> Result<Self> {
        let wants_hash_vectors = emb_dim.is_some();
        let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
        if use_overlay {
            let overlay_sqlite = open_sqlite_readonly(&workspace.overlay_sqlite_path())?;
            let (overlay_idx, fields) = open_tantivy_index(&workspace.overlay_tantivy_dir())?;
            let overlay_reader = overlay_idx.reader()?;
            let overlay_searcher = overlay_reader.searcher();
            let overlay_hash_vec = wants_hash_vectors
                .then(|| {
                    VectorStore::open_readonly(
                        &workspace.overlay_vector_path(),
                        256,
                        HASH_VECTOR_QUANTIZATION,
                    )
                    .ok()
                })
                .flatten();

            let base_dir = workspace
                .base_index_dir
                .clone()
                .unwrap_or_else(|| workspace.index_dir.clone());
            let base_sqlite = open_sqlite_readonly(&base_dir.join("metadata.sqlite3"))?;
            let (base_idx, _) = open_tantivy_index(&base_dir.join("tantivy"))?;
            let base_reader = base_idx.reader()?;
            let base_searcher = base_reader.searcher();
            let base_hash_vec = wants_hash_vectors
                .then(|| {
                    VectorStore::open_readonly(
                        &base_dir.join("vectors.usearch"),
                        256,
                        HASH_VECTOR_QUANTIZATION,
                    )
                    .ok()
                })
                .flatten();
            let base_neural_model = fs::read_to_string(base_dir.join("neural_model.json"))
                .ok()
                .and_then(|value| serde_json::from_str(&value).ok());
            let base_neural_dimensions = base_neural_model
                .as_ref()
                .map_or(384, |identity: &crate::embedding::NeuralModelIdentity| {
                    identity.dimensions
                });
            let base_neural_vec = wants_neural_vectors
                .then(|| {
                    VectorStore::open_readonly(
                        &base_dir.join("vectors_neural.usearch"),
                        base_neural_dimensions,
                        NEURAL_VECTOR_QUANTIZATION,
                    )
                    .ok()
                })
                .flatten();
            let base_neural_profile = fs::read_to_string(base_dir.join("neural_profile"))
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

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
                neural_vectors: None,
                base_neural_vectors: base_neural_vec,
                neural_profile: None,
                base_neural_profile,
                neural_model: None,
                base_neural_model,
                tombstones,
                overlay_files,
            })
        } else {
            let sqlite = open_sqlite_readonly(&workspace.sqlite_path())?;
            let (idx, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
            let reader = idx.reader()?;
            let searcher = reader.searcher();
            let hash_vec = wants_hash_vectors
                .then(|| {
                    VectorStore::open_readonly(
                        &workspace.vector_path(),
                        256,
                        HASH_VECTOR_QUANTIZATION,
                    )
                    .ok()
                })
                .flatten();
            let neural_model = workspace.neural_model_identity();
            let neural_dimensions = neural_model
                .as_ref()
                .map_or(384, |identity| identity.dimensions);
            let neural_vec = wants_neural_vectors
                .then(|| {
                    VectorStore::open_readonly(
                        &workspace.vector_neural_path(),
                        neural_dimensions,
                        NEURAL_VECTOR_QUANTIZATION,
                    )
                    .ok()
                })
                .flatten();
            let neural_profile = workspace.neural_profile_name();

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
        if let Ok(Some(chunk)) = fetch_chunk_by_vector_key(&self.sqlite, vector_key) {
            return Ok(Some(chunk));
        }
        if let Some(base_sqlite) = &self.base_sqlite
            && let Ok(Some(chunk)) = fetch_chunk_by_vector_key(base_sqlite, vector_key)
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
}

/// Fast index-backed literal text search.
///
/// Uses Tantivy to find candidate chunks containing the query terms,
/// then verifies exact case-insensitive substring matches only on those
/// candidates. Falls back to a full SQLite scan only when the query
/// contains terms that wouldn't be in the Tantivy tokenizer.
pub fn literal_search(
    workspace: &Workspace,
    query_text: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
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
    let query = query_text.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }

    let query_lower = query.to_ascii_lowercase();
    let max_hits = options.limit.unwrap_or(500);
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let glob_path_filter = build_glob_path_query_filter(ctx, &path_matcher, options)?;

    let matcher = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()?;

    // Use Tantivy index as a pre-filter: find candidate chunk IDs via the
    // inverted index, then only decompress those to verify the exact match.
    let candidate_chunks = collect_literal_candidates(
        ctx,
        query,
        &matcher,
        &path_matcher,
        &glob_path_filter,
        options,
    )?;

    tracing::trace!(
        "literal_scan={:?} candidates={}",
        t0.elapsed(),
        candidate_chunks.len()
    );

    // Now scan only the candidate chunks' source lines for precise matches and snippet extraction.
    // Group by file to read each file only once.
    let mut chunks_by_file: BTreeMap<PathBuf, Vec<IndexedChunk>> = BTreeMap::new();
    for chunk in candidate_chunks {
        chunks_by_file
            .entry(workspace.root.join(&chunk.file_path))
            .or_default()
            .push(chunk);
    }
    for chunks in chunks_by_file.values_mut() {
        chunks.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| a.end_line.cmp(&b.end_line))
                .then_with(|| a.chunk_id.cmp(&b.chunk_id))
        });
    }

    let mut hits = Vec::new();
    'outer: for (file_path, chunks) in &chunks_by_file {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        for chunk in chunks {
            // Scan lines within this chunk's range for the literal text.
            // Chunk bounds come from the index but the file is read live, so a
            // file truncated since indexing can make start exceed end — clamp
            // start to end to avoid an out-of-range slice panic.
            let end = chunk.end_line.min(lines.len());
            let start = chunk.start_line.saturating_sub(1).min(end);

            for (i, line) in lines[start..end].iter().enumerate() {
                let line_num = start + i + 1;
                let match_found = line.to_ascii_lowercase().contains(&query_lower);

                if match_found {
                    let (snippet_start, snippet_end) =
                        snippet_bounds(line_num, options.context, lines.len());
                    let preview = lines[snippet_start.saturating_sub(1)..snippet_end].join("\n");

                    hits.push(SearchHit {
                        file_path: chunk.file_path.clone(),
                        start_line: snippet_start,
                        end_line: snippet_end,
                        preview,
                        reason: format!("literal match: {}", truncate_for_reason(line.trim())),
                        score: 1.0,
                        sources: vec!["literal".to_string()],
                    });

                    if hits.len() >= max_hits {
                        break 'outer;
                    }
                }
            }
        }
    }

    tracing::trace!("literal_total={:?} hits={}", t0.elapsed(), hits.len());
    Ok(hits)
}

/// Use the Tantivy inverted index to find candidate chunks containing the
/// literal query, then verify with regex on the decompressed text.
/// This is O(index_lookup + matched_candidates) instead of O(all_chunks).
fn collect_literal_candidates(
    ctx: &SearchContext,
    query: &str,
    matcher: &regex::Regex,
    path_matcher: &PathGlobMatcher,
    glob_path_filter: &GlobPathQueryFilter,
    options: &SearchOptions,
) -> Result<Vec<IndexedChunk>> {
    let candidate_limit = if let Some(limit) = options.limit {
        if limit == usize::MAX {
            50_000
        } else {
            (limit * 5).clamp(200, 25_000)
        }
    } else {
        250
    };

    let mut search_fields = vec![ctx.fields.text, ctx.fields.file_path];
    if let Some(f) = ctx.fields.file_path_text {
        search_fields.push(f);
    }
    if let Some(f) = ctx.fields.signature {
        search_fields.push(f);
    }
    let mut parser = QueryParser::for_index(&ctx.indexes[0], search_fields);
    parser.set_conjunction_by_default();

    let mut found_ids = HashSet::<String>::new();
    let target_hits = options.limit.unwrap_or(100).min(500);

    // Phase 1: Collect candidate chunks from Tantivy (metadata only, no text).
    let mut candidates: Vec<IndexedChunk> = Vec::new();
    'outer: for lexical_query in build_lexical_queries(query) {
        let parsed_query = match parser.parse_query(&lexical_query) {
            Ok(q) => q,
            Err(_) => continue,
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
                    && found_ids.insert(chunk.chunk_id.clone())
                {
                    candidates.push(chunk);
                    if candidates.len() >= candidate_limit {
                        break 'outer;
                    }
                }
            }
        }
    }

    // Phase 2: Batch-fetch text from SQLite for all candidates at once.
    let empty_keys: Vec<u64> = candidates
        .iter()
        .filter(|c| c.text.is_empty())
        .map(|c| c.vector_key)
        .collect();
    if !empty_keys.is_empty()
        && let Ok(mut batch) = ctx.fetch_chunks_by_vector_keys_batch(&empty_keys)
    {
        for c in &mut candidates {
            if c.text.is_empty()
                && let Some(full) = batch.remove(&c.vector_key)
            {
                c.text = full.text;
            }
        }
    }

    // Phase 3: Verify exact substring match.
    let mut verified = Vec::<IndexedChunk>::new();
    for chunk in candidates {
        if matcher.is_match(&chunk.text) {
            verified.push(chunk);
            if verified.len() >= target_hits {
                break;
            }
        }
    }

    Ok(verified)
}

pub fn hybrid_search(
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let ctx = SearchContext::load(
        workspace,
        embedding_model.map(|model| model.dimensions()),
        embedding_model.is_some_and(|model| model.model_identity().is_some()),
    )?;
    hybrid_search_with_context(&ctx, workspace, query_text, embedding_model, options)
}

pub fn hybrid_search_with_context(
    ctx: &SearchContext,
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    // An empty/whitespace query has no lexical or literal terms; without this
    // guard the semantic pass would still embed "" and return arbitrary
    // nearest-neighbour noise. Match literal_search and return nothing.
    if query_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let t0 = std::time::Instant::now();
    let output_limit = options.limit.unwrap_or(50);
    let routing = QueryRouting::classify(query_text);
    let corpus_multiplier =
        corpus_candidate_multiplier(ctx.searchers.iter().map(tantivy::Searcher::num_docs).sum());
    // Tantivy lexical candidates: enough headroom for post-hoc filters
    // (gitignore, scope, globs) without blowing up on huge repos.
    // Default natural-language query: 50 → 250, --limit 500 → 2.5K.
    let candidate_limit = if output_limit == usize::MAX {
        50_000
    } else {
        (output_limit * routing.lexical_multiplier).clamp(100, 50_000)
    };
    // Literal pass needs exact substring verification via SQLite (text not
    // stored in Tantivy), so cap tighter: default → 250, scales up with limit.
    let literal_limit = if output_limit == usize::MAX {
        25_000
    } else {
        (output_limit * routing.literal_multiplier * corpus_multiplier).clamp(250, 25_000)
    };
    // Semantic (vector ANN) search: keep proportional but bounded.
    // Default ~50 → 50, --limit 500 → 500, --limit 5000 → 2000.
    // k=200 is ~30ms on 3M vectors; k=2000 is ~200ms. Both acceptable.
    let semantic_limit = if output_limit == usize::MAX {
        2_000
    } else {
        (output_limit * routing.semantic_multiplier * corpus_multiplier).clamp(50, 2_000)
    };
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let glob_path_filter = build_glob_path_query_filter(ctx, &path_matcher, options)?;

    tracing::trace!("open_tantivy={:?}", t0.elapsed());

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    // ── Literal pass ────────────────────────────────────────────────────
    // Always run a fast index-backed literal substring scan so exact matches
    // surface even when tokenization splits them differently.
    // Build a regex alternation of the original query plus snake_case/camelCase
    // variants so "hybrid search" also matches "hybrid_search" and "hybridSearch".
    let trimmed = query_text.trim();
    // Compute once — used by literal pass, lexical pass, and path-match pass.
    let lexical_queries = build_lexical_queries(trimmed);
    let literal_queries = build_literal_queries(trimmed, &lexical_queries);
    let symbol_candidate_limit = output_limit.clamp(20, routing.symbol_limit);
    let literal_matcher = if !literal_queries.is_empty() {
        let literal_pattern = literal_queries
            .iter()
            .map(|v| regex::escape(v))
            .collect::<Vec<_>>()
            .join("|");
        regex::RegexBuilder::new(&literal_pattern)
            .case_insensitive(true)
            .build()
            .ok()
    } else {
        None
    };
    let literal_chunks: Vec<(IndexedChunk, f32)> = if let Some(ref matcher) = literal_matcher {
        let mut all_candidates = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for variant in &literal_queries {
            let variant_matcher = regex::RegexBuilder::new(&regex::escape(variant))
                .case_insensitive(true)
                .build();
            if let Ok(ref vm) = variant_matcher
                && let Ok(mut candidates) = collect_literal_candidates(
                    ctx,
                    variant,
                    vm,
                    &path_matcher,
                    &glob_path_filter,
                    options,
                )
            {
                candidates.truncate(literal_limit);
                for c in candidates {
                    if seen.insert(c.chunk_id.clone()) {
                        all_candidates.push(c);
                    }
                }
            }
            // Once we have enough literal hits, stop trying variant queries
            if all_candidates.len() >= literal_limit {
                break;
            }
        }
        tracing::trace!(
            "literal_pass={:?} found={}",
            t0.elapsed(),
            all_candidates.len()
        );
        all_candidates
            .into_iter()
            .map(|c| {
                let count = matcher.find_iter(&c.text).count().max(1) as f32;
                let score = 1.0 + (count - 1.0).min(4.0) * 0.15; // 1.0 → 1.6 for 5+ matches
                (c, score)
            })
            .collect()
    } else {
        Vec::new()
    };

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    // ── Lexical (BM25) pass ─────────────────────────────────────────────
    // BM25F: search across text, tokenized file path, and definition signature.
    // Boosts on path/signature fields implement Sourcegraph-style BM25F where
    // matches on filenames and symbol definitions count 5× more than body text.
    let mut search_fields = vec![ctx.fields.text, ctx.fields.file_path];
    if let Some(f) = ctx.fields.file_path_text {
        search_fields.push(f);
    }
    if let Some(f) = ctx.fields.signature {
        search_fields.push(f);
    }
    let mut parser = QueryParser::for_index(&ctx.indexes[0], search_fields);
    parser.set_field_boost(ctx.fields.file_path, 2.0);
    if let Some(f) = ctx.fields.file_path_text {
        parser.set_field_boost(f, 5.0);
    }
    if let Some(f) = ctx.fields.signature {
        parser.set_field_boost(f, 10.0);
    }
    let conjunctive_numeric_query = should_use_conjunctive_numeric_query(trimmed);
    if conjunctive_numeric_query {
        parser.set_conjunction_by_default();
    }

    let mut allowed_languages = Vec::new();
    let mut can_pushdown_languages = options.include_globs.is_empty();
    if let Some(tf) = &options.type_filter {
        let resolved = crate::chunking::resolve_type_alias(tf)
            .map(|s| s.to_string())
            .unwrap_or_else(|| tf.to_string());
        allowed_languages.push(resolved);
        can_pushdown_languages = true;
    } else if !options.include_globs.is_empty() {
        can_pushdown_languages = true;
        for glob in &options.include_globs {
            let trimmed = glob.trim();
            if trimmed.starts_with("*.") && !trimmed.contains('/') && !trimmed.contains('?') {
                let ext = &trimmed[1..];
                if let Some(lang) =
                    crate::chunking::language_for_path(&PathBuf::from(format!("dummy{}", ext)))
                {
                    allowed_languages.push(lang.to_string());
                } else {
                    can_pushdown_languages = false;
                    break;
                }
            } else {
                can_pushdown_languages = false;
                break;
            }
        }
    }

    let mut lexical_by_id = HashMap::<String, (IndexedChunk, f32)>::new();
    let lexical_search_queries = if conjunctive_numeric_query {
        &lexical_queries[..1]
    } else {
        lexical_queries.as_slice()
    };
    for lexical_query in lexical_search_queries {
        let mut parsed_query = match parser.parse_query(lexical_query) {
            Ok(query) => query,
            Err(_) => continue,
        };
        parsed_query =
            constrain_query_to_scope(parsed_query, &ctx.fields, options.scope_filter.as_ref())?;
        parsed_query = constrain_query_to_glob_paths(parsed_query, &ctx.fields, &glob_path_filter);

        if can_pushdown_languages && !allowed_languages.is_empty() {
            let mut lang_queries: Vec<(tantivy::query::Occur, Box<dyn tantivy::query::Query>)> =
                Vec::new();
            for lang in &allowed_languages {
                let term = tantivy::Term::from_field_text(ctx.fields.language, lang);
                let q = Box::new(tantivy::query::TermQuery::new(
                    term,
                    tantivy::schema::IndexRecordOption::Basic,
                ));
                lang_queries.push((tantivy::query::Occur::Should, q));
            }
            let lang_boolean = Box::new(tantivy::query::BooleanQuery::new(lang_queries));

            let combined_queries = vec![
                (tantivy::query::Occur::Must, parsed_query),
                (
                    tantivy::query::Occur::Must,
                    lang_boolean as Box<dyn tantivy::query::Query>,
                ),
            ];
            parsed_query = Box::new(tantivy::query::BooleanQuery::new(combined_queries));
        }

        for (i, searcher) in ctx.searchers.iter().enumerate() {
            let lexical_docs = searcher.search(
                &parsed_query,
                &TopDocs::with_limit(candidate_limit).order_by_score(),
            )?;

            for (score, addr) in lexical_docs {
                let doc: TantivyDocument = searcher.doc(addr)?;
                if let Some(chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                    .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                    .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
                    .filter(|chunk| scope_matches(chunk, options.scope_filter.as_ref()))
                    .filter(|chunk| path_matches(chunk, &path_matcher))
                    .filter(|chunk| options.skip_gitignore || !chunk.is_ignored)
                {
                    let boosted = if is_definition_kind(&chunk.kind) {
                        score * 2.0
                    } else {
                        score
                    };
                    lexical_by_id
                        .entry(chunk.chunk_id.clone())
                        .and_modify(|(_, best)| *best = best.max(boosted))
                        .or_insert((chunk, boosted));
                }
            }
        }
    }
    // Sort by BM25 score and truncate to candidate_limit BEFORE populating
    // text from SQLite. This avoids O(all_results) individual SQLite lookups
    // — we only fetch text for the top-scoring candidates that will survive
    // RRF fusion.
    let mut lexical_chunks = lexical_by_id.into_values().collect::<Vec<_>>();
    lexical_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
    lexical_chunks.truncate(candidate_limit);

    // Exact persisted symbol definitions provide a separate bounded rank
    // signal. This avoids inferring every definition solely from text while
    // keeping symbol lookup independent from the main candidate volume.
    let mut symbol_names = lexical_queries.clone();
    symbol_names.push(trimmed.to_string());
    let mut symbol_chunks =
        crate::symbols::definition_candidates(&ctx.sqlite, &symbol_names, symbol_candidate_limit)?;
    if let Some(base_sqlite) = &ctx.base_sqlite {
        let remaining = symbol_candidate_limit.saturating_sub(symbol_chunks.len());
        if remaining > 0 {
            symbol_chunks.extend(
                crate::symbols::definition_candidates(base_sqlite, &symbol_names, remaining)?
                    .into_iter()
                    .filter(|chunk| !ctx.is_shadowed_base_file(1, &chunk.file_path)),
            );
        }
    }
    symbol_chunks.retain(|chunk| {
        type_matches(chunk, options.type_filter.as_deref())
            && scope_matches(chunk, options.scope_filter.as_ref())
            && path_matches(chunk, &path_matcher)
            && (options.skip_gitignore || !chunk.is_ignored)
    });
    symbol_chunks.truncate(symbol_candidate_limit);
    let symbol_chunks = symbol_chunks
        .into_iter()
        .enumerate()
        .map(|(rank, chunk)| (chunk, 1.0 / (rank as f32 + 1.0)))
        .collect::<Vec<_>>();

    // ── Path-match pass ──────────────────────────────────────────────────
    // Collect chunks whose file_path contains the query as a directory/file
    // name. This ensures "my-service" finds files under
    // apps/my-service/ even when the code-content BM25 candidates are
    // dominated by generic single-token matches like "service". These feed
    // their own ranked list in fusion (see fuse_rrf) rather than being
    // injected into the lexical pool with a fake score.
    let mut path_chunks: Vec<(IndexedChunk, f32)> = Vec::new();
    let run_path_pass = matches!(
        routing.intent,
        QueryIntent::ExactIdentifier | QueryIntent::Path
    ) || raw_query_terms(trimmed).len() <= 3;
    if run_path_pass && let Some(fpt_field) = ctx.fields.file_path_text {
        let mut path_parser = QueryParser::for_index(&ctx.indexes[0], vec![fpt_field]);
        path_parser.set_conjunction_by_default();
        // Reuse the lexical_queries computed at the start of hybrid_search.
        let path_query_variants = &lexical_queries;
        // Chunks already in the lexical pool are excluded from the path list
        // (they are ranked there); path-only candidates feed the path pass.
        let lexical_ids: HashSet<String> = lexical_chunks
            .iter()
            .map(|(c, _)| c.chunk_id.clone())
            .collect();

        // Phase 1: collect path-match candidates from Tantivy. The same chunk
        // can match across multiple query variants/searchers with different
        // path-field BM25 scores; dedupe by chunk_id and keep the *highest*
        // score, since path ranking (the path RRF pass) depends on it. Keeping
        // the first-seen score would mis-rank depending on iteration order.
        let mut path_by_id: HashMap<String, (IndexedChunk, f32)> = HashMap::new();
        for pq in path_query_variants {
            if let Ok(parsed) = path_parser.parse_query(pq)
                && let Ok(parsed) =
                    constrain_query_to_scope(parsed, &ctx.fields, options.scope_filter.as_ref())
            {
                let parsed = constrain_query_to_glob_paths(parsed, &ctx.fields, &glob_path_filter);
                for (i, searcher) in ctx.searchers.iter().enumerate() {
                    if let Ok(docs) =
                        searcher.search(&parsed, &TopDocs::with_limit(100).order_by_score())
                    {
                        for (score, addr) in docs {
                            if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
                                && let Some(chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                                    .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                                    .filter(|c| type_matches(c, options.type_filter.as_deref()))
                                    .filter(|c| scope_matches(c, options.scope_filter.as_ref()))
                                    .filter(|c| path_matches(c, &path_matcher))
                                    .filter(|c| options.skip_gitignore || !c.is_ignored)
                                && !lexical_ids.contains(&chunk.chunk_id)
                            {
                                path_by_id
                                    .entry(chunk.chunk_id.clone())
                                    .and_modify(|(_, s)| {
                                        if score > *s {
                                            *s = score;
                                        }
                                    })
                                    .or_insert((chunk, score));
                            }
                        }
                    }
                }
            }
        }
        let mut path_candidates: Vec<(IndexedChunk, f32)> = path_by_id.into_values().collect();

        // Phase 2: batch-fetch text for path candidates.
        let empty_keys: Vec<u64> = path_candidates
            .iter()
            .filter(|(c, _)| c.text.is_empty())
            .map(|(c, _)| c.vector_key)
            .collect();
        if !empty_keys.is_empty()
            && let Ok(mut batch) = ctx.fetch_chunks_by_vector_keys_batch(&empty_keys)
        {
            for (c, _) in &mut path_candidates {
                if c.text.is_empty()
                    && let Some(full) = batch.remove(&c.vector_key)
                {
                    c.text = full.text;
                }
            }
        }

        // Rank path matches by their path-field BM25 score; they feed their
        // own ranked list in fuse_rrf with a bounded weight.
        path_candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
        path_chunks = path_candidates;
    }

    // Batch-populate text from SQLite for the top chunks where Tantivy
    // doesn't store it. Uses a single batched query instead of N individual
    // round-trips, which is dramatically faster on large indexes.
    let empty_text_keys: Vec<u64> = lexical_chunks
        .iter()
        .filter(|(c, _)| c.text.is_empty())
        .map(|(c, _)| c.vector_key)
        .collect();
    if !empty_text_keys.is_empty()
        && let Ok(mut batch_result) = ctx.fetch_chunks_by_vector_keys_batch(&empty_text_keys)
    {
        for (chunk, _) in &mut lexical_chunks {
            if chunk.text.is_empty()
                && let Some(full) = batch_result.remove(&chunk.vector_key)
            {
                chunk.text = full.text;
            }
        }
    }
    tracing::trace!("lexical={:?} found={}", t0.elapsed(), lexical_chunks.len());

    tracing::trace!("open_vector={:?}", t0.elapsed());

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    let mut semantic_chunks = Vec::new();
    let has_hash_vectors = ctx.hash_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_hash_vectors.as_ref().map_or(0, |v| v.size()) > 0;
    let has_neural_vectors = ctx.neural_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_neural_vectors.as_ref().map_or(0, |v| v.size()) > 0;

    // Neural (MiniLM, 384-dim) embeddings are far higher quality than the
    // 256-bucket hash embeddings, so when neural vectors exist they should
    // dominate. But neural enhancement is incremental/resumable, so a partial
    // neural store is a normal state: dropping hash entirely would lose
    // semantic coverage for chunks not yet neural-embedded. Instead, keep hash
    // as a low-weight fallback whenever neural is present (the per-chunk merge
    // takes the max, so neural wins wherever it covers a chunk), and use hash
    // at full weight only when there is no neural store at all.
    let neural_profile_matches = embedding_model.is_none_or(|model| {
        let Some(active_identity) = model.model_identity() else {
            let Some(active_profile) = model.profile_info() else {
                return true;
            };
            return ctx
                .neural_profile
                .as_deref()
                .or(ctx.base_neural_profile.as_deref())
                .unwrap_or("general")
                == active_profile;
        };
        let Some(persisted_identity) = ctx.neural_model.as_ref().or(ctx.base_neural_model.as_ref())
        else {
            // Identity-less neural vectors predate complete model metadata and
            // must not be queried with a potentially incompatible revision.
            return false;
        };
        persisted_identity == active_identity
    });
    let neural_available = routing.use_neural
        && embedding_model.is_some_and(|model| model.model_identity().is_some())
        && has_neural_vectors
        && neural_profile_matches;
    let hash_vector_count = ctx.hash_vectors.as_ref().map_or(0, VectorStore::size)
        + ctx.base_hash_vectors.as_ref().map_or(0, VectorStore::size);
    let neural_vector_count = ctx.neural_vectors.as_ref().map_or(0, VectorStore::size)
        + ctx
            .base_neural_vectors
            .as_ref()
            .map_or(0, VectorStore::size);
    let hash_weight =
        semantic_hash_weight(neural_available, neural_vector_count, hash_vector_count);

    if embedding_model.is_some() && (has_hash_vectors || has_neural_vectors) {
        let mut semantic_by_id = HashMap::<String, (IndexedChunk, f32)>::new();
        let semantic_query_text = build_semantic_query_text(trimmed);

        if has_hash_vectors {
            // Reuse the caller's embedding_model to embed the query for hash
            // vector search — avoids rebuilding a HashEmbeddingModel per search.
            static SEARCH_HASH_MODEL: std::sync::OnceLock<crate::embedding::HashEmbeddingModel> =
                std::sync::OnceLock::new();
            let hash_model =
                SEARCH_HASH_MODEL.get_or_init(|| crate::embedding::HashEmbeddingModel::new(256));
            let hash_query_vector = hash_model.embed(&semantic_query_text);
            let hash_hits = collect_semantic_candidates(
                ctx,
                &path_matcher,
                options,
                &hash_query_vector,
                semantic_limit,
                ctx.hash_vectors.as_ref(),
                ctx.base_hash_vectors.as_ref(),
            )?;
            merge_semantic_candidates(&mut semantic_by_id, hash_hits, hash_weight);
        }

        if let Some(model) = embedding_model
            && routing.use_neural
            && model.model_identity().is_some()
            && has_neural_vectors
            && neural_profile_matches
        {
            let neural_query_vector = model.embed(query_text);
            let neural_hits = collect_semantic_candidates(
                ctx,
                &path_matcher,
                options,
                &neural_query_vector,
                semantic_limit,
                ctx.neural_vectors.as_ref(),
                ctx.base_neural_vectors.as_ref(),
            )?;
            merge_semantic_candidates(&mut semantic_by_id, neural_hits, 1.08);
        }

        semantic_chunks = semantic_by_id.into_values().collect::<Vec<_>>();
        semantic_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
    }
    tracing::trace!(
        "semantic={:?} found={}",
        t0.elapsed(),
        semantic_chunks.len()
    );

    if options.is_cancelled() {
        return Ok(Vec::new());
    }

    let merged = fuse_rrf(
        FusionCandidates {
            lexical: lexical_chunks,
            semantic: semantic_chunks,
            literal: literal_chunks,
            path: path_chunks,
            symbols: symbol_chunks,
        },
        if neural_available || query_targets_secondary_sources(query_text) {
            1.0
        } else {
            0.25
        },
        query_text,
        options.limit,
    );
    tracing::trace!("fuse_rrf={:?} merged={}", t0.elapsed(), merged.len());

    // Group hits by file path so we read each file only once
    let merged_len = merged.len();
    let mut hits_by_file: HashMap<PathBuf, Vec<(IndexedChunk, f32, Vec<String>)>> = HashMap::new();
    for (chunk, score, sources) in merged {
        hits_by_file
            .entry(workspace.root.join(&chunk.file_path))
            .or_default()
            .push((chunk, score, sources));
    }

    let file_count = hits_by_file.len();
    let mut hits = Vec::with_capacity(merged_len);
    for (file_path, file_hits) in hits_by_file {
        let file_content = fs::read_to_string(&file_path).ok();
        for (chunk, score, sources) in file_hits {
            hits.push(to_hit(
                workspace,
                chunk,
                query_text,
                score,
                sources,
                HitPresentation {
                    context_lines: options.context,
                    pre_read_content: file_content.as_deref(),
                    routing,
                },
            )?);
        }
    }
    // Re-sort since grouping by file changed the order
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    if !matches!(
        routing.intent,
        QueryIntent::ExactIdentifier | QueryIntent::Path
    ) {
        crate::reranker::rerank_hits(query_text, &mut hits);
    }
    tracing::trace!(
        "to_hit={:?} hits={} files_read={}",
        t0.elapsed(),
        hits.len(),
        file_count
    );

    Ok(hits)
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

fn to_hit(
    workspace: &Workspace,
    chunk: IndexedChunk,
    query_text: &str,
    score: f32,
    sources: Vec<String>,
    presentation: HitPresentation<'_>,
) -> Result<SearchHit> {
    let HitPresentation {
        context_lines,
        pre_read_content,
        routing,
    } = presentation;
    // Use Cow to avoid cloning the file content when the caller already read it.
    let content: std::borrow::Cow<'_, str> = match pre_read_content {
        Some(c) => std::borrow::Cow::Borrowed(c),
        None => {
            let file_path = workspace.root.join(&chunk.file_path);
            match fs::read_to_string(&file_path) {
                Ok(c) => std::borrow::Cow::Owned(c),
                Err(_) => {
                    return Ok(SearchHit {
                        file_path: chunk.file_path,
                        start_line: chunk.start_line,
                        end_line: chunk.end_line,
                        preview: chunk.text,
                        reason: format!(
                            "route={} neural={}; file no longer on disk",
                            routing.intent.name(),
                            routing.use_neural
                        ),
                        score,
                        sources,
                    });
                }
            }
        }
    };

    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(SearchHit {
            file_path: chunk.file_path,
            start_line: chunk.start_line,
            end_line: chunk.start_line,
            preview: String::new(),
            reason: format!(
                "route={} neural={}; empty file",
                routing.intent.name(),
                routing.use_neural
            ),
            score,
            sources,
        });
    }

    let focus_line = find_focus_line(&chunk, query_text, &lines);
    let (snippet_start, snippet_end) = snippet_bounds(focus_line, context_lines, lines.len());
    let preview = lines[snippet_start.saturating_sub(1)..snippet_end].join("\n");
    let ranking_reason = summarize_reason(
        query_text,
        lines
            .get(focus_line.saturating_sub(1))
            .copied()
            .unwrap_or_default(),
    );
    let reason = format!(
        "route={} neural={}; {ranking_reason}",
        routing.intent.name(),
        routing.use_neural
    );

    Ok(SearchHit {
        file_path: chunk.file_path,
        start_line: snippet_start,
        end_line: snippet_end,
        preview,
        reason,
        score,
        sources,
    })
}

struct HitPresentation<'a> {
    context_lines: usize,
    pre_read_content: Option<&'a str>,
    routing: QueryRouting,
}

fn find_focus_line(chunk: &IndexedChunk, query_text: &str, lines: &[&str]) -> usize {
    let line_count = lines.len();
    let window_start = chunk.start_line.max(1).min(line_count);
    let window_end = chunk.end_line.max(window_start).min(line_count);
    let query = query_text.trim();
    if query.is_empty() {
        return window_start;
    }

    let query_lower = query.to_ascii_lowercase();
    let query_compact = singularize_token(&compact_identifier(query));
    let query_tokens = expanded_query_tokens(query);

    let mut best_line = window_start;
    let mut best_score = 0.0f32;

    for line_no in window_start..=window_end {
        let line = lines[line_no - 1];
        let line_lower = line.to_ascii_lowercase();
        let mut line_score = 0.0f32;

        if line.contains(query) {
            line_score += 8.0;
        } else if line_lower.contains(&query_lower) {
            line_score += 5.0;
        }

        for token in &query_tokens {
            if line_lower.contains(token) {
                line_score += 1.5;
            }
        }

        if !query_compact.is_empty() {
            let line_compact = compact_identifier(line);
            if line_compact.contains(&query_compact) {
                line_score += 3.0;
            }
        }

        if line_score > best_score {
            best_score = line_score;
            best_line = line_no;
        }
    }

    best_line
}

fn snippet_bounds(focus_line: usize, context_lines: usize, line_count: usize) -> (usize, usize) {
    let start = focus_line.saturating_sub(context_lines).max(1);
    let end = (focus_line + context_lines).min(line_count);
    (start, end)
}

fn summarize_reason(query_text: &str, focus_line: &str) -> String {
    let focus = focus_line.trim();
    if focus.is_empty() {
        return "top hybrid relevance in this file".to_string();
    }

    let query = query_text.trim();
    if !query.is_empty() {
        let focus_lower = focus.to_ascii_lowercase();
        let query_lower = query.to_ascii_lowercase();

        if focus.contains(query) || focus_lower.contains(&query_lower) {
            return format!("line contains query terms: {}", truncate_for_reason(focus));
        }

        for token in expanded_query_tokens(query) {
            if focus_lower.contains(&token) {
                return format!(
                    "line matches token `{}`: {}",
                    token,
                    truncate_for_reason(focus)
                );
            }
        }
    }

    format!("top-ranked pointer: {}", truncate_for_reason(focus))
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

    queries.sort();
    queries.dedup();
    queries
}

fn should_run_literal_pass(query_text: &str) -> bool {
    let query = query_text.trim();
    if query.is_empty() {
        return false;
    }

    let tokens = tokenize_query(query);
    tokens.len() <= 2
        || query
            .chars()
            .any(|c| c == '_' || c == '-' || c == '/' || c == ':' || c.is_ascii_uppercase())
}

fn should_use_conjunctive_numeric_query(query_text: &str) -> bool {
    let terms = raw_query_terms(query_text);
    terms.len() >= 3
        && terms
            .iter()
            .any(|term| term.len() >= 2 && term.chars().all(|ch| ch.is_ascii_digit()))
}

fn build_literal_queries(query_text: &str, lexical_queries: &[String]) -> Vec<String> {
    if should_run_literal_pass(query_text) {
        return lexical_queries.to_vec();
    }

    let primary = tokenize_query(query_text);
    let mut aliases = crate::query_aliases::phrase_aliases(&primary)
        .into_iter()
        .filter(|alias| alias.len() >= 5 || alias.contains('_'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn tokenize_query(query: &str) -> Vec<String> {
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

fn raw_query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
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

#[derive(Debug, Default)]
struct GlobPathQueryFilter {
    included_paths: Option<Vec<String>>,
    excluded_paths: Option<Vec<String>>,
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
fn collect_filtered_chunks(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    scope_filter: Option<&WorkspaceScope>,
    type_filter: Option<&str>,
    include_globs: &[String],
    skip_gitignore: bool,
) -> Vec<RawIndexedChunk> {
    let mut chunks = query_filtered_chunks(
        &ctx.sqlite,
        path_matcher,
        scope_filter,
        type_filter,
        include_globs,
        skip_gitignore,
    );
    if let Some(base_sqlite) = &ctx.base_sqlite {
        let mut base_chunks = query_filtered_chunks(
            base_sqlite,
            path_matcher,
            scope_filter,
            type_filter,
            include_globs,
            skip_gitignore,
        );
        base_chunks.retain(|c| !ctx.is_shadowed_base_file(1, &c.file_path));
        chunks.extend(base_chunks);
    }
    chunks
}

fn query_filtered_chunks(
    conn: &Connection,
    path_matcher: &PathGlobMatcher,
    scope_filter: Option<&WorkspaceScope>,
    type_filter: Option<&str>,
    include_globs: &[String],
    skip_gitignore: bool,
) -> Vec<RawIndexedChunk> {
    // Build a SQL query that pushes as much filtering as possible into SQLite.
    let mut sql = String::from(
        "SELECT chunk_id, file_path, start_line, end_line, language, kind, x'', content_hash, vector_key, is_ignored FROM chunks WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if !skip_gitignore {
        sql.push_str(" AND is_ignored = 0");
    }

    if let Some(tf) = type_filter {
        sql.push_str(" AND language = ?");
        params_vec.push(Box::new(tf.to_string()));
    }

    if let Some(scope) = scope_filter {
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

    // Push simple extension globs into SQL for massive performance gains.
    // e.g., "*.yaml" -> language IN ('yaml') (Hits the SQLite index instantly!)
    // Instead of doing `file_path LIKE '%.yaml'` which triggers a full table scan.
    let mut sql_ext_filters: Vec<String> = Vec::new();
    for glob in include_globs {
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

    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();
    let Ok(rows) = stmt.query_map(params_refs.as_slice(), |row| {
        let raw_text: Vec<u8> = row.get(6)?;
        Ok(RawIndexedChunk {
            chunk_id: row.get(0)?,
            file_path: PathBuf::from(row.get::<_, String>(1)?),
            start_line: row.get::<_, i64>(2)? as usize,
            end_line: row.get::<_, i64>(3)? as usize,
            language: row.get(4)?,
            kind: row.get(5)?,
            raw_text,
            content_hash: row.get(7)?,
            vector_key: row.get::<_, i64>(8)? as u64,
            is_ignored: row.get::<_, bool>(9)?,
        })
    }) else {
        return Vec::new();
    };

    // Apply full glob filtering in Rust for complex patterns
    rows.flatten()
        .filter(|chunk| scope_path_matches(&chunk.file_path, scope_filter))
        .filter(|chunk| path_matcher.matches(&chunk.file_path))
        .collect()
}

fn collect_semantic_candidates(
    ctx: &SearchContext,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
    query_vector: &[f32],
    candidate_limit: usize,
    primary_store: Option<&VectorStore>,
    base_store: Option<&VectorStore>,
) -> Result<Vec<(IndexedChunk, f32)>> {
    const MAX_EXACT_FILTERED_CANDIDATES: usize = 50_000;

    let has_filters = !options.include_globs.is_empty()
        || !options.exclude_globs.is_empty()
        || options.scope_filter.is_some()
        || options.type_filter.is_some();

    // ANN cannot push path filters into the graph. For focused searches, exact
    // scoring over the filtered subset is both complete and cheap. This avoids
    // losing the best scoped result behind globally-nearer out-of-scope chunks.
    if has_filters {
        let filtered = collect_filtered_chunks(
            ctx,
            path_matcher,
            options.scope_filter.as_ref(),
            options.type_filter.as_deref(),
            &options.include_globs,
            options.skip_gitignore,
        );
        if filtered.len() <= MAX_EXACT_FILTERED_CANDIDATES {
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
        // Over-fetch so we still have enough candidates after filtering.
        (candidate_limit * 10).min(20_000)
    } else {
        candidate_limit
    };

    let mut matches = Vec::new();
    if let Some(store) = primary_store {
        matches.extend(store.search(query_vector, ann_limit));
    }
    if let Some(store) = base_store {
        matches.extend(store.search(query_vector, ann_limit));
    }
    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut seen_keys = HashSet::new();
    matches.retain(|vector_match| seen_keys.insert(vector_match.key));
    matches.truncate(ann_limit);

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

    Ok(semantic_chunks)
}

fn score_filtered_semantic_candidates(
    ctx: &SearchContext,
    filtered: Vec<RawIndexedChunk>,
    query_vector: &[f32],
    candidate_limit: usize,
    primary_store: Option<&VectorStore>,
    base_store: Option<&VectorStore>,
) -> Result<Vec<(IndexedChunk, f32)>> {
    let mut scored = filtered
        .into_iter()
        .filter_map(|chunk| {
            let score = primary_store
                .and_then(|store| store.score(chunk.vector_key, query_vector))
                .into_iter()
                .chain(base_store.and_then(|store| store.score(chunk.vector_key, query_vector)))
                .max_by(|a, b| a.total_cmp(b))?;
            Some((chunk.vector_key, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(candidate_limit);

    let keys = scored.iter().map(|(key, _)| *key).collect::<Vec<_>>();
    let chunks = ctx.fetch_chunks_by_vector_keys_batch(&keys)?;
    Ok(scored
        .into_iter()
        .filter_map(|(key, score)| chunks.get(&key).cloned().map(|chunk| (chunk, score)))
        .collect())
}

fn merge_semantic_candidates(
    semantic_by_id: &mut HashMap<String, (IndexedChunk, f32)>,
    hits: Vec<(IndexedChunk, f32)>,
    score_multiplier: f32,
) {
    for (chunk, score) in hits {
        let adjusted = score * score_multiplier;
        semantic_by_id
            .entry(chunk.chunk_id.clone())
            .and_modify(|(_, best_score)| *best_score = best_score.max(adjusted))
            .or_insert((chunk, adjusted));
    }
}

struct FusionCandidates {
    lexical: Vec<(IndexedChunk, f32)>,
    semantic: Vec<(IndexedChunk, f32)>,
    literal: Vec<(IndexedChunk, f32)>,
    path: Vec<(IndexedChunk, f32)>,
    symbols: Vec<(IndexedChunk, f32)>,
}

fn fuse_rrf(
    candidates: FusionCandidates,
    semantic_direct_weight: f32,
    query_text: &str,
    limit: Option<usize>,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    const K: f32 = 60.0;
    const LEXICAL_WEIGHT: f32 = 3.2;
    const SEMANTIC_WEIGHT: f32 = 1.0;
    const LITERAL_WEIGHT: f32 = 4.0;
    // Path matches (file_path contains the query) are a useful but bounded
    // signal. They get a moderate rank-based weight — enough to surface a
    // file whose path matches the query when content candidates are weak,
    // without overriding strong content matches. The path-aware boosts below
    // (path_exact_match/path_segment/file_stem) still apply on top.
    const PATH_WEIGHT: f32 = 1.5;
    const SYMBOL_WEIGHT: f32 = 10.0;
    const LEXICAL_SCORE_WEIGHT: f32 = 0.05;
    const SEMANTIC_SCORE_WEIGHT: f32 = 0.08;
    const SEMANTIC_ONLY_PENALTY: f32 = 0.60;
    const TERM_COVERAGE_WEIGHT: f32 = 0.35;
    const PATH_SEGMENT_WEIGHT: f32 = 0.40;
    const FILE_STEM_WEIGHT: f32 = 0.50;
    const DEFINITION_NAME_BONUS: f32 = 0.25;
    const LOCATION_INTENT_WEIGHT: f32 = 0.20;
    // Path-exact matches now also feed their own ranked RRF list (see the
    // `path` pass above), so this additive boost no longer needs to be large
    // enough to single-handedly win — it was 3.0, ~60x the base RRF score.
    const PATH_EXACT_MATCH_WEIGHT: f32 = 0.8;
    const FILE_COVERAGE_WEIGHT: f32 = 3.0;
    const EXACT_LITERAL_MULTIPLIER: f32 = 1.8;
    const ALIAS_LITERAL_MULTIPLIER: f32 = 1.35;
    // Bound the total additive boost relative to the fused base score so
    // boosts perturb the RRF ranking rather than replace it.
    const MAX_BOOST_RATIO: f32 = 3.0;
    const MAX_BOOST_FLOOR: f32 = 0.25;

    let FusionCandidates {
        lexical,
        semantic,
        literal,
        path,
        symbols,
    } = candidates;

    let query_tokens = expanded_query_tokens(query_text);
    let location_intent = has_location_intent(query_text);
    let direct_ids = lexical
        .iter()
        .map(|(chunk, _)| chunk.chunk_id.clone())
        .chain(literal.iter().map(|(chunk, _)| chunk.chunk_id.clone()))
        .chain(path.iter().map(|(chunk, _)| chunk.chunk_id.clone()))
        .chain(symbols.iter().map(|(chunk, _)| chunk.chunk_id.clone()))
        .collect::<HashSet<_>>();

    struct RrfEntry {
        score: f32,
        chunk: IndexedChunk,
        sources: HashSet<&'static str>,
    }

    let mut entries: HashMap<String, RrfEntry> = HashMap::new();
    let mut add_entry = |chunk: IndexedChunk, score: f32, source: &'static str| {
        let entry = entries
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| RrfEntry {
                score: 0.0,
                chunk,
                sources: HashSet::new(),
            });
        entry.score += score;
        entry.sources.insert(source);
    };

    for (rank, (chunk, lexical_score)) in lexical.into_iter().enumerate() {
        add_entry(
            chunk,
            LEXICAL_WEIGHT / (K + rank as f32 + 1.0)
                + normalize_lexical_score(lexical_score) * LEXICAL_SCORE_WEIGHT,
            "lexical",
        );
    }

    for (rank, (chunk, semantic_score)) in semantic.into_iter().enumerate() {
        // Hash vectors are a cheap provisional recall tier. Keep full strength
        // for semantic-only discovery, but do not let hash collisions overrule
        // direct evidence. Neural vectors use semantic_direct_weight=1.0.
        let direct_weight = if direct_ids.contains(&chunk.chunk_id) {
            semantic_direct_weight
        } else {
            1.0
        };
        add_entry(
            chunk,
            direct_weight * SEMANTIC_WEIGHT / (K + rank as f32 + 1.0)
                + direct_weight * normalize_semantic_score(semantic_score) * SEMANTIC_SCORE_WEIGHT,
            "semantic",
        );
    }

    // Literal pass: verified exact substring matches get a strong boost
    for (rank, (chunk, _)) in literal.into_iter().enumerate() {
        add_entry(chunk, LITERAL_WEIGHT / (K + rank as f32 + 1.0), "literal");
    }

    // Path pass: chunks whose file path matches the query, ranked by their
    // path-field BM25 score. Rank-based only — no raw-score magnitude term —
    // so a path match can't dominate via an out-of-scale score.
    for (rank, (chunk, _)) in path.into_iter().enumerate() {
        add_entry(chunk, PATH_WEIGHT / (K + rank as f32 + 1.0), "path");
    }

    for (rank, (chunk, _)) in symbols.into_iter().enumerate() {
        add_entry(chunk, SYMBOL_WEIGHT / (K + rank as f32 + 1.0), "symbol");
    }

    let rerank_limit = rerank_candidate_limit();
    let mut rerank_order = entries
        .iter()
        .map(|(chunk_id, entry)| (chunk_id.clone(), entry.score))
        .collect::<Vec<_>>();
    rerank_order.sort_by(|left, right| right.1.total_cmp(&left.1));
    let rerank_ids = rerank_order
        .into_iter()
        .take(rerank_limit)
        .map(|(chunk_id, _)| chunk_id)
        .collect::<HashSet<_>>();

    let primary_query_tokens = tokenize_query(query_text);
    let mut file_query_matches: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
    let mut boost_contexts = HashMap::with_capacity(entries.len());
    for (chunk_id, entry) in &entries {
        if !rerank_ids.contains(chunk_id) {
            continue;
        }
        let bctx = ChunkBoostContext::new(&entry.chunk);
        if primary_query_tokens.len() >= 3 {
            let matches = file_query_matches
                .entry(entry.chunk.file_path.clone())
                .or_default();
            for (idx, token) in primary_query_tokens.iter().enumerate() {
                if bctx.text_lower.contains(token.as_str())
                    || bctx.path_lower.contains(token.as_str())
                {
                    matches.insert(idx);
                }
            }
        }
        boost_contexts.insert(chunk_id.clone(), bctx);
    }

    // Count how many candidate chunks each file contributes. Secondary-source
    // files with many chunks get a density penalty so they cannot dominate by
    // contributing more candidates; primary implementation files are exempt.
    let mut file_chunk_counts: HashMap<PathBuf, usize> = HashMap::new();
    for e in entries.values() {
        *file_chunk_counts
            .entry(e.chunk.file_path.clone())
            .or_insert(0) += 1;
    }

    let mut ranked = entries
        .into_values()
        .map(|e| {
            let RrfEntry {
                score: base_score,
                chunk,
                sources: source_set,
            } = e;
            let mut source_list = source_set
                .iter()
                .map(|source| (*source).to_string())
                .collect::<Vec<_>>();
            source_list.sort();

            if !rerank_ids.contains(&chunk.chunk_id) {
                return (chunk, base_score, source_list);
            }

            // Precompute lowercased text/path once per candidate instead of
            // redundantly in every boost function.
            let bctx = boost_contexts
                .remove(&chunk.chunk_id)
                .unwrap_or_else(|| ChunkBoostContext::new(&chunk));

            // Accumulate signal boosts separately from the RRF base so they can
            // be bounded. Previously these were added directly and several were
            // 10-60x the base RRF score (~0.05), so a single boost could
            // override the fused rank signal entirely.
            let mut additive_boost = literal_match_boost(query_text, &bctx);

            let coverage = if !query_tokens.is_empty() {
                term_coverage_boost(&query_tokens, &bctx)
            } else {
                0.0
            };
            additive_boost += coverage * TERM_COVERAGE_WEIGHT;

            if !query_tokens.is_empty() {
                additive_boost += path_segment_boost(&query_tokens, &bctx) * PATH_SEGMENT_WEIGHT;
            }

            additive_boost += path_exact_match_boost(query_text, &bctx) * PATH_EXACT_MATCH_WEIGHT;

            if !query_tokens.is_empty() {
                additive_boost += file_stem_boost(&query_tokens, &bctx) * FILE_STEM_WEIGHT;
            }

            if !query_tokens.is_empty() {
                additive_boost +=
                    definition_name_boost(&query_tokens, &bctx) * DEFINITION_NAME_BONUS;
            }

            if location_intent {
                additive_boost += location_intent_boost(&chunk, &bctx) * LOCATION_INTENT_WEIGHT;
            }

            // Keep RRF as the primary ranking signal: cap the total additive
            // boost so it perturbs the fused base score rather than dominating
            // it. The cap scales with the base (with a small floor so even
            // weak-base candidates get a meaningful, bounded lift).
            let boost_cap = (base_score * MAX_BOOST_RATIO).max(MAX_BOOST_FLOOR);
            let mut score = base_score + additive_boost.min(boost_cap);

            if let Some(matches) = file_query_matches.get(&chunk.file_path)
                && matches.len() >= 2
            {
                let file_coverage = matches.len() as f32 / primary_query_tokens.len() as f32;
                score *= 1.0 + file_coverage * file_coverage * FILE_COVERAGE_WEIGHT;
            }

            if source_set.contains("literal") {
                score *= if should_run_literal_pass(query_text) {
                    EXACT_LITERAL_MULTIPLIER
                } else {
                    ALIAS_LITERAL_MULTIPLIER
                };
            }

            if !source_set.contains("lexical") && !source_set.contains("literal") {
                score *= SEMANTIC_ONLY_PENALTY;
            }

            // Chunks with zero query term overlap despite having text are noise
            if !query_tokens.is_empty()
                && coverage < f32::EPSILON
                && !source_set.contains("literal")
            {
                score *= 0.5;
            }

            score *= chunk_kind_boost(&chunk);
            score *= effective_authority_score(query_text, &query_tokens, &bctx);

            // Apply chunk-density normalization: 1/n^x where n is the number
            // of chunks this file has in the candidate set. Primary
            // implementation files use x=0 and are unaffected.
            let n_file_chunks = file_chunk_counts
                .get(&chunk.file_path)
                .copied()
                .unwrap_or(1) as f32;
            score /= n_file_chunks.powf(chunk_density_exponent(&bctx));

            (chunk, score, source_list)
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // Per-file hit diversity cap: keep the best chunk per file at full score,
    // then aggressively decay. This mirrors web-search result diversity: a
    // second snippet from the same file can still show up, but should not crowd
    // out another authoritative file.
    let mut file_hit_counts: HashMap<PathBuf, usize> = HashMap::new();
    for item in &mut ranked {
        let count = file_hit_counts.entry(item.0.file_path.clone()).or_insert(0);
        *count += 1;
        match *count {
            1 => {}
            2 => item.1 *= 0.35,
            3..=4 => item.1 *= 0.15,
            _ => item.1 *= 0.05,
        }
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    let mut filtered = filter_meaningful_scores(ranked, query_text);

    if let Some(limit) = limit {
        filtered.truncate(limit);
    }

    filtered
}

pub fn rerank_candidate_limit() -> usize {
    std::env::var("IVYGREP_RERANK_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(100)
}

fn filter_meaningful_scores(
    ranked: Vec<(IndexedChunk, f32, Vec<String>)>,
    query_text: &str,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    let precise_query = is_precise_lookup_query(query_text);
    let query_tokens = expanded_query_tokens(query_text);
    if ranked.is_empty() {
        return vec![];
    }

    let best_score = ranked[0].1;
    let has_direct_candidate = ranked
        .iter()
        .any(|(_, _, sources)| has_direct_source(sources));
    if !has_direct_candidate {
        return filter_semantic_only_scores(ranked, query_text, &query_tokens, precise_query);
    }

    if ranked.len() == 1 {
        let (chunk, _, sources) = &ranked[0];
        if direct_candidate_has_enough_authority(chunk, sources, query_text, &query_tokens) {
            return ranked;
        }
        return vec![];
    }

    // Adaptive threshold: start from score distribution, then clamp against
    // the best result. Low-authority files are suppressed unless the query is
    // an exact identifier/path-style lookup with a verified literal hit; this
    // avoids fixture/data/vendor junk leaking into high-confidence advice.
    let mean = ranked.iter().map(|(_, score, _)| score).sum::<f32>() / ranked.len() as f32;
    let variance = ranked
        .iter()
        .map(|(_, score, _)| (score - mean).powi(2))
        .sum::<f32>()
        / ranked.len() as f32;
    let stddev = variance.sqrt();
    let adaptive_threshold = (mean - stddev).max(best_score * 0.35).max(0.010);

    let is_meaningful = |(chunk, score, sources): &(IndexedChunk, f32, Vec<String>)| {
        let bctx = ChunkBoostContext::new(chunk);
        let authority = effective_authority_score(query_text, &query_tokens, &bctx);
        let authority_floor =
            recommendation_authority_floor(query_text, &query_tokens, sources, precise_query);
        if has_literal_source(sources) {
            return authority >= authority_floor
                && (precise_query || *score >= adaptive_threshold * 0.7);
        }
        *score >= adaptive_threshold && authority >= authority_floor
    };
    let best_is_fallback = has_direct_source(&ranked[0].2)
        && direct_candidate_has_enough_authority(
            &ranked[0].0,
            &ranked[0].2,
            query_text,
            &query_tokens,
        );
    let mut ranked = ranked.into_iter();
    let best = ranked.next().expect("ranked is non-empty");
    let mut fallback = None;
    let mut filtered = Vec::new();
    if is_meaningful(&best) {
        filtered.push(best);
    } else if best_is_fallback {
        fallback = Some(best);
    }
    filtered.extend(ranked.filter(is_meaningful));
    if filtered.is_empty()
        && let Some(best) = fallback
    {
        filtered.push(best);
    }

    filtered
}

fn filter_semantic_only_scores(
    ranked: Vec<(IndexedChunk, f32, Vec<String>)>,
    query_text: &str,
    query_tokens: &[String],
    precise_query: bool,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    let Some(best) = ranked.first() else {
        return vec![];
    };

    let bctx = ChunkBoostContext::new(&best.0);
    let support = support_signals(query_text, query_tokens, &bctx);
    let authority = effective_authority_score(query_text, query_tokens, &bctx);
    let second_score = ranked.get(1).map(|(_, score, _)| *score).unwrap_or(0.0);
    let authority_floor = if query_targets_secondary_sources(query_text) || precise_query {
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
    let decisive = best.1 >= score_floor
        || (best.1 >= score_floor * 0.8
            && second_score > f32::EPSILON
            && best.1 / second_score >= decisive_ratio);

    if authority >= authority_floor
        && decisive
        && support.is_enough_for_semantic_only(precise_query)
    {
        ranked.into_iter().take(1).collect()
    } else {
        vec![]
    }
}

fn has_direct_source(sources: &[String]) -> bool {
    has_literal_source(sources)
        || sources
            .iter()
            .any(|source| source == "lexical" || source == "path")
}

fn has_literal_source(sources: &[String]) -> bool {
    sources.iter().any(|source| source == "literal")
}

fn direct_candidate_has_enough_authority(
    chunk: &IndexedChunk,
    sources: &[String],
    query_text: &str,
    query_tokens: &[String],
) -> bool {
    let bctx = ChunkBoostContext::new(chunk);
    let authority = effective_authority_score(query_text, query_tokens, &bctx);
    authority
        >= recommendation_authority_floor(
            query_text,
            query_tokens,
            sources,
            is_precise_lookup_query(query_text),
        )
}

fn recommendation_authority_floor(
    query_text: &str,
    query_tokens: &[String],
    sources: &[String],
    precise_query: bool,
) -> f32 {
    if query_targets_secondary_sources(query_text) {
        return 0.30;
    }
    if precise_query || is_short_literal_lookup_query(query_text) {
        return 0.35;
    }
    if query_targets_implementation(query_tokens) {
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

fn is_short_literal_lookup_query(query_text: &str) -> bool {
    let terms = raw_query_terms(query_text);
    !terms.is_empty() && terms.len() <= 2 && !has_location_intent(query_text)
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

fn is_precise_lookup_query(query_text: &str) -> bool {
    let query = query_text.trim();
    !query.is_empty()
        && (tokenize_query(query).len() == 1
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
    /// Path split on '/', owned for lifetime independence.
    path_segments: Vec<String>,
    /// Lowercased file stem (e.g. "search" from "search.rs").
    file_stem: Option<String>,
    /// First meaningful line of the chunk (lowercased) — used for definition name boost.
    first_line: String,
    /// compact_identifier of the full chunk text (for literal_match_boost).
    text_compact: String,
    /// compact_identifier of the file path (for literal_match_boost).
    path_compact: String,
}

impl ChunkBoostContext {
    fn new(chunk: &IndexedChunk) -> Self {
        let text_lower = chunk.text.to_ascii_lowercase();
        let path_string = index_path_string(&chunk.file_path);
        let path_lower = path_string.to_ascii_lowercase();
        let path_segments: Vec<String> = path_lower.split('/').map(String::from).collect();
        let file_stem = chunk
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        let first_line = chunk
            .text
            .lines()
            .find(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with("//") && !t.starts_with('#')
            })
            .unwrap_or_default()
            .to_ascii_lowercase();

        let text_compact = compact_identifier(&chunk.text);
        let path_compact = compact_identifier(&path_string);

        Self {
            text_lower,
            path_lower,
            path_segments,
            file_stem,
            first_line,
            text_compact,
            path_compact,
        }
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
            bctx.path_segments
                .iter()
                .any(|seg| seg.contains(t.as_str()))
        })
        .count();
    matched as f32 / query_tokens.len() as f32
}

/// Massive boost when the full query appears as a path segment (directory or
/// file name). Searching "my-service" should rank files under a directory
/// literally named "my-service/" far above random code mentions.
fn path_exact_match_boost(query: &str, bctx: &ChunkBoostContext) -> f32 {
    let query_lower = query.trim().to_ascii_lowercase();
    if query_lower.is_empty() {
        return 0.0;
    }

    // Also build variants: "my service" -> "my-service", "my_service"
    let hyphenated = query_lower.replace(' ', "-");
    let underscored = query_lower.replace(' ', "_");
    let compacted = query_lower.replace(' ', "");

    let candidates = [&query_lower, &hyphenated, &underscored, &compacted];

    for seg in &bctx.path_segments {
        for candidate in &candidates {
            // Exact segment match: dir name IS the query
            if seg == candidate.as_str() {
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
    for candidate in &candidates {
        if candidate.len() >= 4 && bctx.path_lower.contains(candidate.as_str()) {
            return 0.4;
        }
    }

    0.0
}

fn file_stem_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let Some(ref stem) = bctx.file_stem else {
        return 0.0;
    };

    let compact_stem = compact_identifier(stem);
    let exact_match = query_tokens
        .iter()
        .any(|token| *stem == *token || compact_stem == compact_identifier(token));
    let partial_match = query_tokens
        .iter()
        .any(|token| stem.contains(token.as_str()));

    if exact_match {
        1.0
    } else if partial_match {
        0.5
    } else {
        0.0
    }
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

/// Bonus when a chunk's definition name (first non-blank line) contains query tokens.
/// This is the "are we looking at the definition site?" signal — e.g., query "handle error"
/// should strongly prefer `fn handle_error()` over a comment mentioning errors.
fn definition_name_boost(query_tokens: &[String], bctx: &ChunkBoostContext) -> f32 {
    if query_tokens.is_empty() || bctx.first_line.is_empty() {
        return 0.0;
    }

    let matched = query_tokens
        .iter()
        .filter(|t| bctx.first_line.contains(t.as_str()))
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn literal_match_boost(query_text: &str, bctx: &ChunkBoostContext) -> f32 {
    const LITERAL_MATCH_BOOST: f32 = 0.20;
    const NORMALIZED_IDENTIFIER_BOOST: f32 = 0.10;

    let query = query_text.trim();
    if query.is_empty() {
        return 0.0;
    }

    let query_lower = query.to_ascii_lowercase();
    if bctx.text_lower.contains(&query_lower) || bctx.path_lower.contains(&query_lower) {
        return LITERAL_MATCH_BOOST;
    }

    let query_compact = compact_identifier(query);
    if query_compact.is_empty() {
        return 0.0;
    }

    if bctx.text_compact.contains(&query_compact) || bctx.path_compact.contains(&query_compact) {
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
        // Definition sites are the most valuable — this is PageRank-like thinking:
        // the place where something is *defined* is almost always what the user wants.
        "Function" | "function" => 1.35,
        "Class" | "class" | "Struct" | "struct" | "Trait" | "trait" | "Interface" | "interface" => {
            1.4
        }
        "Impl" | "impl" | "Enum" | "enum" => 1.25,

        // Imports and comments are rarely the target of a search
        "Comment" | "comment" => 0.6,
        "Import" | "import" | "Use" | "use" => 0.65,

        // Generic blocks (if/for/match arms, raw lines) are low-signal:
        // they match many terms but rarely contain the definition the user wants
        "Block" | "block" => 0.75,

        _ => 1.0,
    }
}

/// File authority scoring inspired by PageRank: implementation code is usually
/// more authoritative than support files, tests, fixtures, docs, data files, and
/// vendored dependencies.
fn file_authority_score(bctx: &ChunkBoostContext) -> f32 {
    let path = &bctx.path_lower;

    // Vendored / dependency code — almost never what the user wants
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

    // Lock files, minified bundles, source maps — machine-generated noise
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

fn effective_authority_score(
    query_text: &str,
    query_tokens: &[String],
    bctx: &ChunkBoostContext,
) -> f32 {
    let mut score = file_authority_score(bctx);
    let secondary_intent = query_targets_secondary_sources(query_text);

    if !secondary_intent {
        if path_depth(&bctx.path_lower) <= 3
            && path_role(&bctx.path_lower) == PathRole::PrimarySource
        {
            score *= 1.08;
        }
        if path_depth(&bctx.path_lower) >= 6 {
            score *= match path_query_overlap(query_tokens, bctx) {
                0 => 0.74,
                1 => 0.86,
                _ => 0.95,
            };
        }
    }

    score
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

fn path_query_overlap(query_tokens: &[String], bctx: &ChunkBoostContext) -> usize {
    let mut matched_tokens = Vec::<&str>::new();
    for token in query_tokens {
        let matches_path = bctx
            .path_segments
            .iter()
            .any(|segment| segment.contains(token.as_str()))
            || bctx
                .file_stem
                .as_ref()
                .is_some_and(|stem| stem.contains(token.as_str()));
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
    if path_role(&bctx.path_lower) == PathRole::PrimarySource
        && !is_header_like_path(&bctx.path_lower)
    {
        0.0
    } else {
        0.3
    }
}

fn is_header_like_path(path: &str) -> bool {
    path.ends_with(".h") || path.ends_with(".hpp") || path.ends_with(".hh")
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

    use serial_test::serial;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
    use crate::indexer::{enhance_workspace_hash, index_workspace};
    use crate::workspace::{Workspace, WorkspaceScope};

    use super::*;

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
            ("python sort list descending", QueryIntent::Mixed, false),
        ];
        for (query, expected_intent, expected_neural) in cases {
            let routing = QueryRouting::classify(query);
            assert_eq!(routing.intent, expected_intent, "{query}");
            assert_eq!(routing.use_neural, expected_neural, "{query}");
        }
    }

    #[test]
    fn corpus_candidate_budgets_scale_at_stable_boundaries() {
        assert_eq!(corpus_candidate_multiplier(50_000), 1);
        assert_eq!(corpus_candidate_multiplier(50_001), 2);
        assert_eq!(corpus_candidate_multiplier(500_000), 2);
        assert_eq!(corpus_candidate_multiplier(500_001), 3);
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
                progress_tx: None,
                cancel_token: None,
            },
        )
        .unwrap();

        assert!(!hits.is_empty());
        assert!(hits[0].sources.iter().any(|source| source == "lexical"));
        assert!(hits[0].sources.iter().any(|source| source == "semantic"));

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
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
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
        assert!(hits[0].preview.contains("calculate_tax"));
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
    fn search_uses_hash_vectors_until_neural_vectors_are_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

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
                .any(|hit| hit.sources.iter().any(|source| source == "semantic")),
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
            b_hit.unwrap().sources.iter().any(|s| s == "semantic"),
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
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = literal_search(&workspace, "calculate_tax", &SearchOptions::default()).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].preview.contains("calculate_tax"));
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
    fn literal_pass_runs_only_for_exactish_queries() {
        assert!(should_run_literal_pass("calculate tax"));
        assert!(should_run_literal_pass("calculate_tax_for_region"));
        assert!(should_run_literal_pass("KernelMemoryAllocation"));
        assert!(!should_run_literal_pass("kernel memory allocation"));
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
            vector_key: 0,
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
            vector_key: 0,
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
        let without_symbols = fuse_rrf(
            FusionCandidates {
                lexical: vec![(usage.clone(), 1.0), (definition.clone(), 1.0)],
                semantic: vec![],
                literal: vec![],
                path: vec![],
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
                symbols: vec![(definition, 1.0)],
            },
            1.0,
            "handle_error",
            Some(10),
        );
        unsafe { std::env::remove_var("IVYGREP_RERANK_LIMIT") };

        assert_eq!(without_symbols[0].0.chunk_id, "usage");
        assert_eq!(with_symbols[0].0.chunk_id, "definition");
        assert!(with_symbols[0].2.contains(&"symbol".to_string()));
    }

    #[test]
    #[serial]
    fn reranker_candidate_limit_is_configurable_and_bounded() {
        unsafe { std::env::set_var("IVYGREP_RERANK_LIMIT", "7") };
        assert_eq!(rerank_candidate_limit(), 7);
        unsafe { std::env::set_var("IVYGREP_RERANK_LIMIT", "0") };
        assert_eq!(rerank_candidate_limit(), 100);
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
    fn boost_context_splits_path_segments() {
        let chunk = make_test_chunk("a", "src/my-service/handler.rs", "code", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.path_segments, vec!["src", "my-service", "handler.rs"]);
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
        assert_eq!(bctx.first_line, "pub fn handle_error() {}");
    }

    #[test]
    fn boost_context_computes_compact_identifiers() {
        let chunk = make_test_chunk("a", "src/my-service.rs", "fn foo_bar() {}", "Function");
        let bctx = ChunkBoostContext::new(&chunk);
        assert_eq!(bctx.text_compact, "fnfoobar");
        assert_eq!(bctx.path_compact, "srcmyservicers");
    }

    #[test]
    fn boost_context_handles_empty_text() {
        let chunk = make_test_chunk("a", "src/empty.rs", "", "Block");
        let bctx = ChunkBoostContext::new(&chunk);
        assert!(bctx.text_lower.is_empty());
        assert!(bctx.first_line.is_empty());
        assert!(bctx.text_compact.is_empty());
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
    fn chunk_density_penalty_is_softer_for_primary_source_than_headers() {
        let source = make_test_chunk("a", "src/search.rs", "code", "Function");
        let header = make_test_chunk("b", "include/search.h", "code", "Function");

        assert!(
            chunk_density_exponent(&ChunkBoostContext::new(&source))
                < chunk_density_exponent(&ChunkBoostContext::new(&header))
        );
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
