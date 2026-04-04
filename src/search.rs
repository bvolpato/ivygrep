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
pub struct SearchOptions {
    pub limit: Option<usize>,
    pub context: usize,
    pub type_filter: Option<String>,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub scope_filter: Option<WorkspaceScope>,
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
        }
    }
}

/// Fast index-backed literal text search.
///
/// Uses Tantivy to find chunks containing the literal terms, then scans
/// only those chunks' source lines for exact case-insensitive substring
/// matches. This is O(matched_chunks) instead of O(all_files), making it
/// orders of magnitude faster than `--regex` for exact string searches.
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

    let (index, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    // Use Tantivy QueryParser to find chunks whose text contains the query terms.
    // This narrows the search space from all files to only matching chunks.
    let mut parser = QueryParser::for_index(&index, vec![fields.text, fields.file_path]);
    parser.set_field_boost(fields.file_path, 2.0);

    // Build lexical queries from the literal text
    let lexical_queries = build_lexical_queries(query);
    let mut candidate_chunks = Vec::new();
    let mut seen_ids = HashSet::new();
    let candidate_limit = max_hits.max(100) * 5; // fetch more candidates than needed

    for lexical_query in &lexical_queries {
        let parsed = match parser.parse_query(lexical_query) {
            Ok(q) => q,
            Err(_) => continue,
        };

        let top_docs = searcher.search(
            &parsed,
            &TopDocs::with_limit(candidate_limit).order_by_score(),
        )?;

        for (_score, addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(addr)?;
            if let Some(chunk) = fetch_chunk_by_id(doc, &fields)
                .filter(|c| type_matches(c, options.type_filter.as_deref()))
                .filter(|c| scope_matches(c, options.scope_filter.as_ref()))
                .filter(|c| path_matches(c, &path_matcher))
                .filter(|c| seen_ids.insert(c.chunk_id.clone()))
            {
                candidate_chunks.push(chunk);
            }
        }
    }
    tracing::trace!(
        "literal_tantivy={:?} candidates={}",
        t0.elapsed(),
        candidate_chunks.len()
    );

    // Now scan only the candidate chunks' source lines for exact literal matches.
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

pub fn hybrid_search(
    workspace: &Workspace,
    query_text: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let t0 = std::time::Instant::now();
    let candidate_limit = options.limit.unwrap_or(500).max(100);
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    let (index, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    tracing::trace!("open_tantivy={:?}", t0.elapsed());

    let mut parser = QueryParser::for_index(&index, vec![fields.text, fields.file_path]);
    parser.set_field_boost(fields.file_path, 2.0);

    let sqlite = open_sqlite_readonly(&workspace.sqlite_path())?;

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
                let term = tantivy::Term::from_field_text(fields.language, lang);
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

        let lexical_docs = searcher.search(
            &parsed_query,
            &TopDocs::with_limit(candidate_limit).order_by_score(),
        )?;

        for (score, addr) in lexical_docs {
            let doc: TantivyDocument = searcher.doc(addr)?;
            if let Some(chunk) = fetch_chunk_by_id(doc, &fields)
                .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
                .filter(|chunk| scope_matches(chunk, options.scope_filter.as_ref()))
                .filter(|chunk| path_matches(chunk, &path_matcher))
            {
                lexical_by_id
                    .entry(chunk.chunk_id.clone())
                    .and_modify(|(_, best)| *best = best.max(score))
                    .or_insert((chunk, score));
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
        if let Ok(Some(full)) = fetch_chunk_by_vector_key(&sqlite, vector_key) {
            if let Some((chunk, _)) = lexical_by_id.get_mut(&chunk_id) {
                chunk.text = full.text;
            }
        }
    }

    let mut lexical_chunks = lexical_by_id.into_values().collect::<Vec<_>>();
    lexical_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
    tracing::trace!("lexical={:?} found={}", t0.elapsed(), lexical_chunks.len());

    let mut vector_index_opt = None;
    if let Some(model) = embedding_model {
        let neural_path = workspace.vector_neural_path();
        vector_index_opt = Some(if neural_path.exists() {
            let neural_dims = 384; // AllMiniLML6V2Q output
            if model.dimensions() == neural_dims {
                VectorStore::open_readonly(&neural_path, neural_dims, ScalarKind::F32)?
            } else {
                VectorStore::open_readonly(&workspace.vector_path(), model.dimensions(), ScalarKind::F16)?
            }
        } else {
            VectorStore::open_readonly(&workspace.vector_path(), model.dimensions(), ScalarKind::F16)?
        });
        tracing::trace!(
            "open_vector={:?} size={}",
            t0.elapsed(),
            vector_index_opt.as_ref().unwrap().size()
        );
    }

    // When glob filters or scope filters are active, pre-collect the set of
    // vector_keys that match, so we can skip the expensive full-corpus vector
    // search and only fetch relevant candidates from SQLite.
    let has_filters = !options.include_globs.is_empty()
        || !options.exclude_globs.is_empty()
        || options.scope_filter.is_some()
        || options.type_filter.is_some();

    let mut semantic_chunks = Vec::new();
    if let (Some(model), Some(vector_index)) = (embedding_model, vector_index_opt) {
        if vector_index.size() > 0 {
            // Only now do we pay the cost of embedding the query
            let query_vector = model.embed(query_text);

            if has_filters {
                // Pre-filtered path: query SQLite for matching chunks first,
                // then only look up their embeddings. This turns a 2.3M scan
                // into a few thousand lookups for targeted queries like --include '*.yaml'.
                let filtered_chunks = collect_filtered_chunks(
                    &sqlite,
                    &path_matcher,
                    options.scope_filter.as_ref(),
                    options.type_filter.as_deref(),
                    &options.include_globs,
                );
                // Score each filtered chunk against the query vector
                for chunk in filtered_chunks {
                    if let Some(score) = vector_index.score(chunk.vector_key, &query_vector) {
                        semantic_chunks.push((chunk, score));
                    }
                }
                // Sort by score descending, keep top candidates
                semantic_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));
                semantic_chunks.truncate(candidate_limit);
            } else {
                // Unfiltered path: standard ANN search over entire corpus
                let matches = vector_index.search(&query_vector, candidate_limit);
                for vector_match in matches {
                    if let Some(chunk) = fetch_chunk_by_vector_key(&sqlite, vector_match.key)? {
                        semantic_chunks.push((chunk, vector_match.score));
                    }
                }
            }
        }
    }
    tracing::trace!(
        "semantic={:?} found={}",
        t0.elapsed(),
        semantic_chunks.len()
    );

    let merged = fuse_rrf(&lexical_chunks, &semantic_chunks, query_text, options.limit);
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
    let query_tokens = tokenize_query(query);

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

        for token in tokenize_query(query) {
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
    }

    queries.sort();
    queries.dedup();
    queries
}

fn tokenize_query(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        for segment in split_identifier_segments(raw) {
            let singular = singularize_token(&segment);
            if singular.len() >= 2 {
                tokens.push(singular);
            }
        }
    }

    tokens.sort();
    tokens.dedup();
    tokens
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

/// Pre-collect chunks from SQLite that match glob/scope/type filters.
/// Used to avoid full-corpus vector scan when targeted filters are set.
fn collect_filtered_chunks(
    conn: &Connection,
    path_matcher: &PathGlobMatcher,
    scope_filter: Option<&WorkspaceScope>,
    type_filter: Option<&str>,
    include_globs: &[String],
) -> Vec<IndexedChunk> {
    // Build a SQL query that pushes as much filtering as possible into SQLite.
    let mut sql = String::from(
        "SELECT chunk_id, file_path, start_line, end_line, language, kind, text, content_hash, vector_key FROM chunks WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

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
        Ok(IndexedChunk {
            chunk_id: row.get(0)?,
            file_path: PathBuf::from(row.get::<_, String>(1)?),
            start_line: row.get::<_, i64>(2)? as usize,
            end_line: row.get::<_, i64>(3)? as usize,
            language: row.get(4)?,
            kind: row.get(5)?,
            text: crate::indexer::decompress_text(raw_text),
            content_hash: row.get(7)?,
            vector_key: row.get::<_, i64>(8)? as u64,
        })
    }) else {
        return Vec::new();
    };

    // Apply full glob filtering in Rust for complex patterns
    rows.flatten()
        .filter(|chunk| path_matcher.matches(&chunk.file_path))
        .collect()
}

