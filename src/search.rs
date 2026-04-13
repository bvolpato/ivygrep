use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use rusqlite::Connection;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;

use crate::embedding::EmbeddingModel;
use crate::indexer::{
    IndexedChunk, fetch_chunk_by_id, fetch_chunk_by_vector_key, open_sqlite_readonly,
    open_tantivy_index,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::text::{singularize_token, split_identifier_segments};
use crate::vector_store::{ScalarKind, VectorStore};
use crate::workspace::{Workspace, WorkspaceScope};

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
        }
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

    pub tombstones: HashSet<String>,
    pub overlay_files: HashSet<String>,
}

impl SearchContext {
    pub fn load(workspace: &Workspace, _emb_dim: Option<usize>) -> Result<Self> {
        let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
        if use_overlay {
            let overlay_sqlite = open_sqlite_readonly(&workspace.overlay_sqlite_path())?;
            let (overlay_idx, fields) = open_tantivy_index(&workspace.overlay_tantivy_dir())?;
            let overlay_reader = overlay_idx.reader()?;
            let overlay_searcher = overlay_reader.searcher();
            let overlay_hash_vec =
                VectorStore::open_readonly(&workspace.overlay_vector_path(), 256, ScalarKind::F16)
                    .ok();

            let base_dir = workspace
                .base_index_dir
                .clone()
                .unwrap_or_else(|| workspace.index_dir.clone());
            let base_sqlite = open_sqlite_readonly(&base_dir.join("metadata.sqlite3"))?;
            let (base_idx, _) = open_tantivy_index(&base_dir.join("tantivy"))?;
            let base_reader = base_idx.reader()?;
            let base_searcher = base_reader.searcher();
            let base_hash_vec =
                VectorStore::open_readonly(&base_dir.join("vectors.usearch"), 256, ScalarKind::F16)
                    .ok();
            let base_neural_vec = VectorStore::open_readonly(
                &base_dir.join("vectors_neural.usearch"),
                384,
                ScalarKind::F32,
            )
            .ok();

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
                tombstones,
                overlay_files,
            })
        } else {
            let sqlite = open_sqlite_readonly(&workspace.sqlite_path())?;
            let (idx, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
            let reader = idx.reader()?;
            let searcher = reader.searcher();
            let hash_vec =
                VectorStore::open_readonly(&workspace.vector_path(), 256, ScalarKind::F16).ok();
            let neural_vec =
                VectorStore::open_readonly(&workspace.vector_neural_path(), 384, ScalarKind::F32)
                    .ok();

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
    let t0 = std::time::Instant::now();
    let query = query_text.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }

    let query_lower = query.to_ascii_lowercase();
    let max_hits = options.limit.unwrap_or(500);
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    let ctx = SearchContext::load(workspace, None)?;

    let matcher = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()?;

    // Use Tantivy index as a pre-filter: find candidate chunk IDs via the
    // inverted index, then only decompress those to verify the exact match.
    let candidate_chunks =
        collect_literal_candidates(&ctx, query, &matcher, &path_matcher, options)?;

    tracing::trace!(
        "literal_scan={:?} candidates={}",
        t0.elapsed(),
        candidate_chunks.len()
    );

    // Now scan only the candidate chunks' source lines for precise matches and snippet extraction.
    // Group by file to read each file only once.
    let mut chunks_by_file: HashMap<PathBuf, Vec<IndexedChunk>> = HashMap::new();
    for chunk in candidate_chunks {
        chunks_by_file
            .entry(workspace.root.join(&chunk.file_path))
            .or_default()
            .push(chunk);
    }

    let mut hits = Vec::new();
    'outer: for (file_path, chunks) in &chunks_by_file {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        for chunk in chunks {
            // Scan lines within this chunk's range for the literal text
            let start = chunk.start_line.saturating_sub(1);
            let end = chunk.end_line.min(lines.len());

            for (i, line) in lines[start..end].iter().enumerate() {
                let line_num = start + i + 1;
                if line.to_ascii_lowercase().contains(&query_lower) {
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
    options: &SearchOptions,
) -> Result<Vec<IndexedChunk>> {
    let candidate_limit = if let Some(limit) = options.limit {
        if limit == usize::MAX {
            10_000_000
        } else {
            limit.max(500)
        }
    } else {
        500
    };

    let mut search_fields = vec![ctx.fields.text, ctx.fields.file_path];
    if let Some(f) = ctx.fields.file_path_text {
        search_fields.push(f);
    }
    if let Some(f) = ctx.fields.signature {
        search_fields.push(f);
    }
    let parser = QueryParser::for_index(&ctx.indexes[0], search_fields);

    let mut found_ids = HashSet::<String>::new();
    let mut verified = Vec::<IndexedChunk>::new();

    for lexical_query in build_lexical_queries(query) {
        let parsed_query = match parser.parse_query(&lexical_query) {
            Ok(q) => q,
            Err(_) => continue,
        };

        for (i, searcher) in ctx.searchers.iter().enumerate() {
            let docs = searcher.search(
                &parsed_query,
                &TopDocs::with_limit(candidate_limit).order_by_score(),
            )?;

            for (_score, addr) in docs {
                let doc: TantivyDocument = searcher.doc(addr)?;
                if let Some(mut chunk) = fetch_chunk_by_id(doc, &ctx.fields)
                    .filter(|c| !ctx.is_shadowed_base_file(i, &c.file_path))
                    .filter(|c| type_matches(c, options.type_filter.as_deref()))
                    .filter(|c| scope_matches(c, options.scope_filter.as_ref()))
                    .filter(|c| path_matches(c, path_matcher))
                    .filter(|c| options.skip_gitignore || !c.is_ignored)
                {
                    if found_ids.contains(&chunk.chunk_id) {
                        continue;
                    }
                    if chunk.text.is_empty()
                        && let Ok(Some(full)) = ctx.fetch_chunk_by_vector_key(chunk.vector_key)
                    {
                        chunk.text = full.text;
                    }
                    if matcher.is_match(&chunk.text) {
                        found_ids.insert(chunk.chunk_id.clone());
                        verified.push(chunk);
                    }
                }
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
    let t0 = std::time::Instant::now();
    let candidate_limit = if let Some(limit) = options.limit {
        if limit == usize::MAX {
            10_000_000
        } else {
            limit.max(500)
        }
    } else {
        500
    };
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    let ctx = SearchContext::load(workspace, embedding_model.map(|m| m.dimensions()))?;
    tracing::trace!("open_tantivy={:?}", t0.elapsed());

    // ── Literal pass ────────────────────────────────────────────────────
    // Always run a fast index-backed literal substring scan so exact matches
    // surface even when tokenization splits them differently.
    // Build a regex alternation of the original query plus snake_case/camelCase
    // variants so "hybrid search" also matches "hybrid_search" and "hybridSearch".
    let trimmed = query_text.trim();
    let literal_variants = build_lexical_queries(trimmed);
    let literal_pattern = literal_variants
        .iter()
        .map(|v| regex::escape(v))
        .collect::<Vec<_>>()
        .join("|");
    let literal_matcher = regex::RegexBuilder::new(&literal_pattern)
        .case_insensitive(true)
        .build()
        .ok();
    let literal_chunks: Vec<(IndexedChunk, f32)> = if let Some(ref matcher) = literal_matcher
        && !trimmed.is_empty()
    {
        let mut all_candidates = Vec::new();
        for variant in &literal_variants {
            let variant_matcher = regex::RegexBuilder::new(&regex::escape(variant))
                .case_insensitive(true)
                .build();
            if let Ok(ref vm) = variant_matcher
                && let Ok(candidates) =
                    collect_literal_candidates(&ctx, variant, vm, &path_matcher, options)
            {
                all_candidates.extend(candidates);
            }
        }
        // Deduplicate by chunk_id
        let mut seen = std::collections::HashSet::new();
        all_candidates.retain(|c| seen.insert(c.chunk_id.clone()));
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

    let mut allowed_languages = Vec::new();
    let mut can_pushdown_languages = options.include_globs.is_empty();
    if let Some(tf) = &options.type_filter {
        allowed_languages.push(tf.to_string());
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
    for lexical_query in build_lexical_queries(query_text) {
        let mut parsed_query = match parser.parse_query(&lexical_query) {
            Ok(query) => query,
            Err(_) => continue,
        };

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
    // Populate text from SQLite for chunks where Tantivy doesn't store it.
    let need_text: Vec<(String, u64)> = lexical_by_id
        .iter()
        .filter(|(_, (chunk, _))| chunk.text.is_empty())
        .map(|(id, (chunk, _))| (id.clone(), chunk.vector_key))
        .collect();
    for (chunk_id, vector_key) in need_text {
        if let Ok(Some(full)) = ctx.fetch_chunk_by_vector_key(vector_key)
            && let Some((chunk, _)) = lexical_by_id.get_mut(&chunk_id)
        {
            chunk.text = full.text;
        }
    }

    let mut lexical_chunks = lexical_by_id.into_values().collect::<Vec<_>>();
    lexical_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
    tracing::trace!("lexical={:?} found={}", t0.elapsed(), lexical_chunks.len());

    tracing::trace!("open_vector={:?}", t0.elapsed());

    let mut semantic_chunks = Vec::new();
    let has_hash_vectors = ctx.hash_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_hash_vectors.as_ref().map_or(0, |v| v.size()) > 0;
    let has_neural_vectors = ctx.neural_vectors.as_ref().map_or(0, |v| v.size()) > 0
        || ctx.base_neural_vectors.as_ref().map_or(0, |v| v.size()) > 0;

    if embedding_model.is_some() && (has_hash_vectors || has_neural_vectors) {
        let mut semantic_by_id = HashMap::<String, (IndexedChunk, f32)>::new();

        if has_hash_vectors {
            let hash_model = crate::embedding::HashEmbeddingModel::new(256);
            let hash_query_vector = hash_model.embed(query_text);
            let hash_hits = collect_semantic_candidates(
                &ctx,
                &path_matcher,
                options,
                &hash_query_vector,
                candidate_limit,
                ctx.hash_vectors.as_ref(),
                ctx.base_hash_vectors.as_ref(),
            )?;
            merge_semantic_candidates(&mut semantic_by_id, hash_hits, 1.0);
        }

        if let Some(model) = embedding_model
            && model.dimensions() == 384
            && has_neural_vectors
        {
            let neural_query_vector = model.embed(query_text);
            let neural_hits = collect_semantic_candidates(
                &ctx,
                &path_matcher,
                options,
                &neural_query_vector,
                candidate_limit,
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

    let merged = fuse_rrf(
        &lexical_chunks,
        &semantic_chunks,
        &literal_chunks,
        query_text,
        options.limit,
    );
    tracing::trace!("fuse_rrf={:?} merged={}", t0.elapsed(), merged.len());

    // Group hits by file path so we read each file only once
    let mut hits_by_file: HashMap<PathBuf, Vec<(IndexedChunk, f32, Vec<String>)>> = HashMap::new();
    for (chunk, score, sources) in &merged {
        hits_by_file
            .entry(workspace.root.join(&chunk.file_path))
            .or_default()
            .push((chunk.clone(), *score, sources.clone()));
    }

    let mut hits = Vec::with_capacity(merged.len());
    for (file_path, file_hits) in &hits_by_file {
        let file_content = fs::read_to_string(file_path).ok();
        for (chunk, score, sources) in file_hits {
            hits.push(to_hit(
                workspace,
                chunk,
                query_text,
                *score,
                sources,
                options.context,
                file_content.as_deref(),
            )?);
        }
    }
    // Re-sort since grouping by file changed the order
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    tracing::trace!(
        "to_hit={:?} hits={} files_read={}",
        t0.elapsed(),
        hits.len(),
        hits_by_file.len()
    );

    Ok(hits)
}

fn to_hit(
    workspace: &Workspace,
    chunk: &IndexedChunk,
    query_text: &str,
    score: f32,
    sources: &[String],
    context_lines: usize,
    pre_read_content: Option<&str>,
) -> Result<SearchHit> {
    let content = match pre_read_content {
        Some(c) => c.to_string(),
        None => {
            let file_path = workspace.root.join(&chunk.file_path);
            match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => {
                    return Ok(SearchHit {
                        file_path: chunk.file_path.clone(),
                        start_line: chunk.start_line,
                        end_line: chunk.end_line,
                        preview: chunk.text.clone(),
                        reason: "file no longer on disk".to_string(),
                        score,
                        sources: sources.to_vec(),
                    });
                }
            }
        }
    };

    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(SearchHit {
            file_path: chunk.file_path.clone(),
            start_line: chunk.start_line,
            end_line: chunk.start_line,
            preview: String::new(),
            reason: "empty file".to_string(),
            score,
            sources: sources.to_vec(),
        });
    }

    let focus_line = find_focus_line(chunk, query_text, &lines);
    let (snippet_start, snippet_end) = snippet_bounds(focus_line, context_lines, lines.len());
    let preview = lines[snippet_start.saturating_sub(1)..snippet_end].join("\n");
    let reason = summarize_reason(
        query_text,
        lines
            .get(focus_line.saturating_sub(1))
            .copied()
            .unwrap_or_default(),
    );

    Ok(SearchHit {
        file_path: chunk.file_path.clone(),
        start_line: snippet_start,
        end_line: snippet_end,
        preview,
        reason,
        score,
        sources: sources.to_vec(),
    })
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
        if focus.contains(query)
            || focus
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
        {
            return format!("line contains query terms: {}", truncate_for_reason(focus));
        }

        for token in expanded_query_tokens(query) {
            if focus.to_ascii_lowercase().contains(&token) {
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

    for token in expanded_query_tokens(query) {
        if !normalized_tokens.contains(&token) {
            queries.push(token);
        }
    }

    queries.sort();
    queries.dedup();
    queries
}

fn tokenize_query(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut seen = HashSet::new();
    for raw in query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        for segment in split_identifier_segments(raw) {
            let normalized = singularize_token(&segment.to_ascii_lowercase());
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
        for alias in query_token_aliases(token) {
            let alias = alias.to_string();
            if alias.len() >= 2 && seen.insert(alias.clone()) {
                expanded.push(alias);
            }
        }
    }

    for alias in query_phrase_aliases(&primary) {
        let alias = alias.to_string();
        if alias.len() >= 2 && seen.insert(alias.clone()) {
            expanded.push(alias);
        }
    }

    expanded
}

fn query_token_aliases(token: &str) -> &'static [&'static str] {
    match token {
        "implemented" | "implementing" | "implementation" | "implements" => &["implement"],
        "defined" | "defining" | "definition" | "definitions" | "declared" | "declaration" => {
            &["define"]
        }
        "ranked" | "ranking" => &["rank"],
        "results" => &["result"],
        "chunking" => &["chunk"],
        "flags" | "flag" => &["cli", "arg", "option"],
        "arguments" | "argument" | "args" | "arg" => &["cli", "flag", "option"],
        "command" => &["cli"],
        "parsing" => &["parse", "parser"],
        "matching" => &["match", "matcher"],
        "detection" | "detected" | "detecting" => &["detect", "detector"],
        "counting" | "counts" => &["count", "counter"],
        "output" => &["print", "printer"],
        "colored" | "coloring" => &["color"],
        "walker" => &["walk"],
        _ => &[],
    }
}

fn query_phrase_aliases(tokens: &[String]) -> Vec<&'static str> {
    let has = |needle: &str| tokens.iter().any(|token| token == needle);
    let mut aliases = Vec::new();

    if has("command") && has("line") {
        aliases.push("cli");
    }
    if has("output") && has("format") {
        aliases.push("printer");
    }
    if has("result") && has("output") {
        aliases.push("printer");
    }
    if has("line") && has("number") {
        aliases.push("line_number");
    }

    aliases
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
        Some(filter) => chunk.language.eq_ignore_ascii_case(filter),
        None => true,
    }
}

fn scope_matches(chunk: &IndexedChunk, scope_filter: Option<&WorkspaceScope>) -> bool {
    match scope_filter {
        Some(scope) => scope.matches(&chunk.file_path),
        None => true,
    }
}

fn path_matches(chunk: &IndexedChunk, path_matcher: &PathGlobMatcher) -> bool {
    path_matcher.matches(&chunk.file_path)
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
        "SELECT chunk_id, file_path, start_line, end_line, language, kind, text, content_hash, vector_key, is_ignored FROM chunks WHERE 1=1",
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
        let prefix = scope.rel_path.to_string_lossy().to_string();
        if scope.is_file {
            sql.push_str(" AND file_path = ?");
            params_vec.push(Box::new(prefix));
        } else {
            sql.push_str(" AND file_path LIKE ?");
            params_vec.push(Box::new(format!("{}%", prefix)));
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
    let mut semantic_chunks = Vec::new();
    let has_filters = !options.include_globs.is_empty()
        || !options.exclude_globs.is_empty()
        || options.scope_filter.is_some()
        || options.type_filter.is_some();

    if has_filters {
        let filtered_chunks = collect_filtered_chunks(
            ctx,
            path_matcher,
            options.scope_filter.as_ref(),
            options.type_filter.as_deref(),
            &options.include_globs,
            options.skip_gitignore,
        );

        let mut semantic_raw = Vec::new();
        for chunk in filtered_chunks {
            let mut score =
                primary_store.and_then(|store| store.score(chunk.vector_key, query_vector));
            if score.is_none() {
                score = base_store.and_then(|store| store.score(chunk.vector_key, query_vector));
            }
            if let Some(score) = score {
                semantic_raw.push((chunk, score));
            }
        }
        semantic_raw.sort_by(|a, b| b.1.total_cmp(&a.1));
        semantic_raw.truncate(candidate_limit);

        for (raw_chunk, score) in semantic_raw {
            semantic_chunks.push((raw_chunk.decompress(), score));
        }
    } else {
        let mut matches = Vec::new();
        if let Some(store) = primary_store {
            matches.extend(store.search(query_vector, candidate_limit));
        }
        if let Some(store) = base_store {
            matches.extend(store.search(query_vector, candidate_limit));
        }
        matches.sort_by(|a, b| b.score.total_cmp(&a.score));
        matches.truncate(candidate_limit);

        for vector_match in matches {
            if let Some(chunk) = ctx.fetch_chunk_by_vector_key(vector_match.key)?
                && (options.skip_gitignore || !chunk.is_ignored)
            {
                semantic_chunks.push((chunk, vector_match.score));
            }
        }
    }

    Ok(semantic_chunks)
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

fn fuse_rrf(
    lexical: &[(IndexedChunk, f32)],
    semantic: &[(IndexedChunk, f32)],
    literal: &[(IndexedChunk, f32)],
    query_text: &str,
    limit: Option<usize>,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    const K: f32 = 60.0;
    const LEXICAL_WEIGHT: f32 = 3.2;
    const SEMANTIC_WEIGHT: f32 = 1.0;
    const LITERAL_WEIGHT: f32 = 4.0;
    const LEXICAL_SCORE_WEIGHT: f32 = 0.05;
    const SEMANTIC_SCORE_WEIGHT: f32 = 0.08;
    const SEMANTIC_ONLY_PENALTY: f32 = 0.60;
    const TERM_COVERAGE_WEIGHT: f32 = 0.35;
    const PATH_SEGMENT_WEIGHT: f32 = 0.20;
    const FILE_STEM_WEIGHT: f32 = 0.30;
    const DEFINITION_NAME_BONUS: f32 = 0.25;
    const LOCATION_INTENT_WEIGHT: f32 = 0.20;

    let query_tokens = expanded_query_tokens(query_text);
    let location_intent = has_location_intent(query_text);

    let mut scores = HashMap::<String, f32>::new();
    let mut chunks = HashMap::<String, IndexedChunk>::new();
    let mut sources = HashMap::<String, HashSet<String>>::new();

    for (rank, (chunk, lexical_score)) in lexical.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += LEXICAL_WEIGHT / (K + rank as f32 + 1.0);
        *entry += normalize_lexical_score(*lexical_score) * LEXICAL_SCORE_WEIGHT;
        chunks
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| chunk.clone());
        sources
            .entry(chunk.chunk_id.clone())
            .or_default()
            .insert("lexical".to_string());
    }

    for (rank, (chunk, semantic_score)) in semantic.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += SEMANTIC_WEIGHT / (K + rank as f32 + 1.0);
        // Use the actual cosine similarity score, not just rank position
        *entry += normalize_semantic_score(*semantic_score) * SEMANTIC_SCORE_WEIGHT;
        chunks
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| chunk.clone());
        sources
            .entry(chunk.chunk_id.clone())
            .or_default()
            .insert("semantic".to_string());
    }

    // Literal pass: verified exact substring matches get a strong boost
    for (rank, (chunk, _)) in literal.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += LITERAL_WEIGHT / (K + rank as f32 + 1.0);
        chunks
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| chunk.clone());
        sources
            .entry(chunk.chunk_id.clone())
            .or_default()
            .insert("literal".to_string());
    }

    // Chunk-density normalization (IDF-like):
    // Count how many candidate chunks each file contributes. Files with many
    // chunks (large data files, verbose test suites) get a 1/sqrt(n) penalty
    // so they can't dominate the results just by having more "lottery tickets".
    let mut file_chunk_counts: HashMap<PathBuf, usize> = HashMap::new();
    for chunk in chunks.values() {
        *file_chunk_counts
            .entry(chunk.file_path.clone())
            .or_insert(0) += 1;
    }

    let mut ranked = scores
        .into_iter()
        .filter_map(|(id, base_score)| {
            let chunk = chunks.remove(&id)?;
            let source_set = sources.remove(&id).unwrap_or_default();
            let mut source_list = source_set.iter().cloned().collect::<Vec<_>>();
            source_list.sort();

            let mut score = base_score + literal_match_boost(query_text, &chunk);

            let coverage = if !query_tokens.is_empty() {
                term_coverage_boost(&query_tokens, &chunk)
            } else {
                0.0
            };
            score += coverage * TERM_COVERAGE_WEIGHT;

            if !query_tokens.is_empty() {
                score += path_segment_boost(&query_tokens, &chunk) * PATH_SEGMENT_WEIGHT;
            }

            if !query_tokens.is_empty() {
                score += file_stem_boost(&query_tokens, &chunk) * FILE_STEM_WEIGHT;
            }

            if !query_tokens.is_empty() {
                score += definition_name_boost(&query_tokens, &chunk) * DEFINITION_NAME_BONUS;
            }

            if location_intent {
                score += location_intent_boost(&chunk) * LOCATION_INTENT_WEIGHT;
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
            score *= file_authority_score(&chunk);

            // Apply chunk-density normalization: 1/n^0.3 where n is the number
            // of chunks this file has in the candidate set. Single-chunk files
            // (focused modules) are unaffected; a file with 25 chunks gets ~0.3x.
            let n_file_chunks = file_chunk_counts
                .get(&chunk.file_path)
                .copied()
                .unwrap_or(1) as f32;
            score /= n_file_chunks.powf(0.3);

            Some((chunk, score, source_list))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    // Per-file hit diversity cap: keep at most 2 hits per file at full score,
    // then aggressively decay. This prevents any single file from hogging the
    // top results even after density normalization.
    let mut file_hit_counts: HashMap<PathBuf, usize> = HashMap::new();
    for item in &mut ranked {
        let count = file_hit_counts.entry(item.0.file_path.clone()).or_insert(0);
        *count += 1;
        match *count {
            1..=2 => {}
            3..=4 => item.1 *= 0.4,
            _ => item.1 *= 0.1,
        }
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    let mut filtered = filter_meaningful_scores(&ranked);

    if let Some(limit) = limit {
        filtered.truncate(limit);
    }

    filtered
}

fn filter_meaningful_scores(
    ranked: &[(IndexedChunk, f32, Vec<String>)],
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    if ranked.len() <= 1 {
        return ranked.to_vec();
    }

    let best_score = ranked[0].1;

    // Adaptive threshold: use mean - 1 standard deviation of the score
    // distribution, but clamp to reasonable bounds.
    let scores: Vec<f32> = ranked.iter().map(|(_, s, _)| *s).collect();
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / scores.len() as f32;
    let stddev = variance.sqrt();
    let adaptive_threshold = (mean - stddev).max(best_score * 0.35).max(0.010);

    let mut filtered = ranked
        .iter()
        .filter(|(_, score, sources)| {
            *score >= adaptive_threshold || sources.contains(&"literal".to_string())
        })
        .cloned()
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        filtered.push(ranked[0].clone());
    }

    filtered
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

/// Fraction of query tokens that appear (case-insensitive) in the chunk text.
fn term_coverage_boost(query_tokens: &[String], chunk: &IndexedChunk) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let text_lower = chunk.text.to_ascii_lowercase();
    let matched = query_tokens
        .iter()
        .filter(|t| text_lower.contains(t.as_str()))
        .count();
    matched as f32 / query_tokens.len() as f32
}

/// Boost when query tokens match file-path segments (directory/filename).
fn path_segment_boost(query_tokens: &[String], chunk: &IndexedChunk) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let path_lower = chunk.file_path.to_string_lossy().to_ascii_lowercase();
    let segments: Vec<&str> = path_lower.split('/').collect();
    let matched = query_tokens
        .iter()
        .filter(|t| segments.iter().any(|seg| seg.contains(t.as_str())))
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn file_stem_boost(query_tokens: &[String], chunk: &IndexedChunk) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let Some(stem) = chunk
        .file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_ascii_lowercase())
    else {
        return 0.0;
    };

    let compact_stem = compact_identifier(&stem);
    let exact_match = query_tokens
        .iter()
        .any(|token| stem == *token || compact_stem == compact_identifier(token));
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

fn location_intent_boost(chunk: &IndexedChunk) -> f32 {
    let mut boost: f32 = 0.0;
    let path = chunk.file_path.to_string_lossy().to_ascii_lowercase();

    if is_definition_kind(&chunk.kind) {
        boost += 0.7;
    }
    if matches!(chunk.kind.as_str(), "Module" | "module") {
        boost += 0.5;
    }
    if path.starts_with("src/")
        || path.starts_with("app/")
        || path.starts_with("lib/")
        || path.starts_with("pkg/")
    {
        boost += 0.35;
    }
    if is_test_path(&path) {
        boost -= 0.35;
    }

    boost.max(0.0)
}

/// Bonus when a chunk's definition name (first non-blank line) contains query tokens.
/// This is the "are we looking at the definition site?" signal — e.g., query "handle error"
/// should strongly prefer `fn handle_error()` over a comment mentioning errors.
fn definition_name_boost(query_tokens: &[String], chunk: &IndexedChunk) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    // Extract the first meaningful line (often the fn/class signature)
    let first_line = chunk
        .text
        .lines()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with('#')
        })
        .unwrap_or_default()
        .to_ascii_lowercase();

    if first_line.is_empty() {
        return 0.0;
    }

    let matched = query_tokens
        .iter()
        .filter(|t| first_line.contains(t.as_str()))
        .count();
    matched as f32 / query_tokens.len() as f32
}

fn literal_match_boost(query_text: &str, chunk: &IndexedChunk) -> f32 {
    const LITERAL_MATCH_BOOST: f32 = 0.20;
    const NORMALIZED_IDENTIFIER_BOOST: f32 = 0.10;

    let query = query_text.trim();
    if query.is_empty() {
        return 0.0;
    }

    let file_path = chunk.file_path.to_string_lossy();
    let query_lower = query.to_ascii_lowercase();
    let text_lower = chunk.text.to_ascii_lowercase();
    let path_lower = file_path.to_ascii_lowercase();

    if text_lower.contains(&query_lower) || path_lower.contains(&query_lower) {
        return LITERAL_MATCH_BOOST;
    }

    let query_compact = compact_identifier(query);
    if query_compact.is_empty() {
        return 0.0;
    }

    let text_compact = compact_identifier(&chunk.text);
    let path_compact = compact_identifier(&file_path);
    if text_compact.contains(&query_compact) || path_compact.contains(&query_compact) {
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

/// File authority scoring inspired by PageRank: files that are "core" source code
/// are more authoritative than tests, fixtures, generated code, data files, and
/// vendored dependencies. The scoring range is deliberately wide (0.3–1.3) to
/// create meaningful separation in the final ranking.
fn file_authority_score(chunk: &IndexedChunk) -> f32 {
    let path = chunk.file_path.to_string_lossy().to_ascii_lowercase();

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

    // Generated / snapshot files
    if path.contains("generated/")
        || path.contains("__snapshots__/")
        || path.contains("fixtures/")
        || path.contains("testdata/")
        || path.contains("test_data/")
    {
        return 0.35;
    }

    // Data / config files — they match many terms but are rarely the answer
    if path.ends_with(".json")
        || path.ends_with(".csv")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".xml")
        || path.ends_with(".toml")
        || path.ends_with(".ini")
        || path.ends_with(".env")
        || path.ends_with(".sql")
    {
        return 0.4;
    }

    // Test / spec / mock files — useful but secondary to the implementation
    if is_test_path(&path) {
        return 0.6;
    }

    // Documentation — helpful but not code
    if path.ends_with(".md") || path.ends_with(".txt") || path.ends_with(".rst") {
        return 0.5;
    }

    // Core source code gets a small boost to positively separate it
    1.0
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
        || path.contains("/__mocks__/")
        || path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("spec/")
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
        || path.starts_with("test_")  // Python: test_handler.py (at root)
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
    use serial_test::serial;

    use crate::EMBEDDING_DIMENSIONS;
    use crate::embedding::HashEmbeddingModel;
    use crate::indexer::index_workspace;
    use crate::workspace::{Workspace, WorkspaceScope};

    use super::*;

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
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("scoped")).unwrap();
        std::fs::create_dir_all(tmp.path().join("other")).unwrap();
        std::fs::write(
            tmp.path().join("scoped/match.rs"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("other/match.rs"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

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
                    rel_path: std::path::PathBuf::from("scoped"),
                    is_file: false,
                }),
                skip_gitignore: false,
            },
        )
        .unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits.iter()
                .all(|hit| hit.file_path.starts_with(std::path::Path::new("scoped")))
        );
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
        assert!(expanded.iter().any(|token| token == "arg"));
        assert!(expanded.iter().any(|token| token == "option"));

        let lexical = build_lexical_queries("command line flags");
        assert!(lexical.iter().any(|query| query == "cli"));
    }

    #[test]
    fn query_expansion_adds_printer_alias_for_output_format() {
        let expanded = expanded_query_tokens("search results output format");
        assert!(expanded.iter().any(|token| token == "printer"));

        let lexical = build_lexical_queries("search results output format");
        assert!(lexical.iter().any(|query| query == "printer"));
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
}
