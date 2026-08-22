use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::{Connection, params, types::ToSql};

use crate::chunking::ChunkDefinition;
use crate::indexer::{
    IndexedChunk, open_sqlite_readonly, reconcile_worktree_overlay, try_decompress_text,
};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::search::SearchOptions;
use crate::text::strip_leading_annotations;
use crate::workspace::{Workspace, WorkspaceScope};

const SYMBOL_DEFINITION_LOOKUP_BATCH: usize = 128;
/// Columns written per `symbols` row; keeps batched insert sizing in sync.
pub(crate) const SYMBOL_ROW_COLUMNS: usize = 6;

/// Owner tier used to rank qualified lookups: exact-case owner match,
/// case-insensitive owner match, or no owner match (bare-name fallback).
const OWNER_TIER_EXACT: u8 = 0;
const OWNER_TIER_FOLDED: u8 = 1;
const OWNER_TIER_NONE: u8 = 2;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SymbolSearchMode {
    Definitions,
    References,
    Callers,
}

/// One persisted `symbols` row. Language and kind live on the chunk row and
/// are joined when needed; `name` is stored only when its case differs from
/// `normalized_name`, which keeps the table close to its pre-v22 footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolRow {
    pub normalized_name: String,
    pub chunk_key: i64,
    pub name: Option<String>,
    pub owner: Option<String>,
}

impl SymbolRow {
    pub(crate) fn push_params<'a>(&'a self, params: &mut Vec<&'a dyn ToSql>) {
        params.push(&self.normalized_name);
        params.push(&self.chunk_key);
        params.push(&self.name);
        params.push(&self.owner);
    }
}

/// Display-case name column value: `None` when the name is already lowercase.
pub(crate) fn stored_symbol_name(name: &str, normalized_name: &str) -> Option<String> {
    (name != normalized_name).then(|| name.to_string())
}

/// A symbol lookup split into its bare name and optional owner qualifier
/// (`Owner.method`, `Owner::method`, `Owner#method`, `Owner->method`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SymbolQuery<'a> {
    pub name: &'a str,
    pub owner: Option<&'a str>,
}

pub(crate) fn parse_symbol_query(raw: &str) -> SymbolQuery<'_> {
    let candidate = canonical_symbol(raw);
    let mut split = None;
    for separator in [":", ".", "#", "->"] {
        if let Some(index) = candidate.rfind(separator)
            && split.is_none_or(|(best, _)| index > best)
        {
            split = Some((index, separator.len()));
        }
    }
    let Some((index, width)) = split else {
        return SymbolQuery {
            name: candidate,
            owner: None,
        };
    };
    let name = &candidate[index + width..];
    let owner = candidate[..index]
        .rsplit([':', '.', '#', '-', '>'])
        .find(|part| !part.is_empty());
    match owner {
        Some(owner) if !name.is_empty() && is_symbol_identifier(name) => SymbolQuery {
            name,
            owner: Some(owner),
        },
        _ => SymbolQuery {
            name: candidate,
            owner: None,
        },
    }
}

fn is_symbol_identifier(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
}

fn owner_tier(requested: Option<&str>, stored: Option<&str>) -> u8 {
    match (requested, stored) {
        (None, _) => OWNER_TIER_EXACT,
        (Some(requested), Some(stored)) if requested == stored => OWNER_TIER_EXACT,
        (Some(requested), Some(stored)) if requested.eq_ignore_ascii_case(stored) => {
            OWNER_TIER_FOLDED
        }
        _ => OWNER_TIER_NONE,
    }
}

pub fn index_chunk_definition(
    conn: &Connection,
    chunk: &IndexedChunk,
    chunk_key: i64,
) -> Result<()> {
    let mut rows = Vec::new();
    append_chunk_definition_rows(chunk, chunk_key, &mut rows);
    let mut stmt = conn.prepare_cached(
        "INSERT OR REPLACE INTO symbols (normalized_name, chunk_key, name, owner)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for row in rows {
        stmt.execute(params![
            row.normalized_name,
            row.chunk_key,
            row.name,
            row.owner
        ])?;
    }
    Ok(())
}

