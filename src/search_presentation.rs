use std::fs;

use anyhow::Result;

use crate::indexer::IndexedChunk;
use crate::protocol::SearchHit;
use crate::search_routing::QueryRouting;
use crate::workspace::Workspace;

use super::{
    CachedFileContent, FusionQuery, compact_identifier, is_definition_kind, truncate_for_reason,
};

#[derive(Clone, Copy)]
pub(super) struct LineSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) struct HitPresentation<'a> {
    pub(super) context_lines: usize,
    pub(super) query: &'a PresentationQuery,
    pub(super) routing: QueryRouting,
    pub(super) neural_executed: bool,
}

pub(super) struct PresentationQuery {
    text: String,
    lower: String,
    compact: String,
    pub(super) compact_matching: bool,
    tokens: Vec<String>,
}

impl PresentationQuery {
    #[cfg(test)]
    pub(super) fn new(query_text: &str) -> Self {
        let query = FusionQuery::new(query_text);
        Self::from_fusion(&query)
    }

    pub(super) fn from_fusion(query: &FusionQuery<'_>) -> Self {
        Self {
            text: query.text.to_string(),
            lower: query.lower.clone(),
            compact: crate::text::singularize_token(&query.compact),
            compact_matching: query.compact_candidate_text,
            tokens: query.tokens.clone(),
        }
    }
}

pub(super) fn to_hit(
    workspace: &Workspace,
    chunk: IndexedChunk,
    score: f32,
    sources: Vec<String>,
    pre_read_file: Option<&CachedFileContent>,
    presentation: HitPresentation<'_>,
) -> Result<SearchHit> {
    if let Some(file) = pre_read_file {
        return Ok(to_hit_from_file(
            chunk,
            score,
            sources,
            &file.content,
            &file.lines,
            presentation,
        ));
    }

    let file_path = workspace.root.join(&chunk.file_path);
    match fs::read_to_string(&file_path) {
        Ok(content) => {
            let lines = line_spans(&content);
            Ok(to_hit_from_file(
                chunk,
                score,
                sources,
                &content,
                &lines,
                presentation,
            ))
        }
        Err(_) => Ok(SearchHit {
            file_path: chunk.file_path,
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            preview: chunk.text,
            reason: format!(
                "route={} neural_requested={} neural_executed={}; file no longer on disk",
                presentation.routing.intent.name(),
                presentation.routing.use_neural,
                presentation.neural_executed
            ),
            score,
            sources,
            neural_requested: presentation.routing.use_neural,
            neural_executed: presentation.neural_executed,
        }),
    }
}

fn to_hit_from_file(
    chunk: IndexedChunk,
    score: f32,
    sources: Vec<String>,
    content: &str,
    lines: &[LineSpan],
    presentation: HitPresentation<'_>,
) -> SearchHit {
    let HitPresentation {
        context_lines,
        query,
        routing,
        neural_executed,
    } = presentation;
    if lines.is_empty() {
        return SearchHit {
            file_path: chunk.file_path,
            start_line: chunk.start_line,
            end_line: chunk.start_line,
            preview: String::new(),
            reason: format!(
                "route={} neural_requested={} neural_executed={}; empty file",
                routing.intent.name(),
                routing.use_neural,
                neural_executed
            ),
            score,
            sources,
            neural_requested: routing.use_neural,
            neural_executed,
        };
    }

    let focus_line = find_focus_line(&chunk, query, content, lines);
    let (snippet_start, snippet_end) = snippet_bounds(focus_line, context_lines, lines.len());
    let preview = preview_from_lines(content, lines, snippet_start, snippet_end);
    let ranking_reason = summarize_reason(query, line_at(content, lines, focus_line));
    let reason = format!(
        "route={} neural_requested={} neural_executed={}; {ranking_reason}",
        routing.intent.name(),
        routing.use_neural,
        neural_executed
    );

    SearchHit {
        file_path: chunk.file_path,
        start_line: snippet_start,
        end_line: snippet_end,
        preview,
        reason,
        score,
        sources,
        neural_requested: routing.use_neural,
        neural_executed,
    }
}

