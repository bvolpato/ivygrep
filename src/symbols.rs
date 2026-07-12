use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use rusqlite::{Connection, params, types::ToSql};

use crate::indexer::{
    IndexedChunk, decompress_text, open_sqlite_readonly, reconcile_worktree_overlay,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::search::SearchOptions;
use crate::workspace::{Workspace, WorkspaceScope};

const SYMBOL_DEFINITION_LOOKUP_BATCH: usize = 128;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SymbolSearchMode {
    Definitions,
    References,
    Callers,
}

pub fn index_chunk_definition(
    conn: &Connection,
    chunk: &IndexedChunk,
    chunk_key: i64,
) -> Result<()> {
    let mut rows = Vec::new();
    append_chunk_definition_rows(chunk, chunk_key, &mut rows);
    let mut stmt = conn.prepare_cached(
        "INSERT OR REPLACE INTO symbols (
            normalized_name, chunk_key
         ) VALUES (?1, ?2)",
    )?;
    for (normalized_name, chunk_key) in rows {
        stmt.execute(params![normalized_name, chunk_key])?;
    }
    Ok(())
}

pub(crate) fn append_chunk_definition_rows(
    chunk: &IndexedChunk,
    chunk_key: i64,
    rows: &mut Vec<(String, i64)>,
) {
    for name in definition_names(chunk) {
        rows.push((normalize_symbol(&name), chunk_key));
    }
}