fn fuse_rrf(
    lexical: &[(IndexedChunk, f32)],
    semantic: &[(IndexedChunk, f32)],
    query_text: &str,
    limit: Option<usize>,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    const K: f32 = 60.0;
    const LEXICAL_WEIGHT: f32 = 3.2;
    const SEMANTIC_WEIGHT: f32 = 1.0;
    const LEXICAL_SCORE_WEIGHT: f32 = 0.05;
    const SEMANTIC_SCORE_WEIGHT: f32 = 0.08;
    const SEMANTIC_ONLY_PENALTY: f32 = 0.82;
    const TERM_COVERAGE_WEIGHT: f32 = 0.12;
    const PATH_SEGMENT_WEIGHT: f32 = 0.08;

    let query_tokens = tokenize_query(query_text);

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

    let mut ranked = scores
        .into_iter()
        .filter_map(|(id, base_score)| {
            let chunk = chunks.remove(&id)?;
            let source_set = sources.remove(&id).unwrap_or_default();
            let mut source_list = source_set.iter().cloned().collect::<Vec<_>>();
            source_list.sort();

            let mut score = base_score + literal_match_boost(query_text, &chunk);

            // Term-coverage bonus: reward chunks that match more query tokens
            if !query_tokens.is_empty() {
                score += term_coverage_boost(&query_tokens, &chunk) * TERM_COVERAGE_WEIGHT;
            }

            // File-path segment matching: boost files whose path contains query tokens
            if !query_tokens.is_empty() {
                score += path_segment_boost(&query_tokens, &chunk) * PATH_SEGMENT_WEIGHT;
            }

            if !source_set.contains("lexical") {
                score *= SEMANTIC_ONLY_PENALTY;
            }

            Some((chunk, score, source_list))
        })
        .collect::<Vec<_>>();

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
        .filter(|(_, score, _)| *score >= adaptive_threshold)
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

fn literal_match_boost(query_text: &str, chunk: &IndexedChunk) -> f32 {
    const CASE_SENSITIVE_BOOST: f32 = 0.20;
    const CASE_INSENSITIVE_BOOST: f32 = 0.14;
    const NORMALIZED_IDENTIFIER_BOOST: f32 = 0.10;

    let query = query_text.trim();
    if query.is_empty() {
        return 0.0;
    }

    let file_path = chunk.file_path.to_string_lossy();
    if chunk.text.contains(query) || file_path.contains(query) {
        return CASE_SENSITIVE_BOOST;
    }

    let query_lower = query.to_ascii_lowercase();
    let text_lower = chunk.text.to_ascii_lowercase();
    let path_lower = file_path.to_ascii_lowercase();
    if text_lower.contains(&query_lower) || path_lower.contains(&query_lower) {
        return CASE_INSENSITIVE_BOOST;
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
}
