use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::indexer::{IndexedChunk, decompress_text, open_sqlite_readonly};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::search::SearchOptions;
use crate::workspace::{Workspace, WorkspaceScope};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SymbolSearchMode {
    Definitions,
    References,
    Callers,
}

pub fn index_chunk_graph(conn: &Connection, chunk: &IndexedChunk) -> Result<()> {
    let definition = definition_name(chunk);
    if let Some(name) = &definition {
        conn.prepare_cached(
            "INSERT OR REPLACE INTO symbols (
                normalized_name, display_name, symbol_kind, chunk_id
             ) VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![
            normalize_symbol(name),
            name,
            chunk.kind,
            chunk.chunk_id,
        ])?;
    }

    let normalized_source = definition
        .as_deref()
        .map(normalize_symbol)
        .unwrap_or_default();
    let mut skipped_declaration = false;
    let mut seen = HashSet::new();
    let mut insert_edge = conn.prepare_cached(
        "INSERT OR IGNORE INTO symbol_edges (
            target_name, edge_kind, source_chunk_id, line
         ) VALUES (?1, 'call', ?2, ?3)",
    )?;
    for (target, line) in call_targets(chunk) {
        let normalized = normalize_symbol(&target);
        if !skipped_declaration && !normalized_source.is_empty() && normalized == normalized_source
        {
            skipped_declaration = true;
            continue;
        }
        if normalized.is_empty() || !seen.insert((normalized.clone(), line)) {
            continue;
        }
        insert_edge.execute(params![normalized, chunk.chunk_id, line as i64,])?;
    }
    Ok(())
}

pub fn remove_file_graph(conn: &Connection, file_path: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM symbol_edges
         WHERE source_chunk_id IN (SELECT chunk_id FROM chunks WHERE file_path = ?1)",
        [file_path],
    )?;
    conn.execute(
        "DELETE FROM symbols
         WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE file_path = ?1)",
        [file_path],
    )?;
    Ok(())
}

pub fn search_symbols(
    workspace: &Workspace,
    name: &str,
    mode: SymbolSearchMode,
    limit: Option<usize>,
    scope: Option<&WorkspaceScope>,
) -> Result<Vec<SearchHit>> {
    let options = SearchOptions {
        limit,
        scope_filter: scope.cloned(),
        ..SearchOptions::default()
    };
    search_symbols_with_options(workspace, name, mode, &options)
}

pub fn search_symbols_with_options(
    workspace: &Workspace,
    name: &str,
    mode: SymbolSearchMode,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let normalized = normalize_symbol(name);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    let primary_sqlite = if workspace.has_overlay() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    let mut hits = query_workspace_db(
        &open_sqlite_readonly(&primary_sqlite)?,
        &normalized,
        mode,
        options,
        &path_matcher,
    )?;

    if let Some(base_dir) = &workspace.base_index_dir {
        let tombstones = load_path_set(&workspace.overlay_sqlite_path(), "tombstones")?;
        let overlay_files = load_chunk_paths(&workspace.overlay_sqlite_path())?;
        let remaining = options.limit.map(|limit| limit.saturating_sub(hits.len()));
        if remaining != Some(0) {
            let base = open_sqlite_readonly(&base_dir.join("metadata.sqlite3"))?;
            let mut base_options = options.clone();
            base_options.limit = remaining;
            for hit in query_workspace_db(&base, &normalized, mode, &base_options, &path_matcher)? {
                let path = hit.file_path.to_string_lossy();
                if !tombstones.contains(path.as_ref()) && !overlay_files.contains(path.as_ref()) {
                    hits.push(hit);
                }
            }
        }
    }

    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    hits.dedup_by(|left, right| {
        left.file_path == right.file_path && left.start_line == right.start_line
    });
    if let Some(limit) = options.limit {
        hits.truncate(limit);
    }
    Ok(hits)
}

pub fn definition_candidates(
    conn: &Connection,
    names: &[String],
    limit: usize,
) -> Result<Vec<IndexedChunk>> {
    let mut seen_names = HashSet::new();
    let mut seen_chunks = HashSet::new();
    let mut chunks = Vec::new();
    let mut stmt = conn.prepare_cached(
        "SELECT c.chunk_id, c.file_path, c.start_line, c.end_line, c.language,
                c.kind, c.text, c.content_hash, c.vector_key, c.is_ignored
         FROM symbols s JOIN chunks c ON c.chunk_id = s.chunk_id
         WHERE s.normalized_name = ?1
         ORDER BY c.file_path, c.start_line
         LIMIT ?2",
    )?;

    for name in names {
        let normalized = normalize_symbol(name);
        if normalized.is_empty() || !seen_names.insert(normalized.clone()) {
            continue;
        }
        let remaining = limit.saturating_sub(chunks.len());
        if remaining == 0 {
            break;
        }
        let rows = stmt.query_map(params![normalized, remaining as i64], |row| {
            let raw: Vec<u8> = row.get(6)?;
            Ok(IndexedChunk {
                chunk_id: row.get(0)?,
                file_path: PathBuf::from(row.get::<_, String>(1)?),
                start_line: row.get::<_, i64>(2)? as usize,
                end_line: row.get::<_, i64>(3)? as usize,
                language: row.get(4)?,
                kind: row.get(5)?,
                text: decompress_text(raw),
                content_hash: row.get(7)?,
                vector_key: row.get::<_, i64>(8)? as u64,
                is_ignored: row.get(9)?,
            })
        })?;
        for row in rows {
            let chunk = row?;
            if seen_chunks.insert(chunk.chunk_id.clone()) {
                chunks.push(chunk);
            }
        }
    }
    Ok(chunks)
}