pub fn remove_file_graph(conn: &Connection, file_path: &str) -> Result<()> {
    let mut chunks = Vec::new();
    {
        let mut stmt = conn.prepare_cached(
            "SELECT chunk_key, start_line, end_line, language, kind, text, vector_key, is_ignored
             FROM chunks
             WHERE file_path = ?1",
        )?;
        let rows = stmt.query_map([file_path], |row| {
            let raw: Vec<u8> = row.get(5)?;
            Ok((
                row.get::<_, i64>(0)?,
                IndexedChunk {
                    chunk_id: String::new(),
                    file_path: PathBuf::from(file_path),
                    start_line: row.get::<_, i64>(1)? as usize,
                    end_line: row.get::<_, i64>(2)? as usize,
                    language: row.get(3)?,
                    kind: row.get(4)?,
                    text: decompress_text(raw),
                    content_hash: String::new(),
                    vector_key: row.get::<_, i64>(6)? as u64,
                    is_ignored: row.get(7)?,
                },
            ))
        })?;
        for row in rows {
            chunks.push(row?);
        }
    }

    let mut delete = conn.prepare_cached(
        "DELETE FROM symbols
         WHERE normalized_name = ?1 AND chunk_key = ?2",
    )?;
    for (chunk_key, chunk) in chunks {
        for name in definition_names(&chunk) {
            delete.execute(params![normalize_symbol(&name), chunk_key])?;
        }
    }
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
    let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    reconcile_worktree_overlay(workspace, &model)?;

    let candidate_name = canonical_symbol(name);
    let normalized = normalize_symbol(candidate_name);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    if mode != SymbolSearchMode::Definitions {
        return search_call_sites(workspace, candidate_name, &normalized, mode, options);
    }

    let primary_sqlite = if workspace.has_overlay() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    let mut hits = query_workspace_db(
        &open_sqlite_readonly(&primary_sqlite)?,
        &normalized,
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
            for hit in query_workspace_db(&base, &normalized, &base_options, &path_matcher)? {
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
    if limit == 0 || names.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen_normalized = HashSet::new();
    let mut requested = Vec::new();
    for name in names {
        let normalized = normalize_symbol(name);
        if normalized.is_empty() || !seen_normalized.insert(normalized.clone()) {
            continue;
        }
        requested.push((normalized, name.as_str()));
    }
    if requested.is_empty() {
        return Ok(Vec::new());
    }

    let mut by_name = (0..requested.len())
        .map(|_| Vec::new())
        .collect::<Vec<Vec<(bool, bool, IndexedChunk)>>>();
    let mut per_name_seen = (0..requested.len())
        .map(|_| HashSet::new())
        .collect::<Vec<HashSet<u64>>>();
    let candidate_limit = if limit > 256 {
        limit
    } else {
        limit.saturating_mul(8).min(256)
    };
    let candidate_limit_i64 = candidate_limit as i64;

    for (batch_index, batch) in requested.chunks(SYMBOL_DEFINITION_LOOKUP_BATCH).enumerate() {
        let base_ordinal = batch_index * SYMBOL_DEFINITION_LOOKUP_BATCH;
        let values = (0..batch.len())
            .map(|index| format!("(?{}, {index})", index + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "WITH requested(name, ordinal) AS (VALUES {values}),
                  ranked AS (
                    SELECT r.ordinal,
                           c.file_path, c.start_line, c.end_line, c.language,
                           c.kind, c.text, c.vector_key, c.is_ignored,
                           row_number() OVER (
                             PARTITION BY r.ordinal
                             ORDER BY c.file_path, c.start_line
                           ) AS rn
                    FROM requested r
                    JOIN symbols s ON s.normalized_name = r.name
                    JOIN chunks c ON c.chunk_key = s.chunk_key
                  )
             SELECT ordinal, file_path, start_line, end_line, language,
                    kind, text, vector_key, is_ignored
             FROM ranked
             WHERE rn <= ?{}
             ORDER BY ordinal, file_path, start_line",
            batch.len() + 1
        );
        let mut params: Vec<&dyn ToSql> = batch
            .iter()
            .map(|(normalized, _)| normalized as &dyn ToSql)
            .collect();
        params.push(&candidate_limit_i64);

        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| {
            let raw: Vec<u8> = row.get(6)?;
            let file_path = PathBuf::from(row.get::<_, String>(1)?);
            let start_line = row.get::<_, i64>(2)? as usize;
            let end_line = row.get::<_, i64>(3)? as usize;
            let language = row.get::<_, String>(4)?;
            let kind = row.get::<_, String>(5)?;
            let vector_key = row.get::<_, i64>(7)? as u64;
            Ok((
                base_ordinal + row.get::<_, i64>(0)? as usize,
                IndexedChunk {
                    chunk_id: String::new(),
                    file_path,
                    start_line,
                    end_line,
                    language,
                    kind,
                    text: decompress_text(raw),
                    content_hash: String::new(),
                    vector_key,
                    is_ignored: row.get(8)?,
                },
            ))
        })?;

        for row in rows {
            let (ordinal, chunk) = row?;
            if ordinal >= by_name.len() {
                continue;
            }
            if !per_name_seen[ordinal].insert(chunk.vector_key) {
                continue;
            }
            let name = requested[ordinal].1;
            let exact_case = chunk_defines_exact_name(&chunk, name);
            let canonical_file = file_stem_matches_symbol(&chunk, name);
            by_name[ordinal].push((exact_case, canonical_file, chunk));
        }
    }

    let mut seen_chunks = HashSet::new();
    let mut chunks = Vec::new();
    for name_candidates in &mut by_name {
        let remaining = limit.saturating_sub(chunks.len());
        if remaining == 0 {
            break;
        }
        name_candidates.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.file_path.cmp(&right.2.file_path))
                .then_with(|| left.2.start_line.cmp(&right.2.start_line))
        });
        for (_, _, chunk) in name_candidates.drain(..).take(remaining) {
            if seen_chunks.insert(chunk.vector_key) {
                chunks.push(chunk);
            }
        }
    }
    Ok(chunks)
}