pub(crate) fn append_chunk_definition_rows(
    chunk: &IndexedChunk,
    chunk_key: i64,
    rows: &mut Vec<SymbolRow>,
) {
    for definition in chunk_definitions(chunk) {
        let name = canonical_symbol(&definition.name).to_string();
        let normalized_name = name.to_ascii_lowercase();
        rows.push(SymbolRow {
            name: stored_symbol_name(&name, &normalized_name),
            normalized_name,
            chunk_key,
            owner: definition.owner,
        });
    }
}

pub fn remove_file_graph(conn: &Connection, file_path: &str) -> Result<()> {
    conn.prepare_cached(
        "DELETE FROM symbols
         WHERE chunk_key IN (SELECT chunk_key FROM chunks WHERE file_path = ?1)",
    )?
    .execute(params![file_path])?;
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

    search_symbols_in_current_index(workspace, name, mode, options)
}

pub(crate) fn search_symbols_in_current_index(
    workspace: &Workspace,
    name: &str,
    mode: SymbolSearchMode,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let candidate_name = canonical_symbol(name);
    let normalized = normalize_symbol(candidate_name);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    if mode != SymbolSearchMode::Definitions {
        return search_call_sites(workspace, candidate_name, &normalized, mode, options);
    }

    let query = parse_symbol_query(name);
    let query_normalized = normalize_symbol(query.name);
    let primary_sqlite = if workspace.has_overlay() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    let mut hits = query_workspace_db(
        &open_sqlite_readonly(&primary_sqlite)?,
        &query,
        &query_normalized,
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
            for hit in query_workspace_db(
                &base,
                &query,
                &query_normalized,
                &base_options,
                &path_matcher,
            )? {
                let path = hit.1.file_path.to_string_lossy();
                if !tombstones.contains(path.as_ref()) && !overlay_files.contains(path.as_ref()) {
                    hits.push(hit);
                }
            }
        }
    }

    // Qualified lookups keep only the best owner tier that exists anywhere in
    // the workspace; bare-name rows are the fallback, never a supplement.
    if query.owner.is_some()
        && let Some(best_tier) = hits.iter().map(|(tier, _)| *tier).min()
        && best_tier < OWNER_TIER_NONE
    {
        hits.retain(|(tier, _)| *tier == best_tier);
    }

    let mut hits = hits.into_iter().map(|(_, hit)| hit).collect::<Vec<_>>();
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

    let mut seen_requests = HashSet::new();
    let mut requested = Vec::new();
    for name in names {
        let query = parse_symbol_query(name);
        let normalized = normalize_symbol(query.name);
        if normalized.is_empty()
            || !seen_requests.insert((normalized.clone(), query.owner.map(str::to_ascii_lowercase)))
        {
            continue;
        }
        requested.push((normalized, query));
    }
    if requested.is_empty() {
        return Ok(Vec::new());
    }

    let mut by_name = (0..requested.len())
        .map(|_| Vec::new())
        .collect::<Vec<Vec<(u8, bool, bool, IndexedChunk)>>>();
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
            .map(|index| {
                format!(
                    "(?{}, ?{}, ?{}, {index})",
                    index * 3 + 1,
                    index * 3 + 2,
                    index * 3 + 3
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        // Owner-qualified rows and exact-case names rank ahead of the
        // alphabetical tail so common names do not crowd them out of the
        // bounded candidate window.
        let sql = format!(
            "WITH requested(name, exact_name, owner, ordinal) AS (VALUES {values}),
                  ranked AS (
                    SELECT r.ordinal,
                           c.file_path, c.start_line, c.end_line, c.language,
                           c.kind, c.text, c.vector_key, c.is_ignored,
                           COALESCE(s.name, s.normalized_name) AS name, s.owner,
                           row_number() OVER (
                             PARTITION BY r.ordinal
                             ORDER BY CASE
                                        WHEN r.owner IS NULL THEN 0
                                        WHEN s.owner = r.owner THEN 0
                                        WHEN s.owner = r.owner COLLATE NOCASE THEN 1
                                        ELSE 2
                                      END,
                                      (COALESCE(s.name, s.normalized_name) = r.exact_name) DESC,
                                      c.file_path, c.start_line
                           ) AS rn
                    FROM requested r
                    JOIN symbols s ON s.normalized_name = r.name
                    JOIN chunks c ON c.chunk_key = s.chunk_key
                  )
             SELECT ordinal, file_path, start_line, end_line, language,
                    kind, text, vector_key, is_ignored, name, owner
             FROM ranked
             WHERE rn <= ?{}
             ORDER BY ordinal, rn",
            batch.len() * 3 + 1
        );
        let mut params: Vec<&dyn ToSql> = Vec::with_capacity(batch.len() * 3 + 1);
        for (normalized, query) in batch {
            params.push(normalized as &dyn ToSql);
            params.push(&query.name as &dyn ToSql);
            params.push(&query.owner as &dyn ToSql);
        }
        params.push(&candidate_limit_i64);

        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                base_ordinal + row.get::<_, i64>(0)? as usize,
                PathBuf::from(row.get::<_, String>(1)?),
                row.get::<_, i64>(2)? as usize,
                row.get::<_, i64>(3)? as usize,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, i64>(7)? as u64,
                row.get::<_, bool>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })?;

        for row in rows {
            let (
                ordinal,
                file_path,
                start_line,
                end_line,
                language,
                kind,
                raw,
                vector_key,
                is_ignored,
                stored_name,
                stored_owner,
            ) = row?;
            let text = try_decompress_text(raw).with_context(|| {
                format!(
                    "failed to read stored symbol text for {}:{start_line}-{end_line}",
                    file_path.display()
                )
            })?;
            let chunk = IndexedChunk {
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
                definitions: None,
            };
            if ordinal >= by_name.len() {
                continue;
            }
            if !per_name_seen[ordinal].insert(chunk.vector_key) {
                continue;
            }
            let query = requested[ordinal].1;
            let tier = owner_tier(query.owner, stored_owner.as_deref());
            let exact_case = stored_name == query.name;
            let canonical_file = file_stem_matches_symbol(&chunk, query.name);
            by_name[ordinal].push((tier, exact_case, canonical_file, chunk));
        }
    }

    let mut seen_chunks = HashSet::new();
    let mut chunks = Vec::new();
    for (name_candidates, (_, query)) in by_name.iter_mut().zip(&requested) {
        let remaining = limit.saturating_sub(chunks.len());
        if remaining == 0 {
            break;
        }
        if query.owner.is_some()
            && let Some(best_tier) = name_candidates.iter().map(|(tier, ..)| *tier).min()
            && best_tier < OWNER_TIER_NONE
        {
            name_candidates.retain(|(tier, ..)| *tier == best_tier);
        }
        name_candidates.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| left.3.file_path.cmp(&right.3.file_path))
                .then_with(|| left.3.start_line.cmp(&right.3.start_line))
        });
        for (_, _, _, chunk) in name_candidates.drain(..).take(remaining) {
            if seen_chunks.insert(chunk.vector_key) {
                chunks.push(chunk);
            }
        }
    }
    Ok(chunks)
}

