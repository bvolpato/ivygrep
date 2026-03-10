use std::collections::{HashMap, HashSet};
use std::fs;

use anyhow::{Context, Result};
use rusqlite::Connection;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;

use crate::embedding::EmbeddingModel;
use crate::indexer::{
    IndexedChunk, fetch_chunk_by_id, fetch_chunk_by_vector_key, open_sqlite, open_tantivy_index,
};
use crate::protocol::SearchHit;
use crate::vector_store::VectorStore;
use crate::workspace::Workspace;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub context: usize,
    pub type_filter: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            context: 0,
            type_filter: None,
        }
    }
}

pub fn hybrid_search(
    workspace: &Workspace,
    query_text: &str,
    embedding_model: &dyn EmbeddingModel,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let (index, fields) = open_tantivy_index(&workspace.tantivy_dir())?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut parser = QueryParser::for_index(&index, vec![fields.text, fields.file_path]);
    parser.set_field_boost(fields.file_path, 2.0);

    let parsed_query = parser.parse_query(query_text)?;
    let lexical_docs = searcher.search(&parsed_query, &TopDocs::with_limit(50))?;

    let sqlite = open_sqlite(&workspace.sqlite_path())?;

    let mut lexical_chunks = Vec::new();
    for (_score, addr) in lexical_docs {
        let doc: TantivyDocument = searcher.doc(addr)?;
        if let Some(chunk) = fetch_chunk_by_id(doc, &fields)
            .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
        {
            lexical_chunks.push(chunk);
        }
    }

    let vector_index = VectorStore::open(&workspace.vector_path(), embedding_model.dimensions())?;
    let query_vector = embedding_model.embed(query_text);

    let mut semantic_chunks = Vec::new();
    if vector_index.size() > 0 {
        let matches = vector_index.search(&query_vector, 50);
        for vector_match in matches {
            if let Some(chunk) = fetch_chunk_by_vector_key(&sqlite, vector_match.key)?
                .filter(|chunk| type_matches(chunk, options.type_filter.as_deref()))
            {
                semantic_chunks.push(chunk);
            }
        }
    }

    let merged = fuse_rrf(&lexical_chunks, &semantic_chunks, options.limit);
    let hits = merged
        .into_iter()
        .map(|(chunk, score, sources)| to_hit(workspace, chunk, score, sources, options.context))
        .collect::<Result<Vec<_>>>()?;

    Ok(hits)
}

fn to_hit(
    workspace: &Workspace,
    chunk: IndexedChunk,
    score: f32,
    sources: Vec<String>,
    context_lines: usize,
) -> Result<SearchHit> {
    let preview = if context_lines == 0 {
        chunk
            .text
            .lines()
            .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("//"))
            .unwrap_or("")
            .trim()
            .to_string()
    } else {
        let file_path = workspace.root.join(&chunk.file_path);
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("failed reading {}", file_path.display()))?;

        let lines = content.lines().collect::<Vec<_>>();
        if lines.is_empty() {
            String::new()
        } else {
            let start = chunk.start_line.saturating_sub(context_lines + 1);
            let end = (chunk.start_line + context_lines).min(lines.len());
            lines[start..end].join("\n")
        }
    };

    Ok(SearchHit {
        file_path: chunk.file_path,
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        preview,
        score,
        sources,
    })
}

fn type_matches(chunk: &IndexedChunk, type_filter: Option<&str>) -> bool {
    match type_filter {
        Some(filter) => chunk.language.eq_ignore_ascii_case(filter),
        None => true,
    }
}

fn fuse_rrf(
    lexical: &[IndexedChunk],
    semantic: &[IndexedChunk],
    limit: usize,
) -> Vec<(IndexedChunk, f32, Vec<String>)> {
    let k = 60.0f32;

    let mut scores = HashMap::<String, f32>::new();
    let mut chunks = HashMap::<String, IndexedChunk>::new();
    let mut sources = HashMap::<String, HashSet<String>>::new();

    for (rank, chunk) in lexical.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += 1.0 / (k + rank as f32 + 1.0);
        chunks
            .entry(chunk.chunk_id.clone())
            .or_insert_with(|| chunk.clone());
        sources
            .entry(chunk.chunk_id.clone())
            .or_default()
            .insert("lexical".to_string());
    }

    for (rank, chunk) in semantic.iter().enumerate() {
        let entry = scores.entry(chunk.chunk_id.clone()).or_insert(0.0);
        *entry += 1.0 / (k + rank as f32 + 1.0);
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
        .filter_map(|(id, score)| {
            let chunk = chunks.remove(&id)?;
            let mut source_list = sources
                .remove(&id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            source_list.sort();
            Some((chunk, score, source_list))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked.truncate(limit);
    ranked
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
    use crate::workspace::Workspace;

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
}