pub(super) fn line_spans(content: &str) -> Vec<LineSpan> {
    let bytes = content.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let end = if index > start && bytes[index - 1] == b'\r' {
            index - 1
        } else {
            index
        };
        spans.push(LineSpan { start, end });
        start = index.saturating_add(1);
    }
    if start < bytes.len() {
        spans.push(LineSpan {
            start,
            end: bytes.len(),
        });
    }
    spans
}

pub(super) fn line_at<'a>(content: &'a str, lines: &[LineSpan], line_number: usize) -> &'a str {
    let span = lines
        .get(line_number.saturating_sub(1))
        .expect("line number must be in bounds");
    &content[span.start..span.end]
}

fn preview_from_lines(
    content: &str,
    lines: &[LineSpan],
    snippet_start: usize,
    snippet_end: usize,
) -> String {
    lines[snippet_start.saturating_sub(1)..snippet_end]
        .iter()
        .map(|span| &content[span.start..span.end])
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn find_focus_line(
    chunk: &IndexedChunk,
    query: &PresentationQuery,
    content: &str,
    lines: &[LineSpan],
) -> usize {
    let line_count = lines.len();
    let window_start = chunk.start_line.max(1).min(line_count);
    let window_end = chunk.end_line.max(window_start).min(line_count);
    if query.text.is_empty() {
        return window_start;
    }

    let symbol_name = query
        .text
        .rsplit([':', '\\', '.', '/', '#'])
        .find(|part| !part.is_empty())
        .unwrap_or(query.text.as_str());
    if is_definition_kind(&chunk.kind)
        && (crate::symbols::chunk_defines_exact_name(chunk, &query.text)
            || crate::symbols::chunk_defines_exact_name(chunk, symbol_name))
        && let Some(line) = first_source_definition_line(content, lines, window_start, window_end)
    {
        return line;
    }

    let mut best_line = window_start;
    let mut best_score = 0.0f32;
    for line_no in window_start..=window_end {
        let line = line_at(content, lines, line_no);
        let line_lower = line.to_ascii_lowercase();
        let mut line_score = 0.0f32;

        if line.contains(&query.text) {
            line_score += 8.0;
        } else if line_lower.contains(&query.lower) {
            line_score += 5.0;
        }
        for token in &query.tokens {
            if line_lower.contains(token) {
                line_score += 1.5;
            }
        }
        if query.compact_matching && !query.compact.is_empty() {
            let line_compact = compact_identifier(line);
            if line_compact.contains(&query.compact) {
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

fn first_source_definition_line(
    content: &str,
    lines: &[LineSpan],
    window_start: usize,
    window_end: usize,
) -> Option<usize> {
    let mut in_block_comment = false;
    for line_no in window_start..=window_end {
        let line = line_at(content, lines, line_no).trim();
        if in_block_comment {
            if line.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if line.starts_with("/*") {
            if !line.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with('#')
            || line.starts_with('@')
            || line.starts_with('*')
            || line.starts_with('[') && line.ends_with(']')
        {
            continue;
        }
        return Some(line_no);
    }
    None
}

pub(super) fn should_use_compact_identifier_matching(
    query_text: &str,
    primary_tokens: &[String],
) -> bool {
    primary_tokens.len() <= 2
        || query_text
            .chars()
            .any(|ch| ch == '_' || ch == '-' || ch == '/' || ch == ':' || ch.is_ascii_uppercase())
}

pub(super) fn snippet_bounds(
    focus_line: usize,
    context_lines: usize,
    line_count: usize,
) -> (usize, usize) {
    let start = focus_line.saturating_sub(context_lines).max(1);
    let end = focus_line.saturating_add(context_lines).min(line_count);
    (start, end)
}

fn summarize_reason(query: &PresentationQuery, focus_line: &str) -> String {
    let focus = focus_line.trim();
    if focus.is_empty() {
        return "top hybrid relevance in this file".to_string();
    }
    if !query.text.is_empty() {
        let focus_lower = focus.to_ascii_lowercase();
        if focus.contains(&query.text) || focus_lower.contains(&query.lower) {
            return format!("line contains query terms: {}", truncate_for_reason(focus));
        }
        for token in &query.tokens {
            if focus_lower.contains(token) {
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