fn query_workspace_db(
    conn: &Connection,
    query: &SymbolQuery<'_>,
    normalized: &str,
    options: &SearchOptions,
    path_matcher: &PathGlobMatcher,
) -> Result<Vec<(u8, SearchHit)>> {
    let sql = "SELECT c.file_path, c.start_line, c.end_line, c.text,
                      c.language, c.is_ignored, COALESCE(s.name, s.normalized_name), s.owner
               FROM symbols s JOIN chunks c ON c.chunk_key = s.chunk_key
               WHERE s.normalized_name = ?1
               ORDER BY CASE
                          WHEN ?3 IS NULL THEN 0
                          WHEN s.owner = ?3 THEN 0
                          WHEN s.owner = ?3 COLLATE NOCASE THEN 1
                          ELSE 2
                        END,
                        (COALESCE(s.name, s.normalized_name) = ?2) DESC,
                        c.file_path, c.start_line";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![normalized, query.name, query.owner], |row| {
        Ok((
            PathBuf::from(row.get::<_, String>(0)?),
            row.get::<_, i64>(1)? as usize,
            row.get::<_, i64>(2)? as usize,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    })?;

    let mut hits = Vec::new();
    let mut accepted_tier = None;
    for row in rows {
        let (file_path, start_line, end_line, raw, language, is_ignored, name, owner) = row?;
        let tier = owner_tier(query.owner, owner.as_deref());
        // Rows arrive best owner tier first; once a qualified tier produced a
        // hit, weaker tiers are the fallback and are skipped.
        match accepted_tier {
            Some(accepted) if tier > accepted && accepted < OWNER_TIER_NONE => break,
            _ => {}
        }
        let preview = try_decompress_text(raw).with_context(|| {
            format!(
                "failed to read stored symbol text for {}:{start_line}-{end_line}",
                file_path.display()
            )
        })?;
        let exact_case = name == query.name;
        let mut score = if exact_case { 10.0 } else { 9.0 };
        if tier == OWNER_TIER_FOLDED {
            score -= 0.5;
        }
        let hit = SearchHit {
            file_path,
            start_line,
            end_line,
            preview,
            reason: "exact symbol match".to_string(),
            score,
            sources: vec!["symbol".to_string()],
            neural_requested: false,
            neural_executed: false,
        };
        if options
            .scope_filter
            .as_ref()
            .is_none_or(|scope| scope.matches(&hit.file_path))
            && type_matches(&language, options.type_filter.as_deref())
            && path_matcher.matches(&hit.file_path)
            && (options.skip_gitignore || !is_ignored)
        {
            accepted_tier.get_or_insert(tier);
            hits.push((tier, hit));
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
    let (callers, references) =
        search_call_sites_with_references(workspace, name, normalized, options)?;
    match mode {
        SymbolSearchMode::Callers => Ok(callers),
        SymbolSearchMode::References => Ok(references),
        SymbolSearchMode::Definitions => unreachable!(),
    }
}

pub(crate) fn search_symbol_relationships_in_current_index(
    workspace: &Workspace,
    name: &str,
    options: &SearchOptions,
) -> Result<(Vec<SearchHit>, Vec<SearchHit>)> {
    let candidate_name = canonical_symbol(name);
    let normalized = normalize_symbol(candidate_name);
    if normalized.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    search_call_sites_with_references(workspace, candidate_name, &normalized, options)
}

/// Languages that define the requested symbol, gathered from the overlay and
/// base symbol tables. Owner-qualified lookups use the best owner tier that
/// exists. Empty when the symbol has no known definition.
fn definition_languages(workspace: &Workspace, name: &str) -> Result<HashSet<String>> {
    let query = parse_symbol_query(name);
    let normalized = normalize_symbol(query.name);
    if normalized.is_empty() {
        return Ok(HashSet::new());
    }
    let primary = if workspace.has_overlay() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    // Base rows are visible only where the worktree neither deleted nor
    // replaced the defining file, mirroring definition lookup.
    let mut databases = vec![(primary, None)];
    if let Some(base_dir) = &workspace.base_index_dir {
        let shadowed = load_path_set(&workspace.overlay_sqlite_path(), "tombstones")?
            .into_iter()
            .chain(load_chunk_paths(&workspace.overlay_sqlite_path())?)
            .collect::<HashSet<_>>();
        databases.push((base_dir.join("metadata.sqlite3"), Some(shadowed)));
    }

    let mut tiers: [HashSet<String>; 3] = Default::default();
    for (path, shadowed) in databases {
        if !path.exists() {
            continue;
        }
        let conn = open_sqlite_readonly(&path)?;
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT c.language, s.owner, c.file_path
             FROM symbols s JOIN chunks c ON c.chunk_key = s.chunk_key
             WHERE s.normalized_name = ?1",
        )?;
        let rows = stmt.query_map([&normalized], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (language, owner, file_path) = row?;
            if shadowed
                .as_ref()
                .is_some_and(|shadowed| shadowed.contains(&file_path))
            {
                continue;
            }
            let tier = owner_tier(query.owner, owner.as_deref());
            tiers[usize::from(tier)].insert(language.to_ascii_lowercase());
        }
    }
    Ok(tiers
        .into_iter()
        .find(|languages| !languages.is_empty())
        .unwrap_or_default())
}

fn search_call_sites_with_references(
    workspace: &Workspace,
    name: &str,
    normalized: &str,
    options: &SearchOptions,
) -> Result<(Vec<SearchHit>, Vec<SearchHit>)> {
    let mut candidate_options = options.clone();
    candidate_options.limit = options.limit.map(|limit| limit.saturating_mul(4));
    // Call sites are matched textually, so restrict them to the languages
    // that actually define the symbol. An explicit --type filter wins.
    let languages = if options.type_filter.is_none() {
        definition_languages(workspace, name)?
    } else {
        HashSet::new()
    };
    if languages.len() == 1 {
        candidate_options.type_filter = languages.iter().next().cloned();
    }
    let query = format!("{}(", name.trim());
    let mut candidates = if options.limit.is_some() {
        crate::search::exact_literal_chunks(workspace, &query, &candidate_options)?
    } else {
        crate::search::exact_literal_chunks_unbounded(workspace, &query, &candidate_options)?
    };
    if !languages.is_empty() {
        candidates.retain(|chunk| languages.contains(&chunk.language.to_ascii_lowercase()));
    }
    let mut callers = Vec::new();
    let mut references = Vec::new();
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
            if options.limit.is_none_or(|limit| callers.len() < limit) {
                callers.push(SearchHit {
                    file_path: chunk.file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    preview: chunk.text.clone(),
                    reason: "exact caller match".to_string(),
                    score: 8.0,
                    sources: vec!["caller".to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
            }
            for (line, preview) in call_lines {
                if options.limit.is_none_or(|limit| references.len() < limit) {
                    references.push(SearchHit {
                        file_path: chunk.file_path.clone(),
                        start_line: line,
                        end_line: line,
                        preview,
                        reason: "exact reference match".to_string(),
                        score: 6.0,
                        sources: vec!["reference".to_string()],
                        neural_requested: false,
                        neural_executed: false,
                    });
                }
            }
            if options
                .limit
                .is_some_and(|limit| callers.len() >= limit && references.len() >= limit)
            {
                return Ok((callers, references));
            }
        }
    }
    Ok((callers, references))
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
        .any(|part| {
            matches!(
                part,
                "class"
                    | "def"
                    | "enum"
                    | "fn"
                    | "func"
                    | "function"
                    | "interface"
                    | "record"
                    | "struct"
                    | "trait"
            )
        });
    if has_definition_keyword {
        return true;
    }

    let suffix = &line[name_offset..];
    let after_parameters = after_parameter_list(suffix);
    if after_parameters.is_some_and(|after| {
        after.starts_with('{')
            || after.starts_with("->")
            || after.starts_with(':')
            || after.starts_with("throws ")
    }) {
        return true;
    }

    after_parameters.is_some_and(looks_like_prototype_suffix) && looks_like_prototype_prefix(prefix)
}

fn looks_like_prototype_suffix(suffix: &str) -> bool {
    let Some(mut suffix) = suffix.trim_end().strip_suffix(';').map(str::trim) else {
        return false;
    };
    while !suffix.is_empty() {
        if let Some(default) = suffix.strip_prefix('=').map(str::trim) {
            return matches!(default, "0" | "default" | "delete");
        }
        if let Some(rest) = suffix
            .strip_prefix("&&")
            .or_else(|| suffix.strip_prefix('&'))
        {
            suffix = rest.trim_start();
            continue;
        }
        let qualifier_end = suffix
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .unwrap_or(suffix.len());
        let qualifier = &suffix[..qualifier_end];
        if !matches!(
            qualifier,
            "const" | "final" | "noexcept" | "override" | "volatile"
        ) {
            return false;
        }
        suffix = suffix[qualifier_end..].trim_start();
        if qualifier == "noexcept" && suffix.starts_with('(') {
            let Some(after) = after_parameter_list(suffix) else {
                return false;
            };
            suffix = after;
        }
    }
    true
}

fn after_parameter_list(suffix: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (offset, character) in suffix.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 1 => return Some(suffix[offset + 1..].trim_start()),
            ')' if depth > 1 => depth -= 1,
            _ => {}
        }
    }
    None
}