fn query_workspace_db(
    conn: &Connection,
    normalized: &str,
    options: &SearchOptions,
    path_matcher: &PathGlobMatcher,
) -> Result<Vec<SearchHit>> {
    let sql = "SELECT c.file_path, c.start_line, c.end_line, c.text,
                      c.language, c.is_ignored
               FROM symbols s JOIN chunks c ON c.chunk_key = s.chunk_key
               WHERE s.normalized_name = ?1
               ORDER BY c.file_path, c.start_line";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([normalized], |row| {
        let raw: Vec<u8> = row.get(3)?;
        Ok((
            SearchHit {
                file_path: PathBuf::from(row.get::<_, String>(0)?),
                start_line: row.get::<_, i64>(1)? as usize,
                end_line: row.get::<_, i64>(2)? as usize,
                preview: decompress_text(raw),
                reason: "exact symbol match".to_string(),
                score: 10.0,
                sources: vec!["symbol".to_string()],
                neural_requested: false,
                neural_executed: false,
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

fn search_call_sites(
    workspace: &Workspace,
    name: &str,
    normalized: &str,
    mode: SymbolSearchMode,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let source = match mode {
        SymbolSearchMode::References => "reference",
        SymbolSearchMode::Callers => "caller",
        SymbolSearchMode::Definitions => unreachable!(),
    };
    let score = if mode == SymbolSearchMode::Callers {
        8.0
    } else {
        6.0
    };
    let mut candidate_options = options.clone();
    candidate_options.limit = options.limit.map(|limit| limit.saturating_mul(4));
    let query = format!("{}(", name.trim());
    let candidates = if options.limit.is_some() {
        crate::search::exact_literal_chunks(workspace, &query, &candidate_options)?
    } else {
        crate::search::exact_literal_chunks_unbounded(workspace, &query, &candidate_options)?
    };
    let mut hits = Vec::new();
    let mut seen_call_sites = HashSet::new();
    let mut chunks_by_file = BTreeMap::<PathBuf, Vec<IndexedChunk>>::new();
    for chunk in candidates {
        chunks_by_file
            .entry(chunk.file_path.clone())
            .or_default()
            .push(chunk);
    }
    for (file_path, mut chunks) in chunks_by_file {
        let Ok(text) = fs::read_to_string(workspace.root.join(&file_path)) else {
            continue;
        };
        chunks.sort_by_key(|chunk| {
            (
                chunk.end_line.saturating_sub(chunk.start_line),
                chunk.start_line,
            )
        });
        for chunk in chunks {
            let call_lines =
                matching_call_lines(&text, normalized, chunk.start_line, chunk.end_line)
                    .into_iter()
                    .filter(|(line, _)| seen_call_sites.insert((file_path.clone(), *line)))
                    .collect::<Vec<_>>();
            if call_lines.is_empty() {
                continue;
            }
            if mode == SymbolSearchMode::Callers {
                hits.push(SearchHit {
                    file_path: chunk.file_path,
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    preview: chunk.text,
                    reason: format!("exact {source} match"),
                    score,
                    sources: vec![source.to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
            } else {
                for (line, preview) in call_lines {
                    hits.push(SearchHit {
                        file_path: chunk.file_path.clone(),
                        start_line: line,
                        end_line: line,
                        preview,
                        reason: format!("exact {source} match"),
                        score,
                        sources: vec![source.to_string()],
                        neural_requested: false,
                        neural_executed: false,
                    });
                }
            }
            if options.limit.is_some_and(|limit| hits.len() >= limit) {
                hits.truncate(options.limit.unwrap_or(hits.len()));
                return Ok(hits);
            }
        }
    }
    Ok(hits)
}

fn matching_call_lines(
    text: &str,
    normalized: &str,
    start_line: usize,
    end_line: usize,
) -> Vec<(usize, String)> {
    let mut matches = Vec::new();
    let needle = format!("{normalized}(");
    for (offset, line) in text
        .lines()
        .enumerate()
        .skip(start_line.saturating_sub(1))
        .take(end_line.saturating_sub(start_line).saturating_add(1))
    {
        let lower = line.to_ascii_lowercase();
        let mut from = 0;
        while let Some(relative) = lower[from..].find(&needle) {
            let index = from + relative;
            let boundary_ok = index == 0
                || !lower.as_bytes()[index - 1].is_ascii_alphanumeric()
                    && lower.as_bytes()[index - 1] != b'_';
            if boundary_ok && !looks_like_definition(&lower, index) {
                matches.push((offset + 1, line.trim().to_string()));
                break;
            }
            from = index + needle.len();
        }
    }
    matches
}

fn looks_like_definition(line: &str, name_offset: usize) -> bool {
    let prefix = line[..name_offset].trim_end();
    if prefix.contains(['{', ';', '=']) {
        return false;
    }
    let has_definition_keyword = prefix
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|part| matches!(part, "fn" | "def" | "func" | "function"));
    if has_definition_keyword {
        return true;
    }

    let suffix = &line[name_offset..];
    suffix
        .find(')')
        .map(|close| suffix[close + 1..].trim_start())
        .is_some_and(|after| {
            after.starts_with('{')
                || after.starts_with("->")
                || after.starts_with(':')
                || after.starts_with("throws ")
        })
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
            | "Enum"
            | "enum"
            | "Module"
            | "module"
    ) {
        return None;
    }
    if chunk.language.eq_ignore_ascii_case("haskell")
        && matches!(chunk.kind.as_str(), "Class" | "class")
        && let Some(name) = haskell_class_definition_name(&chunk.text)
    {
        return Some(name);
    }

    let signature = first_definition_signature(&chunk.text)?;

    let keywords: &[&str] = match chunk.kind.as_str() {
        "Function" | "function" => &["fn", "def", "func", "function", "fun"],
        "Class" | "class" | "Struct" | "struct" | "Trait" | "trait" | "Interface" | "interface"
        | "Enum" | "enum" => &[
            "class",
            "struct",
            "trait",
            "enum",
            "interface",
            "type",
            "typealias",
            "union",
        ],
        "Module" | "module" => &["module"],
        _ => &[],
    };
    let allow_function_fallback = matches!(chunk.kind.as_str(), "Function" | "function");
    let require_type_alias_assignment = chunk.language.eq_ignore_ascii_case("typescript");
    if chunk.language.eq_ignore_ascii_case("zig")
        && matches!(
            chunk.kind.as_str(),
            "Class" | "class" | "Struct" | "struct" | "Enum" | "enum"
        )
    {
        return definition_name_from_signature(signature, &["const", "var"], false, false);
    }
    definition_name_from_signature(
        signature,
        keywords,
        allow_function_fallback,
        require_type_alias_assignment,
    )
}

fn first_definition_signature(text: &str) -> Option<&str> {
    let mut in_block_comment = false;
    for raw_line in text.lines() {
        let line = raw_line.trim();
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
        {
            continue;
        }
        return Some(line);
    }
    None
}

fn definition_names(chunk: &IndexedChunk) -> Vec<String> {
    let mut seen = HashSet::new();
    let is_module = matches!(chunk.kind.as_str(), "Module" | "module");
    let mut names = Vec::new();
    if chunk.language.eq_ignore_ascii_case("elixir") && is_module {
        // Keep the declared module name ahead of case-colliding nested
        // functions such as Ecto.Query's `query/6`.
        names.extend(elixir_module_definition_names(&chunk.text));
    }
    if is_module {
        if chunk.language.eq_ignore_ascii_case("haskell") {
            names.extend(haskell_module_definition_names(&chunk.text));
        } else {
            const MODULE_KEYWORDS: &[&str] = &[
                "fn",
                "def",
                "func",
                "function",
                "class",
                "struct",
                "trait",
                "enum",
                "interface",
                "type",
                "union",
                "module",
            ];
            names.extend(
                chunk
                    .text
                    .lines()
                    .filter_map(|line| {
                        let signature = line.trim();
                        if signature.is_empty()
                            || signature.starts_with("//")
                            || signature.starts_with('#')
                            || signature.starts_with('@')
                        {
                            return None;
                        }
                        definition_name_from_signature(signature, MODULE_KEYWORDS, false, true)
                    })
                    .collect::<Vec<_>>(),
            );
        }
    } else {
        names.extend(definition_name(chunk));
    }
    names.extend(public_reexport_names(chunk));
    names
        .into_iter()
        .filter(|name| seen.insert(normalize_symbol(name)))
        .collect()
}

pub(crate) fn chunk_defines_exact_name(chunk: &IndexedChunk, name: &str) -> bool {
    exact_name_namespace_depth(chunk, name).is_some()
}

pub(crate) fn exact_name_namespace_depth(chunk: &IndexedChunk, name: &str) -> Option<usize> {
    if chunk.language.eq_ignore_ascii_case("elixir")
        && matches!(chunk.kind.as_str(), "Module" | "module")
    {
        let module_names = elixir_module_definition_names(&chunk.text);
        if let Some(depth) = module_names
            .iter()
            .filter(|definition| definition.contains('.'))
            .filter(|definition| {
                definition.as_str() == name
                    || definition
                        .rsplit('.')
                        .next()
                        .is_some_and(|leaf| leaf == name)
            })
            .map(|definition| definition.split('.').count())
            .min()
        {
            return Some(depth);
        }
        if module_names.iter().any(|definition| definition == name) {
            return Some(1);
        }
    }
    definition_names(chunk)
        .iter()
        .any(|definition| definition == name)
        .then_some(1)
}

fn elixir_module_definition_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines().map(str::trim) {
        let mut tokens = line.split_whitespace();
        let Some(keyword) = tokens.next() else {
            continue;
        };
        if !matches!(keyword, "defmodule" | "defprotocol" | "defimpl") {
            continue;
        }
        let Some(raw_name) = tokens.next() else {
            continue;
        };
        let full_name = raw_name.trim_end_matches(',');
        if full_name.is_empty()
            || !full_name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_')
            })
        {
            continue;
        }
        names.push(full_name.to_string());
        if let Some(leaf) = full_name.rsplit('.').next()
            && leaf != full_name
        {
            names.push(leaf.to_string());
        }
    }
    names
}

fn haskell_module_definition_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("--") || line.starts_with("{-") {
            continue;
        }
        let candidate = line.trim_start_matches([',', '(']).trim_start();
        let Some(rest) = candidate.strip_prefix("module ") else {
            continue;
        };
        let full_name = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_')
            })
            .collect::<String>();
        if full_name.is_empty() {
            continue;
        }
        names.push(full_name.clone());
        if let Some(leaf) = full_name.rsplit('.').next()
            && leaf != full_name
        {
            names.push(leaf.to_string());
        }
    }
    names
}

