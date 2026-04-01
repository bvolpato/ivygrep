use std::collections::{HashMap, HashSet};
use std::fs;

use anyhow::Result;
use rusqlite::Connection;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;

use crate::embedding::EmbeddingModel;
use crate::indexer::{
    IndexedChunk, fetch_chunk_by_id, fetch_chunk_by_vector_key, open_sqlite, open_tantivy_index,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::text::{singularize_token, split_identifier_segments};
use crate::vector_store::VectorStore;
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

pub fn hybrid_search(
    workspace: &Workspace,
    query_text: &str,
    embedding_model: &dyn EmbeddingModel,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let candidate_limit = options.limit.unwrap_or(500).max(100);
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    let (index, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut parser = QueryParser::for_index(&index, vec![fields.text, fields.file_path]);
    parser.set_field_boost(fields.file_path, 2.0);

    let sqlite = open_sqlite(&workspace.sqlite_path())?;

    let mut lexical_by_id = HashMap::<String, (IndexedChunk, f32)>::new();
    for lexical_query in build_lexical_queries(query_text) {
        let parsed_query = match parser.parse_query(&lexical_query) {
            Ok(query) => query,
            Err(_) => continue,
        };
        let lexical_docs = searcher.search(&parsed_query, &TopDocs::with_limit(candidate_limit))?;

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
    let mut lexical_chunks = lexical_by_id.into_values().collect::<Vec<_>>();
    lexical_chunks.sort_by(|a, b| b.1.total_cmp(&a.1));

    // Prefer neural vector store if available, otherwise fall back to hash.
    let neural_path = workspace.vector_neural_path();
    let (vector_index, query_vector) = if neural_path.exists() {
        // Neural store exists — use neural model for query embedding.
        // The neural model may be the same as embedding_model if ONNX was
        // passed, or we need to create one. Since we can't easily create a
        // separate model here, we use the provided one and check dims match.
        let neural_dims = 384; // AllMiniLML6V2Q output
        if embedding_model.dimensions() == neural_dims {
            let vi = VectorStore::open(&neural_path, neural_dims)?;
            let qv = embedding_model.embed(query_text);
            (vi, qv)
        } else {
            // Fall back to hash vectors — model dimension mismatch
            let vi = VectorStore::open(&workspace.vector_path(), embedding_model.dimensions())?;
            let qv = embedding_model.embed(query_text);
            (vi, qv)
        }
    } else {
        let vi = VectorStore::open(&workspace.vector_path(), embedding_model.dimensions())?;
        let qv = embedding_model.embed(query_text);
        (vi, qv)
    };

    let mut semantic_chunks = Vec::new();
    if vector_index.size() > 0 {
        let matches = vector_index.search(&query_vector, candidate_limit);
        for vector_match in matches {
            if let Some(chunk) = fetch_chunk_by_vector_key(&sqlite, vector_match.key)?
                .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
                .filter(|chunk| scope_matches(chunk, options.scope_filter.as_ref()))
                .filter(|chunk| path_matches(chunk, &path_matcher))
            {
                semantic_chunks.push((chunk, vector_match.score));
            }
        }
    }

    let merged = fuse_rrf(&lexical_chunks, &semantic_chunks, query_text, options.limit);
    let hits = merged
        .into_iter()
        .map(|(chunk, score, sources)| {
            to_hit(
                workspace,
                chunk,
                query_text,
                score,
                sources,
                options.context,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(hits)
}

fn to_hit(
    workspace: &Workspace,
    chunk: IndexedChunk,
    query_text: &str,
    score: f32,
    sources: Vec<String>,
    context_lines: usize,
) -> Result<SearchHit> {
    let file_path = workspace.root.join(&chunk.file_path);
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(_) => {
            // File was deleted since indexing — fall back to stored chunk text
            return Ok(SearchHit {
                file_path: chunk.file_path,
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                preview: chunk.text,
                reason: "file no longer on disk".to_string(),
                score,
                sources,
            });
        }
    };

    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(SearchHit {
            file_path: chunk.file_path,
            start_line: chunk.start_line,
            end_line: chunk.start_line,
            preview: String::new(),
            reason: "empty file".to_string(),
            score,
            sources,
        });
    }

    let focus_line = find_focus_line(&chunk, query_text, &lines);
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
        file_path: chunk.file_path,
        start_line: snippet_start,
        end_line: snippet_end,
        preview,
        reason,
        score,
        sources,
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
    const SEMANTIC_ONLY_PENALTY: f32 = 0.82;

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

    for (rank, (chunk, _semantic_score)) in semantic.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += SEMANTIC_WEIGHT / (K + rank as f32 + 1.0);
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
    if ranked.is_empty() {
        return vec![];
    }

    let best_score = ranked[0].1;
    let threshold = (best_score * 0.60).max(0.015);

    let mut filtered = ranked
        .iter()
        .filter(|(_, score, _)| *score >= threshold)
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
    let conn: Connection = open_sqlite(&workspace.sqlite_path())?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
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
            &model,
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

        let hits =
            hybrid_search(&workspace, "applyFilter", &model, &SearchOptions::default()).unwrap();

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

        let hits =
            hybrid_search(&workspace, "applyFilter", &model, &SearchOptions::default()).unwrap();
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
            &model,
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
            &model,
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
}