fn query_workspace_db(
    conn: &Connection,
    normalized: &str,
    mode: SymbolSearchMode,
    options: &SearchOptions,
    path_matcher: &PathGlobMatcher,
) -> Result<Vec<SearchHit>> {
    let (sql, source, score) = match mode {
        SymbolSearchMode::Definitions => (
            "SELECT c.file_path, c.start_line, c.end_line, c.text,
                    c.language, c.is_ignored
             FROM symbols s JOIN chunks c ON c.chunk_id = s.chunk_id
             WHERE s.normalized_name = ?1
             ORDER BY c.file_path, c.start_line",
            "symbol",
            10.0,
        ),
        SymbolSearchMode::References => (
            "SELECT c.file_path, e.line, e.line, c.text,
                    c.language, c.is_ignored
             FROM symbol_edges e JOIN chunks c ON c.chunk_id = e.source_chunk_id
             WHERE e.target_name = ?1
             ORDER BY c.file_path, e.line",
            "reference",
            6.0,
        ),
        SymbolSearchMode::Callers => (
            "SELECT c.file_path, c.start_line, c.end_line, c.text,
                    c.language, c.is_ignored
             FROM symbol_edges e JOIN chunks c ON c.chunk_id = e.source_chunk_id
             WHERE e.target_name = ?1 AND e.edge_kind = 'call'
             ORDER BY c.file_path, c.start_line",
            "caller",
            8.0,
        ),
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([normalized], |row| {
        let raw: Vec<u8> = row.get(3)?;
        Ok((
            SearchHit {
                file_path: PathBuf::from(row.get::<_, String>(0)?),
                start_line: row.get::<_, i64>(1)? as usize,
                end_line: row.get::<_, i64>(2)? as usize,
                preview: decompress_text(raw),
                reason: format!("exact {source} match"),
                score,
                sources: vec![source.to_string()],
            },
            row.get::<_, String>(4)?,
            row.get::<_, bool>(5)?,
        ))
    })?;

    let mut hits = Vec::new();
    for row in rows {
        let (hit, language, is_ignored) = row?;
        if options
            .scope_filter
            .as_ref()
            .is_none_or(|scope| scope.matches(&hit.file_path))
            && type_matches(&language, options.type_filter.as_deref())
            && path_matcher.matches(&hit.file_path)
            && (options.skip_gitignore || !is_ignored)
        {
            hits.push(hit);
            if options.limit.is_some_and(|limit| hits.len() >= limit) {
                break;
            }
        }
    }
    Ok(hits)
}

fn type_matches(language: &str, type_filter: Option<&str>) -> bool {
    type_filter.is_none_or(|filter| {
        language.eq_ignore_ascii_case(filter)
            || crate::chunking::resolve_type_alias(filter)
                .is_some_and(|canonical| language.eq_ignore_ascii_case(canonical))
    })
}

fn load_path_set(path: &std::path::Path, table: &str) -> Result<HashSet<String>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let conn = open_sqlite_readonly(path)?;
    let mut stmt = conn.prepare(&format!("SELECT file_path FROM {table}"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn load_chunk_paths(path: &std::path::Path) -> Result<HashSet<String>> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let conn = open_sqlite_readonly(path)?;
    let mut stmt = conn.prepare("SELECT DISTINCT file_path FROM chunks")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn definition_name(chunk: &IndexedChunk) -> Option<String> {
    if !matches!(
        chunk.kind.as_str(),
        "Function" | "function" | "Class" | "class" | "Module" | "module"
    ) {
        return None;
    }
    let signature = chunk
        .text
        .lines()
        .find(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("//")
                && !line.starts_with('#')
                && !line.starts_with('@')
        })?
        .trim();

    let keywords: &[&str] = match chunk.kind.as_str() {
        "Function" | "function" => &["fn", "def", "func", "function"],
        "Class" | "class" => &["class", "struct", "trait", "enum", "interface", "type"],
        "Module" | "module" => &["module"],
        _ => &[],
    };
    let tokens = signature.split_whitespace().collect::<Vec<_>>();
    for keyword in keywords {
        if let Some(keyword_index) = tokens
            .iter()
            .position(|token| token.trim_end_matches('*') == *keyword)
        {
            let candidate = if *keyword == "func"
                && tokens
                    .get(keyword_index + 1)
                    .is_some_and(|token| token.starts_with('('))
            {
                tokens[keyword_index + 1..]
                    .iter()
                    .position(|token| token.contains(')'))
                    .and_then(|offset| tokens.get(keyword_index + offset + 2))
            } else {
                tokens.get(keyword_index + 1)
            };
            let name = candidate
                .map(|candidate| identifier_prefix(candidate.trim_start_matches('*')))
                .unwrap_or_default();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    let before_paren = signature.split('(').next()?.trim();
    let candidate = before_paren.split_whitespace().last()?;
    let candidate = identifier_prefix(candidate.trim_start_matches('*').trim_start_matches('&'));
    (!candidate.is_empty()).then(|| candidate.to_string())
}

fn call_targets(chunk: &IndexedChunk) -> Vec<(String, usize)> {
    const SKIP: &[&str] = &[
        "if", "for", "while", "match", "switch", "catch", "return", "sizeof", "typeof", "function",
        "fn", "def", "func",
    ];
    let mut calls = Vec::new();
    for (offset, line) in chunk.text.lines().enumerate() {
        let bytes = line.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte != b'(' || index == 0 {
                continue;
            }
            let mut start = index;
            while start > 0 {
                let previous = bytes[start - 1];
                if previous.is_ascii_alphanumeric() || previous == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            let candidate = &line[start..index];
            if candidate.len() >= 2 && !SKIP.contains(&candidate) {
                calls.push((
                    candidate.to_string(),
                    chunk.start_line.saturating_add(offset),
                ));
            }
        }
    }
    calls
}

fn identifier_prefix(value: &str) -> &str {
    let end = value
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_alphanumeric() && character != '_').then_some(index)
        })
        .unwrap_or(value.len());
    &value[..end]
}

fn normalize_symbol(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(language: &str, kind: &str, text: &str) -> IndexedChunk {
        IndexedChunk {
            chunk_id: "chunk".to_string(),
            file_path: PathBuf::from("src/example.txt"),
            start_line: 10,
            end_line: 20,
            language: language.to_string(),
            kind: kind.to_string(),
            text: text.to_string(),
            content_hash: "hash".to_string(),
            vector_key: 1,
            is_ignored: false,
        }
    }

    #[test]
    fn extracts_common_definition_names() {
        assert_eq!(
            definition_name(&chunk("rust", "Function", "pub fn calculate_tax() {}")).as_deref(),
            Some("calculate_tax")
        );
        assert_eq!(
            definition_name(&chunk("python", "Function", "def charge_card(user):")).as_deref(),
            Some("charge_card")
        );
        assert_eq!(
            definition_name(&chunk(
                "java",
                "Function",
                "public Result execute(Request request) {"
            ))
            .as_deref(),
            Some("execute")
        );
        assert_eq!(
            definition_name(&chunk(
                "go",
                "Function",
                "func (client *Client) SendRequest(ctx context.Context) error {"
            ))
            .as_deref(),
            Some("SendRequest")
        );
    }

    #[test]
    fn extracts_distinct_call_targets() {
        let calls = call_targets(&chunk(
            "rust",
            "Function",
            "fn run() { parse(input); client.send(value); parse(other); }",
        ));
        assert!(calls.iter().any(|(name, _)| name == "parse"));
        assert!(calls.iter().any(|(name, _)| name == "send"));
    }

    #[test]
    fn declaration_is_not_persisted_as_a_self_call() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                display_name TEXT NOT NULL,
                symbol_kind TEXT NOT NULL,
                chunk_id TEXT PRIMARY KEY
             ) WITHOUT ROWID;
             CREATE TABLE symbol_edges (
                target_name TEXT NOT NULL,
                edge_kind TEXT NOT NULL,
                source_chunk_id TEXT NOT NULL,
                line INTEGER NOT NULL,
                PRIMARY KEY(target_name, edge_kind, source_chunk_id, line)
             ) WITHOUT ROWID;",
        )
        .unwrap();
        let function = chunk("rust", "Function", "fn run() { parse(); }");
        index_chunk_graph(&conn, &function).unwrap();

        let self_calls: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_edges WHERE target_name = 'run'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let parse_calls: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_edges WHERE target_name = 'parse'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(self_calls, 0);
        assert_eq!(parse_calls, 1);
    }

    #[test]
    fn distinct_reference_lines_are_preserved() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                display_name TEXT NOT NULL,
                symbol_kind TEXT NOT NULL,
                chunk_id TEXT PRIMARY KEY
             ) WITHOUT ROWID;
             CREATE TABLE symbol_edges (
                target_name TEXT NOT NULL,
                edge_kind TEXT NOT NULL,
                source_chunk_id TEXT NOT NULL,
                line INTEGER NOT NULL,
                PRIMARY KEY(target_name, edge_kind, source_chunk_id, line)
             ) WITHOUT ROWID;",
        )
        .unwrap();
        let function = chunk(
            "rust",
            "Function",
            "fn run() {\n    parse(first);\n    parse(second);\n}",
        );
        index_chunk_graph(&conn, &function).unwrap();

        let parse_calls: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbol_edges WHERE target_name = 'parse'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parse_calls, 2);
    }
}