fn haskell_class_definition_name(text: &str) -> Option<String> {
    let mut declaration = String::new();
    let mut collecting = false;
    for line in text.lines().map(str::trim) {
        if !collecting {
            let Some(rest) = line.strip_prefix("class ") else {
                continue;
            };
            declaration.push_str(rest);
            collecting = true;
        } else {
            declaration.push(' ');
            declaration.push_str(line);
        }
        if line == "where" || line.ends_with(" where") {
            break;
        }
    }
    if declaration.is_empty() {
        return None;
    }
    let head = declaration
        .rsplit_once("=>")
        .map(|(_, head)| head)
        .unwrap_or(&declaration)
        .trim();
    let candidate = head.split_whitespace().next()?;
    let name = identifier_prefix(candidate);
    (!name.is_empty()).then(|| name.to_string())
}

fn public_reexport_names(chunk: &IndexedChunk) -> Vec<String> {
    let mut names = Vec::new();

    if chunk.language.eq_ignore_ascii_case("rust") {
        for statement in chunk.text.split(';') {
            let Some(offset) = statement.rfind("pub use ") else {
                continue;
            };
            names.extend(exported_names_from_clause(
                &statement[offset + "pub use ".len()..],
            ));
        }
    }

    if matches!(
        chunk.language.to_ascii_lowercase().as_str(),
        "javascript" | "typescript" | "tsx" | "jsx"
    ) {
        for statement in chunk.text.split(';') {
            let Some(offset) = statement.rfind("export {") else {
                continue;
            };
            names.extend(exported_names_from_clause(
                &statement[offset + "export ".len()..],
            ));
        }
    }

    if chunk.language.eq_ignore_ascii_case("python")
        && chunk.file_path.file_name().and_then(|name| name.to_str()) == Some("__init__.py")
    {
        for line in chunk.text.lines().map(str::trim) {
            let Some((_, imported)) = line.split_once(" import ") else {
                continue;
            };
            if line.starts_with("from ") {
                names.extend(exported_names_from_clause(imported));
            }
        }
    }

    names
}