fn looks_like_prototype_prefix(prefix: &str) -> bool {
    if prefix.is_empty() || prefix.ends_with('.') || prefix.ends_with("->") {
        return false;
    }
    if prefix.ends_with("::") && !prefix.contains(char::is_whitespace) {
        return false;
    }

    let first_word = prefix
        .trim_start_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '_'
        })
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .unwrap_or_default();
    !matches!(
        first_word,
        "await"
            | "co_await"
            | "co_return"
            | "co_yield"
            | "defer"
            | "delete"
            | "new"
            | "return"
            | "throw"
            | "try"
            | "yield"
    )
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
    // Continuation windows repeat a definition body; their first code line
    // is arbitrary and the function fallback would register a callee.
    if is_continuation_text(&chunk.text) {
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

/// Chunk windows after the first carry a `// continuation of ...` header.
pub(crate) fn is_continuation_text(text: &str) -> bool {
    text.lines()
        .nth(1)
        .is_some_and(|line| line.starts_with("// continuation of "))
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
        // `@Override public void run() {` keeps its declaration once the
        // annotation prefix is removed; a bare decorator line is skipped.
        let line = strip_leading_annotations(line);
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

/// Definitions a chunk registers in the symbol table: parser-derived names
/// when the chunker captured them, otherwise the text heuristic, plus public
/// re-exports in either case.
fn chunk_definitions(chunk: &IndexedChunk) -> Vec<ChunkDefinition> {
    let mut definitions = match &chunk.definitions {
        Some(parsed) => parsed.clone(),
        None => heuristic_definition_names(chunk)
            .into_iter()
            .map(|name| ChunkDefinition { name, owner: None })
            .collect(),
    };
    definitions.extend(
        public_reexport_names(chunk)
            .into_iter()
            .map(|name| ChunkDefinition { name, owner: None }),
    );
    attach_qualified_owners(&mut definitions);
    let mut seen = HashSet::new();
    definitions.retain(|definition| seen.insert(normalize_symbol(&definition.name)));
    definitions
}

/// Heuristic names such as Elixir's `Phoenix.Channel` also register the leaf
/// `Channel`; give that leaf its qualifier as owner so `Phoenix.Channel`
/// lookups stay precise.
fn attach_qualified_owners(definitions: &mut [ChunkDefinition]) {
    let qualified = definitions
        .iter()
        .filter_map(|definition| {
            let (qualifier, leaf) = definition.name.rsplit_once('.')?;
            let owner = qualifier.rsplit('.').find(|part| !part.is_empty())?;
            Some((leaf.to_string(), owner.to_string()))
        })
        .collect::<Vec<_>>();
    for (leaf, owner) in qualified {
        if let Some(definition) = definitions
            .iter_mut()
            .find(|definition| definition.owner.is_none() && definition.name == leaf)
        {
            definition.owner = Some(owner);
        }
    }
}

fn definition_names(chunk: &IndexedChunk) -> Vec<String> {
    chunk_definitions(chunk)
        .into_iter()
        .map(|definition| definition.name)
        .collect()
}

fn heuristic_definition_names(chunk: &IndexedChunk) -> Vec<String> {
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
                        let signature = strip_leading_annotations(line.trim());
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
    names
}

pub(crate) fn likely_definition_names(text: &str) -> Vec<String> {
    const KEYWORDS: &[&str] = &[
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
    let mut seen = HashSet::new();
    text.lines()
        .filter_map(|line| definition_name_from_signature(line.trim(), KEYWORDS, false, false))
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
            definitions: None,
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
    fn heuristic_names_skip_annotations_and_continuation_windows() {
        assert_eq!(
            definition_name(&chunk(
                "java",
                "Function",
                "// src/Worker.java\n\n@Override public void run() {\n    if (ready()) { dispatch(); }\n}"
            ))
            .as_deref(),
            Some("run")
        );
        assert_eq!(
            definition_name(&chunk(
                "java",
                "Function",
                "@Inject\n@SuppressWarnings(\"unchecked\")\npublic Worker(Repo repo) {"
            ))
            .as_deref(),
            Some("Worker")
        );
        assert_eq!(
            definition_name(&chunk(
                "python",
                "Function",
                "@property\ndef name(self):\n    return normalize(self._name)"
            ))
            .as_deref(),
            Some("name")
        );
        assert_eq!(
            definition_name(&chunk(
                "rust",
                "Function",
                "#[inline] pub fn method(&self) -> u32 { self.speed }"
            ))
            .as_deref(),
            Some("method")
        );
        assert_eq!(
            definition_name(&chunk(
                "rust",
                "Function",
                "// src/pipeline.rs\n// continuation of pub fn long_pipeline() {\n\n    probe_callee(41);\n    probe_callee(42);"
            )),
            None,
            "continuation windows must not register body callees"
        );
    }

    #[test]
    fn parser_definitions_take_precedence_over_the_heuristic() {
        let mut parsed = chunk(
            "java",
            "Function",
            "// src/Worker.java\n\n@Override public void run() {\n    if (ready()) { dispatch(); }\n}",
        );
        parsed.definitions = Some(vec![ChunkDefinition {
            name: "run".to_string(),
            owner: Some("Worker".to_string()),
        }]);
        let mut rows = Vec::new();
        append_chunk_definition_rows(&parsed, 3, &mut rows);
        assert_eq!(
            rows,
            [SymbolRow {
                normalized_name: "run".to_string(),
                chunk_key: 3,
                name: None,
                owner: Some("Worker".to_string()),
            }]
        );

        let mut empty = parsed.clone();
        empty.definitions = Some(Vec::new());
        let mut rows = Vec::new();
        append_chunk_definition_rows(&empty, 4, &mut rows);
        assert!(rows.is_empty(), "{rows:?}");
    }

    #[test]
    fn symbol_queries_split_owner_qualifiers() {
        for (raw, owner, name) in [
            ("Outer.method", Some("Outer"), "method"),
            ("Outer::method", Some("Outer"), "method"),
            ("Outer#method", Some("Outer"), "method"),
            ("Outer->method", Some("Outer"), "method"),
            ("a.b.Outer.method()", Some("Outer"), "method"),
            ("method", None, "method"),
            (" method( ", None, "method"),
        ] {
            assert_eq!(
                parse_symbol_query(raw),
                SymbolQuery { name, owner },
                "{raw}"
            );
        }
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
                name TEXT,
                owner TEXT,
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
                name TEXT,
                owner TEXT,
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
                name TEXT,
                owner TEXT,
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
                name TEXT,
                owner TEXT,
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
    fn persisted_symbol_reads_reject_corrupt_compressed_text() {
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
                name TEXT,
                owner TEXT,
                PRIMARY KEY (normalized_name, chunk_key)
             ) WITHOUT ROWID;",
        )
        .unwrap();

        let mut corrupt = zstd::stream::encode_all(&b"pub fn broken() {}"[..], 1).unwrap();
        corrupt.truncate(corrupt.len() - 2);
        conn.execute(
            "INSERT INTO chunks (
                chunk_key, file_path, start_line, end_line, language, kind,
                text, vector_key, is_ignored
             ) VALUES (1, 'src/broken.rs', 4, 4, 'rust', 'Function', ?1, 7, 0)",
            [&corrupt],
        )
        .unwrap();
        conn.execute("INSERT INTO symbols VALUES ('broken', 1, NULL, NULL)", [])
            .unwrap();

        let candidate_error = definition_candidates(&conn, &["broken".to_string()], 1)
            .unwrap_err()
            .to_string();
        assert!(candidate_error.contains("failed to read stored symbol text"));

        let matcher = PathGlobMatcher::new(&[], &[]).unwrap();
        let query = parse_symbol_query("broken");
        let search_error =
            query_workspace_db(&conn, &query, "broken", &SearchOptions::default(), &matcher)
                .unwrap_err()
                .to_string();
        assert!(search_error.contains("failed to read stored symbol text"));

        // Removal no longer re-derives names from stored text.
        remove_file_graph(&conn, "src/broken.rs").unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
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
    fn type_declarations_are_not_call_sites() {
        for (source, symbol) in [
            ("pub struct UserService(pub u64);", "userservice"),
            ("data class UserService(val id: Long)", "userservice"),
            ("record UserService(String id) {}", "userservice"),
        ] {
            assert!(
                matching_call_lines(source, symbol, 1, 1).is_empty(),
                "{source}"
            );
        }
        assert_eq!(
            matching_call_lines("let service = UserService(7);", "userservice", 1, 1),
            [(1, "let service = UserService(7);".to_string())]
        );
    }

    #[test]
    fn function_prototypes_are_not_call_sites() {
        for (source, symbol) in [
            ("int parse();", "parse"),
            ("void send();", "send"),
            ("public abstract void send();", "send"),
            ("int parser::parse();", "parse"),
            ("int parse(void (*callback)());", "parse"),
            ("int parse() const;", "parse"),
            ("void send() noexcept;", "send"),
            ("virtual bool send() = 0;", "send"),
            ("Widget make() const noexcept final;", "make"),
            ("Result parse() && override;", "parse"),
            ("void send() noexcept(noexcept(flush()));", "send"),
        ] {
            assert!(
                matching_call_lines(source, symbol, 1, 1).is_empty(),
                "{source}"
            );
        }
        for (source, symbol) in [
            ("parse();", "parse"),
            ("client.send();", "send"),
            ("parser::parse();", "parse"),
            ("return parse();", "parse"),
            ("await parse();", "parse"),
            ("ready && parse() && accepted;", "parse"),
        ] {
            assert_eq!(
                matching_call_lines(source, symbol, 1, 1),
                [(1, source.to_string())],
                "{source}"
            );
        }
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