fn exported_names_from_clause(clause: &str) -> Vec<String> {
    let list = clause
        .split_once('{')
        .and_then(|(_, rest)| rest.rsplit_once('}').map(|(inner, _)| inner))
        .unwrap_or(clause);
    list.split(',')
        .filter_map(|item| {
            let item = item.trim().trim_matches(['{', '}', '(', ')']);
            if item.is_empty() || item == "self" || item == "*" {
                return None;
            }
            let public_name = item
                .rsplit_once(" as ")
                .map(|(_, alias)| alias)
                .unwrap_or(item)
                .rsplit("::")
                .next()?
                .trim();
            let name = public_name
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$');
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

fn definition_name_from_signature(
    signature: &str,
    keywords: &[&str],
    allow_function_fallback: bool,
    require_type_alias_assignment: bool,
) -> Option<String> {
    let tokens = signature.split_whitespace().collect::<Vec<_>>();
    for keyword in keywords {
        if *keyword == "type" && require_type_alias_assignment && !signature.contains('=') {
            continue;
        }
        if let Some(keyword_index) = tokens
            .iter()
            .position(|token| token.trim_end_matches('*') == *keyword)
        {
            let candidate = if *keyword == "fun" {
                signature
                    .split('(')
                    .next()
                    .and_then(|prefix| prefix.split_whitespace().last())
                    .and_then(|name| name.rsplit('.').next())
            } else if *keyword == "func"
                && tokens
                    .get(keyword_index + 1)
                    .is_some_and(|token| token.starts_with('('))
            {
                tokens[keyword_index + 1..]
                    .iter()
                    .position(|token| token.contains(')'))
                    .and_then(|offset| tokens.get(keyword_index + offset + 2))
                    .copied()
            } else {
                tokens.get(keyword_index + 1).copied()
            };
            let name = candidate
                .map(|candidate| identifier_prefix(candidate.trim_start_matches('*')))
                .unwrap_or_default();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    if !allow_function_fallback {
        return None;
    }
    let before_paren = signature.split('(').next()?.trim();
    let candidate = before_paren.split_whitespace().last()?;
    let candidate = identifier_prefix(candidate.trim_start_matches('*').trim_start_matches('&'));
    (!candidate.is_empty()).then(|| candidate.to_string())
}

fn identifier_prefix(value: &str) -> &str {
    let end = value
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_alphanumeric() && character != '_' && character != '$')
                .then_some(index)
        })
        .unwrap_or(value.len());
    &value[..end]
}

fn normalize_symbol(value: &str) -> String {
    canonical_symbol(value).to_ascii_lowercase()
}

fn canonical_symbol(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_')
}

fn file_stem_matches_symbol(chunk: &IndexedChunk, name: &str) -> bool {
    let symbol = name
        .rsplit([':', '\\', '.', '/', '#'])
        .find(|part| !part.is_empty())
        .unwrap_or(name);
    chunk
        .file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(symbol))
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use tempfile::tempdir;

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
        assert_eq!(
            definition_name(&chunk(
                "rust",
                "Class",
                "/// Main request router.\npub struct Router<S = ()> {"
            ))
            .as_deref(),
            Some("Router")
        );
        assert_eq!(
            definition_name(&chunk("rust", "Enum", "pub enum RouteKind {")).as_deref(),
            Some("RouteKind")
        );
        assert_eq!(
            definition_name(&chunk("rust", "Class", "impl<S> Router<S> {")),
            None,
            "implementation blocks are not canonical definitions"
        );
        assert_eq!(
            definition_name(&chunk("rust", "Impl", "impl<S> Router<S> {")),
            None,
            "legacy implementation chunks are not canonical definitions"
        );
        assert_eq!(
            definition_name(&chunk(
                "typescript",
                "Class",
                "type MiddlewareBuilder as TRPCMiddlewareBuilder,"
            )),
            None,
            "type re-exports are not canonical definitions"
        );
        assert_eq!(
            definition_name(&chunk(
                "typescript",
                "Class",
                "export type AnyRouter = Router<any, any>;"
            ))
            .as_deref(),
            Some("AnyRouter")
        );
        assert_eq!(
            definition_name(&chunk(
                "kotlin",
                "Class",
                "public typealias Channel<T> = Flow<T>"
            ))
            .as_deref(),
            Some("Channel")
        );
        assert_eq!(
            definition_name(&chunk(
                "kotlin",
                "Function",
                "private fun <T> Flow<T>.debounceInternal(timeout: Long): Flow<T> = this"
            ))
            .as_deref(),
            Some("debounceInternal")
        );
        assert_eq!(
            definition_name(&chunk(
                "kotlin",
                "Class",
                "* A cold asynchronous stream.\n */\npublic interface Flow<out T> {"
            ))
            .as_deref(),
            Some("Flow")
        );
        assert_eq!(
            definition_name(&chunk("zig", "Class", "pub const Client = struct {")).as_deref(),
            Some("Client")
        );
        assert_eq!(
            definition_name(&chunk(
                "typescript",
                "Class",
                "export interface $ZodType<Input = unknown> {"
            ))
            .as_deref(),
            Some("$ZodType")
        );
        assert_eq!(
            definition_name(&chunk(
                "haskell",
                "Class",
                "class (Functor m, Applicative m, Monad m)\n  => PandocMonad m where"
            ))
            .as_deref(),
            Some("PandocMonad")
        );
        assert_eq!(
            definition_names(&chunk(
                "elixir",
                "Module",
                "defmodule Phoenix.Channel do\n  def join(topic), do: topic\nend"
            )),
            ["Phoenix.Channel", "Channel", "join"]
        );
        let elixir_case_collision = chunk(
            "elixir",
            "Module",
            "defmodule Ecto.Query do\n  def query(meta), do: meta\nend",
        );
        assert_eq!(
            definition_names(&elixir_case_collision),
            ["Ecto.Query", "Query"]
        );
        assert!(chunk_defines_exact_name(&elixir_case_collision, "Query"));
        assert!(!chunk_defines_exact_name(&elixir_case_collision, "query"));
        assert_eq!(
            exact_name_namespace_depth(&elixir_case_collision, "Query"),
            Some(2)
        );
        assert_eq!(
            definition_names(&chunk(
                "rust",
                "Module",
                "// src/router.rs\n\npub struct Router<S = ()> {\n}\npub enum RouteKind { Static }\npub type RouteId = usize;\npub use axum_core::extract::{FromRequest, FromRequestParts};"
            )),
            [
                "Router",
                "RouteKind",
                "RouteId",
                "FromRequest",
                "FromRequestParts"
            ]
        );
        assert_eq!(
            definition_names(&chunk(
                "typescript",
                "Module",
                "export {\n  type AnyRouter as AnyTRPCRouter,\n};\nexport type AnyRouter = Router<any, any>;"
            )),
            ["AnyRouter", "AnyTRPCRouter"]
        );
    }

    #[test]
    fn haskell_modules_index_declared_and_reexported_names() {
        let module = chunk(
            "haskell",
            "Module",
            "module Text.Pandoc.Class\n  ( module Text.Pandoc.Class.PandocMonad\n  , Translations\n  ) where",
        );
        let names = definition_names(&module);
        for expected in [
            "Text.Pandoc.Class",
            "Class",
            "Text.Pandoc.Class.PandocMonad",
            "PandocMonad",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing {expected}: {names:?}"
            );
        }
        assert!(!names.iter().any(|name| name == "Text"));
    }

    #[test]
    fn sigiled_javascript_identifiers_share_the_unsigiled_lookup_key() {
        assert_eq!(identifier_prefix("$ZodType<Input>"), "$ZodType");
        assert_eq!(normalize_symbol("$ZodType"), normalize_symbol("ZodType"));
    }

    #[test]
    fn definitions_store_multiple_names_for_one_chunk() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;",
        )
        .unwrap();
        let module = chunk(
            "rust",
            "Module",
            "pub struct Router;\npub enum RouteKind { Static }",
        );
        index_chunk_definition(&conn, &module, 7).unwrap();

        let mut stmt = conn
            .prepare("SELECT normalized_name, chunk_key FROM symbols ORDER BY normalized_name")
            .unwrap();
        let stored = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            stored,
            [("routekind".to_string(), 7), ("router".to_string(), 7)]
        );
    }

    #[test]
    fn definition_candidates_prefer_exact_case_before_case_folded_names() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                chunk_key INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                text BLOB NOT NULL,
                vector_key INTEGER NOT NULL,
                is_ignored INTEGER NOT NULL
             );
             CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;",
        )
        .unwrap();

        let lower = chunk(
            "kotlin",
            "Function",
            "public fun <T> flow(block: suspend () -> T): Flow<T> = TODO()",
        );
        let upper = chunk(
            "kotlin",
            "Class",
            "public interface Flow<out T> {\n    suspend fun collect(value: T)\n}",
        );
        for (chunk_key, vector_key, path, candidate) in [
            (1_i64, 1_i64, "src/Builders.kt", &lower),
            (2_i64, 2_i64, "src/Flow.kt", &upper),
        ] {
            conn.execute(
                "INSERT INTO chunks (
                    chunk_key, file_path, start_line, end_line, language, kind,
                    text, vector_key, is_ignored
                 ) VALUES (?1, ?2, 1, 3, ?3, ?4, ?5, ?6, 0)",
                params![
                    chunk_key,
                    path,
                    candidate.language,
                    candidate.kind,
                    candidate.text.as_bytes(),
                    vector_key
                ],
            )
            .unwrap();
            index_chunk_definition(&conn, candidate, chunk_key).unwrap();
        }

        let candidates = definition_candidates(&conn, &["Flow".to_string()], 1).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].file_path, PathBuf::from("src/Flow.kt"));
    }

    #[test]
    fn definition_candidates_prefer_canonical_file_over_partial_definitions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                chunk_key INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                text BLOB NOT NULL,
                vector_key INTEGER NOT NULL,
                is_ignored INTEGER NOT NULL
             );
             CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;",
        )
        .unwrap();

        let definition = chunk("csharp", "Class", "public static partial class SqlMapper {");
        for (chunk_key, path) in [
            (1_i64, "SqlMapper.Async.cs"),
            (2_i64, "SqlMapper.CacheInfo.cs"),
            (3_i64, "SqlMapper.cs"),
        ] {
            conn.execute(
                "INSERT INTO chunks (
                    chunk_key, file_path, start_line, end_line, language, kind,
                    text, vector_key, is_ignored
                 ) VALUES (?1, ?2, 1, 3, ?3, ?4, ?5, ?1, 0)",
                params![
                    chunk_key,
                    path,
                    definition.language,
                    definition.kind,
                    definition.text.as_bytes(),
                ],
            )
            .unwrap();
            index_chunk_definition(&conn, &definition, chunk_key).unwrap();
        }

        let candidates = definition_candidates(&conn, &["SqlMapper".to_string()], 1).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].file_path, PathBuf::from("SqlMapper.cs"));
    }

    #[test]
    fn definition_candidates_handles_many_requested_names() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                chunk_key INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                text BLOB NOT NULL,
                vector_key INTEGER NOT NULL,
                is_ignored INTEGER NOT NULL
             );
             CREATE TABLE symbols (
                normalized_name TEXT NOT NULL,
                chunk_key INTEGER NOT NULL,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;",
        )
        .unwrap();

        let target = chunk("rust", "Class", "pub struct BatchTarget;");
        conn.execute(
            "INSERT INTO chunks (
                chunk_key, file_path, start_line, end_line, language, kind,
                text, vector_key, is_ignored
             ) VALUES (1, 'src/batch_target.rs', 1, 1, ?1, ?2, ?3, 1, 0)",
            params![target.language, target.kind, target.text.as_bytes()],
        )
        .unwrap();
        index_chunk_definition(&conn, &target, 1).unwrap();

        let mut names = (0..SYMBOL_DEFINITION_LOOKUP_BATCH + 10)
            .map(|index| format!("Missing{index}"))
            .collect::<Vec<_>>();
        names.push("BatchTarget".to_string());

        let candidates = definition_candidates(&conn, &names, 1).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].file_path,
            PathBuf::from("src/batch_target.rs")
        );
    }

    #[test]
    #[serial]
    fn references_are_resolved_from_the_lexical_index() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            root.path().join("lib.rs"),
            "fn parse() {}\nfn run() {\n    let one = 1;\n    let two = 2;\n    let three = 3;\n    let four = 4;\n    let five = 5;\n    parse();\n}\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&workspace, &model).unwrap();

        let hits = search_symbols(
            &workspace,
            "parse",
            SymbolSearchMode::References,
            Some(10),
            None,
        )
        .unwrap();
        assert!(
            hits.iter().any(|hit| {
                hit.file_path == std::path::Path::new("lib.rs")
                    && hit.start_line == 8
                    && hit.reason == "exact reference match"
                    && hit.preview == "parse();"
            }),
            "{hits:#?}"
        );
        assert!(
            hits.iter().all(|hit| !hit.preview.starts_with("fn parse(")),
            "definitions must not be returned as references: {hits:?}"
        );

        let callers = search_symbols(
            &workspace,
            "parse",
            SymbolSearchMode::Callers,
            Some(10),
            None,
        )
        .unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].start_line, 2);
        assert!(callers[0].preview.contains("fn run()"));
        assert!(callers[0].preview.contains("parse();"));
    }

    #[test]
    #[serial]
    fn unbounded_call_site_searches_are_not_truncated() {
        let root = tempdir().unwrap();
        let home = tempdir().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(root.path().join("definition.rs"), "fn parse() {}\n").unwrap();
        for index in 0..125 {
            std::fs::write(
                root.path().join(format!("caller_{index}.rs")),
                format!("fn caller_{index}() {{ parse(); }}\n"),
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = crate::embedding::HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        crate::indexer::index_workspace(&workspace, &model).unwrap();

        let references = search_symbols(
            &workspace,
            "parse",
            SymbolSearchMode::References,
            None,
            None,
        )
        .unwrap();
        assert_eq!(references.len(), 125);

        let callers =
            search_symbols(&workspace, "parse", SymbolSearchMode::Callers, None, None).unwrap();
        assert_eq!(callers.len(), 125);
    }
}
