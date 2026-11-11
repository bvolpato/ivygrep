use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::Bytes;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};
use rayon::prelude::*;
use regex_syntax::hir::{Hir, HirKind};
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, RegexQuery, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::schema::Value;

use crate::indexer::{open_sqlite_readonly, open_tantivy_index};
use crate::merkle::MerkleSnapshot;
use crate::path_glob::PathGlobMatcher;
use crate::protocol::{FileSearchResult, SearchHit};
use crate::search::SearchOptions;
use crate::workspace::{Workspace, WorkspaceScope, index_path_string};
use crate::workspace_file::RootHandle;

const MAX_CONTEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_COVERAGE_CACHE_ENTRIES: usize = 32;
const MAX_COVERAGE_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_UNINDEXED_FILES: usize = 4_096;
const REGEX_PARALLEL_BATCH_FILES: usize = 256;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum CoverageConsumer {
    Literal,
    Regex,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct CoverageKey {
    workspace_id: String,
    index_dir: PathBuf,
    consumer: CoverageConsumer,
    skip_gitignore: bool,
    publication: PublicationStamp,
}

/// Identifies one completed index publication. Each publication bumps a
/// generation and atomically replaces the saved Merkle snapshot.
#[derive(Clone, Eq, Hash, PartialEq)]
struct PublicationStamp {
    index_generation: u64,
    base_generation: u64,
    snapshot: crate::merkle::SnapshotStamp,
}

/// Snapshot paths without chunks. `None` means there are too many to enumerate.
type Coverage = Option<Arc<Vec<PathBuf>>>;

enum CachedCoverage {
    Literal(Coverage),
    // Only a chunk-backed path with unchanged metadata can rely on indexed trigrams.
    Regex(HashMap<String, String>),
}

impl CachedCoverage {
    fn retained_bytes(&self, key: &CoverageKey) -> usize {
        let overhead = std::mem::size_of::<(CoverageKey, Arc<Self>, usize)>()
            + std::mem::size_of::<Self>()
            + 4 * std::mem::size_of::<usize>()
            + key.workspace_id.capacity()
            + key.index_dir.capacity();
        overhead
            + match self {
                Self::Regex(hashes) => {
                    2 * hashes.capacity() * (std::mem::size_of::<(String, String)>() + 1)
                        + 16
                        + hashes
                            .iter()
                            .map(|(path, hash)| path.capacity() + hash.capacity())
                            .sum::<usize>()
                }
                Self::Literal(Some(paths)) => {
                    2 * std::mem::size_of::<usize>()
                        + std::mem::size_of::<Vec<PathBuf>>()
                        + paths.capacity() * std::mem::size_of::<PathBuf>()
                        + paths.iter().map(PathBuf::capacity).sum::<usize>()
                }
                Self::Literal(None) => 0,
            }
    }
}

fn coverage_cache() -> &'static Mutex<crate::byte_cache::ByteCache<CoverageKey, Arc<CachedCoverage>>>
{
    static CACHE: OnceLock<Mutex<crate::byte_cache::ByteCache<CoverageKey, Arc<CachedCoverage>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(crate::byte_cache::ByteCache::new(
            MAX_COVERAGE_CACHE_ENTRIES,
            MAX_COVERAGE_CACHE_BYTES,
        ))
    })
}

fn remember_coverage(key: CoverageKey, coverage: CachedCoverage) -> Arc<CachedCoverage> {
    let bytes = coverage.retained_bytes(&key);
    let coverage = Arc::new(coverage);
    if let Ok(mut cache) = coverage_cache().lock() {
        cache.insert(key, coverage.clone(), bytes);
    }
    coverage
}

/// Index-backed regex search.
///
/// When the workspace has an index, extracts literal fragments from the regex
/// pattern and uses the Tantivy inverted index to pre-filter to only files
/// that could possibly match. Files are then regex-scanned in parallel using
/// rayon for maximum throughput.
///
/// Falls back to a filesystem walk when no index exists or no usable literals
/// can be extracted from the pattern.
pub fn regex_search(
    workspace: &Workspace,
    pattern: &str,
    limit: Option<usize>,
    scope_filter: Option<&WorkspaceScope>,
    include_globs: &[String],
    exclude_globs: &[String],
    skip_gitignore: bool,
) -> Result<Vec<SearchHit>> {
    regex_search_with_options(
        workspace,
        pattern,
        &SearchOptions {
            limit,
            context: 0,
            scope_filter: scope_filter.cloned(),
            include_globs: include_globs.to_vec(),
            exclude_globs: exclude_globs.to_vec(),
            skip_gitignore,
            ..Default::default()
        },
    )
}

/// Regex search with language filtering, context expansion, and shared search options.
pub fn regex_search_with_options(
    workspace: &Workspace,
    pattern: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    let max_hits = options.bounded_limit().unwrap_or(usize::MAX);
    if max_hits == 0 {
        return Ok(Vec::new());
    }
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;

    // Try to use index-backed pre-filtering via literal extraction.
    let candidate_files = index_prefilter_files(
        workspace,
        pattern,
        options.scope_filter.as_ref(),
        &path_matcher,
        options,
    );

    let mut hits = if let Some(paths) = candidate_files {
        tracing::trace!(
            "regex index prefilter: {} candidate files from index",
            paths.len()
        );
        regex_search_parallel(workspace, pattern, &paths, max_hits, options)
    } else {
        regex_search_walk(workspace, pattern, max_hits, &path_matcher, options)
    }?;
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    expand_regex_context(workspace, &mut hits, options.bounded_context(), options);
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    Ok(hits)
}

/// Rank files by matching line count without retaining previews or expanding context.
pub fn regex_file_name_search_with_options(
    workspace: &Workspace,
    pattern: &str,
    options: &SearchOptions,
) -> Result<Vec<FileSearchResult>> {
    if options.is_cancelled() || options.bounded_limit() == Some(0) {
        return Ok(Vec::new());
    }
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;
    let files = RootHandle::new(&workspace.root);
    let mut groups = if let Some(paths) = index_prefilter_files(
        workspace,
        pattern,
        options.scope_filter.as_ref(),
        &path_matcher,
        options,
    ) {
        paths
            .par_iter()
            .filter_map(|path| {
                scan_regex_file(
                    &files,
                    path,
                    &matcher,
                    &mut text_searcher(),
                    options,
                    RegexLines::Count,
                )
                .ok()
                .flatten()
            })
            .collect()
    } else {
        let mut groups = Vec::new();
        let mut searcher = text_searcher();
        walk_regex_paths(
            workspace,
            &path_matcher,
            options,
            false,
            |rel, type_filter| {
                if let Some(group) = scan_regex_file(
                    &files,
                    rel,
                    &matcher,
                    &mut searcher,
                    options,
                    RegexLines::Count,
                )? && (type_filter != PathTypeFilterMatch::ValidateText
                    || unknown_file_is_indexable_text(&files, rel))
                {
                    groups.push(group);
                }
                Ok(true)
            },
        )?;
        groups
    };
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    groups.sort_by(|a, b| {
        b.hit_count
            .cmp(&a.hit_count)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });
    if let Some(limit) = options.bounded_limit() {
        groups.truncate(limit);
    }
    Ok(groups)
}

fn required_literal_runs(pattern: &str) -> Option<Vec<String>> {
    let hir = regex_syntax::Parser::new().parse(pattern).ok()?;
    let mut literals = Vec::new();
    collect_required_literals(&hir, &mut literals)?;
    let mut runs = literals
        .into_iter()
        .flat_map(|literal| {
            literal
                // Unicode case-insensitive regexes also match K for K and ſ for S.
                // Neither character contributes an ASCII trigram to the index.
                .split(|byte: &u8| {
                    !byte.is_ascii_alphanumeric() || matches!(byte, b'k' | b'K' | b's' | b'S')
                })
                .filter(|run| run.len() >= 3 && run.is_ascii())
                .map(|run| String::from_utf8_lossy(run).to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
    runs.dedup();
    (!runs.is_empty()).then_some(runs)
}

fn collect_required_literals(hir: &Hir, literals: &mut Vec<Vec<u8>>) -> Option<()> {
    match hir.kind() {
        HirKind::Literal(literal) => literals.push(literal.0.to_vec()),
        HirKind::Repetition(repetition) if repetition.min > 0 => {
            collect_required_literals(&repetition.sub, literals)?;
        }
        HirKind::Capture(capture) => collect_required_literals(&capture.sub, literals)?,
        HirKind::Concat(expressions) => {
            for expression in expressions {
                collect_required_literals(expression, literals)?;
            }
        }
        HirKind::Alternation(_) => return None,
        HirKind::Empty | HirKind::Class(_) | HirKind::Look(_) | HirKind::Repetition(_) => {}
    }
    Some(())
}

/// Use the Tantivy index to find files containing the literal fragments
/// extracted from the regex pattern. Returns None if no index or no
/// usable literals.
fn index_prefilter_files(
    workspace: &Workspace,
    pattern: &str,
    scope_filter: Option<&WorkspaceScope>,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Option<Vec<PathBuf>> {
    if scope_filter.is_some_and(|scope| scope.is_file) {
        return None;
    }
    let required_runs = required_literal_runs(pattern)?;
    let publication = publication_stamp(workspace)?;
    let (use_overlay, shadowed_paths) = overlay_shadowed_paths(workspace)?;
    let tiers = if use_overlay {
        let base = workspace.base_index_dir.as_ref()?;
        vec![
            (workspace.overlay_tantivy_dir(), false),
            (base.join("tantivy"), true),
        ]
    } else {
        vec![(workspace.tantivy_dir(), false)]
    };
    let mut candidate_files = HashSet::new();
    for (tantivy_dir, is_base) in tiers {
        if !tantivy_dir.exists() {
            return None;
        }
        let (idx, fields) = open_tantivy_index(&tantivy_dir).ok()?;
        let reader = idx.reader().ok()?;
        let searcher = reader.searcher();
        let query =
            crate::search::substring_candidate_query(fields.text_trigrams?, &required_runs)?;
        let query = constrain_query_to_scope(query, fields.file_path, scope_filter)?;
        let docs = searcher
            .search(&query, &TopDocs::with_limit(10_000).order_by_score())
            .ok()?;
        if docs.len() == 10_000 {
            return None;
        }

        for (_score, addr) in docs {
            if options.is_cancelled() {
                return Some(Vec::new());
            }
            if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
                && (options.skip_gitignore
                    || fields
                        .is_ignored
                        .and_then(|field| doc.get_first(field))
                        .and_then(|value| value.as_u64())
                        .is_none_or(|value| value == 0))
                && let Some(path_val) = doc.get_first(fields.file_path)
                && let Some(path_str) = path_val.as_str()
                && !(is_base && shadowed_paths.contains(path_str))
            {
                let rel = PathBuf::from(path_str);
                if scope_filter.is_none_or(|s| s.matches(&rel))
                    && path_matcher.matches(&rel)
                    && options.type_filter.as_deref().is_none_or(|filter| {
                        doc.get_first(fields.language)
                            .and_then(|value| value.as_str())
                            .is_some_and(|language| type_filter_matches_language(language, filter))
                    })
                {
                    candidate_files.insert(rel);
                }
            }
        }
    }

    live_regex_candidates(
        workspace,
        candidate_files,
        publication,
        path_matcher,
        options,
    )
}

/// Overlay chunks and tombstones hide the same base paths from index scans.
fn overlay_shadowed_paths(workspace: &Workspace) -> Option<(bool, HashSet<String>)> {
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    if use_overlay && workspace.worktree_overlay_is_stale().ok()? {
        return None;
    }
    let overlay_sqlite = use_overlay
        .then(|| open_sqlite_readonly(&workspace.overlay_sqlite_path()).ok())
        .flatten();
    if use_overlay && overlay_sqlite.is_none() {
        return None;
    }
    let mut shadowed_paths = HashSet::new();
    if let Some(sqlite) = &overlay_sqlite {
        for query in [
            "SELECT DISTINCT file_path FROM chunks",
            "SELECT file_path FROM tombstones",
        ] {
            collect_sqlite_paths(sqlite, query, &mut shadowed_paths)?;
        }
    }
    Some((use_overlay, shadowed_paths))
}

fn live_regex_candidates(
    workspace: &Workspace,
    indexed_candidates: HashSet<PathBuf>,
    publication: PublicationStamp,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Option<Vec<PathBuf>> {
    let coverage = cached_coverage(
        workspace,
        CoverageConsumer::Regex,
        options,
        publication.clone(),
    )?;
    let CachedCoverage::Regex(hashes) = coverage.as_ref() else {
        return None;
    };
    let files = RootHandle::new(&workspace.root);
    let mut paths = Vec::new();
    let mut changed = 0;
    walk_regex_paths(
        workspace,
        path_matcher,
        options,
        false,
        |rel, type_match| {
            if !indexed_candidates.contains(rel) {
                #[cfg(unix)]
                let normalized = rel.to_string_lossy();
                #[cfg(not(unix))]
                let normalized: std::borrow::Cow<'_, str> =
                    std::borrow::Cow::Owned(index_path_string(rel));
                if hashes.get(normalized.as_ref()).is_some_and(|hash| {
                    MerkleSnapshot::path_matches_metadata_snapshot(&workspace.root.join(rel), hash)
                }) {
                    return Ok(true);
                }
                changed += 1;
                if changed > MAX_UNINDEXED_FILES {
                    return Ok(false);
                }
            }
            if type_match != PathTypeFilterMatch::ValidateText
                || unknown_file_is_indexable_text(&files, rel)
            {
                paths.push(rel.to_owned());
            }
            Ok(true)
        },
    )
    .ok()?;
    if changed > MAX_UNINDEXED_FILES || publication_stamp(workspace)? != publication {
        return None;
    }
    paths.sort();
    Some(paths)
}

fn collect_sqlite_paths(
    sqlite: &rusqlite::Connection,
    query: &str,
    paths: &mut HashSet<String>,
) -> Option<()> {
    let mut statement = sqlite.prepare(query).ok()?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?;
    for row in rows {
        paths.insert(row.ok()?);
    }
    Some(())
}

/// Unindexed files that literal verification must still read, such as
/// minified bundles. Visibility comes from the saved Merkle snapshot rather
/// than a separate walk, so ignored and excluded paths stay hidden on every
/// platform. Files over the indexing size limit are never recorded and remain
/// regex-only. `None` means coverage is unknown or too large, and callers keep
/// only indexed candidates.
pub(crate) fn unindexed_literal_candidates(
    workspace: &Workspace,
    options: &SearchOptions,
) -> Coverage {
    cached_coverage(
        workspace,
        CoverageConsumer::Literal,
        options,
        publication_stamp(workspace)?,
    )
    .and_then(|coverage| match coverage.as_ref() {
        CachedCoverage::Literal(paths) => paths.clone(),
        CachedCoverage::Regex(_) => None,
    })
}

fn cached_coverage(
    workspace: &Workspace,
    consumer: CoverageConsumer,
    options: &SearchOptions,
    publication: PublicationStamp,
) -> Option<Arc<CachedCoverage>> {
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    if use_overlay && workspace.worktree_overlay_is_stale().ok()? {
        return None;
    }
    let key = CoverageKey {
        workspace_id: workspace.id.clone(),
        index_dir: workspace.index_dir.clone(),
        consumer,
        skip_gitignore: options.skip_gitignore,
        publication,
    };
    if let Some(coverage) = coverage_cache().lock().ok()?.get(&key) {
        return Some(coverage.clone());
    }

    let snapshot = MerkleSnapshot::load(&workspace.merkle_snapshot_path()).ok()?;
    let covered = chunk_backed_paths(workspace, use_overlay)?;
    // Stores commit before the snapshot is saved. Never pair chunks from one
    // publication with the file list of another.
    if publication_stamp(workspace)? != key.publication {
        return None;
    }
    let visible = |hash: &str| options.skip_gitignore || !hash.ends_with("-1");
    if consumer == CoverageConsumer::Regex {
        let hashes = snapshot
            .files
            .into_iter()
            .filter(|(path, hash)| covered.contains(path) && visible(hash))
            .collect();
        return Some(remember_coverage(key, CachedCoverage::Regex(hashes)));
    }
    let mut recorded = Vec::new();
    for (path, hash) in &snapshot.files {
        if covered.contains(path) || !visible(hash) {
            continue;
        }
        recorded.push(PathBuf::from(path));
        if recorded.len() > MAX_UNINDEXED_FILES {
            return Some(remember_coverage(key, CachedCoverage::Literal(None)));
        }
    }
    // Empty files and files with a NUL in the sniffed prefix cannot produce a literal hit.
    let files = RootHandle::new(&workspace.root);
    recorded = recorded
        .into_par_iter()
        .filter(|path| !options.is_cancelled() && may_contain_text(&files, path))
        .collect();
    if options.is_cancelled() {
        return None;
    }
    Some(remember_coverage(
        key,
        CachedCoverage::Literal(Some(Arc::new(recorded))),
    ))
}

/// Returns `None` while this index or its base carries a publication marker,
/// including after a failed publication, because stores and the snapshot may
/// then disagree.
fn publication_stamp(workspace: &Workspace) -> Option<PublicationStamp> {
    if workspace.has_unfinished_index_publication()
        || workspace.base_has_unfinished_index_publication()
    {
        return None;
    }
    let index_generation = workspace
        .read_metadata()
        .ok()?
        .map_or(0, |m| m.index_generation);
    let base_generation = workspace
        .base_index_dir
        .as_ref()
        .and_then(|base| fs::read(base.join("workspace.json")).ok())
        .and_then(|raw| serde_json::from_slice::<crate::workspace::WorkspaceMetadata>(&raw).ok())
        .map_or(0, |metadata| metadata.index_generation);
    Some(PublicationStamp {
        index_generation,
        base_generation,
        snapshot: crate::merkle::snapshot_stamp(&workspace.merkle_snapshot_path())?,
    })
}

/// Paths with chunks in the effective index. Worktree tombstones hide base
/// chunks, including base files replaced by content the indexer skips.
fn chunk_backed_paths(workspace: &Workspace, use_overlay: bool) -> Option<HashSet<String>> {
    // Seek `idx_chunks_file_path` once per distinct path. `SELECT DISTINCT`
    // visits every chunk row, which costs several times more on large indexes.
    const CHUNK_PATHS: &str = "WITH RECURSIVE paths(file_path) AS (
            SELECT MIN(file_path) FROM chunks
            UNION ALL
            SELECT (SELECT MIN(file_path) FROM chunks WHERE file_path > paths.file_path)
            FROM paths WHERE paths.file_path IS NOT NULL
        )
        SELECT file_path FROM paths WHERE file_path IS NOT NULL";
    let mut covered = HashSet::new();
    if !use_overlay {
        let sqlite = open_sqlite_readonly(&workspace.sqlite_path()).ok()?;
        collect_sqlite_paths(&sqlite, CHUNK_PATHS, &mut covered)?;
        return Some(covered);
    }
    let base =
        open_sqlite_readonly(&workspace.base_index_dir.as_ref()?.join("metadata.sqlite3")).ok()?;
    collect_sqlite_paths(&base, CHUNK_PATHS, &mut covered)?;
    let overlay = open_sqlite_readonly(&workspace.overlay_sqlite_path()).ok()?;
    let mut tombstones = HashSet::new();
    collect_sqlite_paths(
        &overlay,
        "SELECT file_path FROM tombstones",
        &mut tombstones,
    )?;
    covered.retain(|path| !tombstones.contains(path));
    collect_sqlite_paths(&overlay, CHUNK_PATHS, &mut covered)?;
    Some(covered)
}

/// Matches literal verification, which finds no text in empty files or files
/// with a NUL in the prefix indexing sniffs. Unreadable files stay candidates,
/// so a transient open failure cannot hide them for a whole publication.
fn may_contain_text(files: &RootHandle, path: &std::path::Path) -> bool {
    let Ok(file) = files.open(path) else {
        return true;
    };
    let mut sample = Vec::with_capacity(crate::chunking::TEXT_SNIFF_BYTES);
    match file
        .take(crate::chunking::TEXT_SNIFF_BYTES as u64)
        .read_to_end(&mut sample)
    {
        Ok(_) => !sample.is_empty() && !sample.contains(&0),
        Err(_) => true,
    }
}

fn walk_regex_paths(
    workspace: &Workspace,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
    path_order: bool,
    mut visit: impl FnMut(&Path, PathTypeFilterMatch) -> Result<bool>,
) -> Result<()> {
    let scope = options.scope_filter.as_ref();
    let mut visit_path = |rel: &Path| {
        let filter = type_filter_match_for_path(rel, options.type_filter.as_deref());
        if filter == PathTypeFilterMatch::Reject || !path_matcher.matches(rel) {
            return Ok(true);
        }
        visit(rel, filter)
    };
    if let Some(scope) = scope.filter(|s| s.is_file) {
        if !options.is_cancelled()
            && crate::walker::SourcePathMatcher::new(&workspace.root, options.skip_gitignore)
                .allows(&scope.rel_path)?
        {
            visit_path(&scope.rel_path)?;
        }
        return Ok(());
    }

    let mut walk = crate::walker::source_walker_scoped(
        &workspace.root,
        options.skip_gitignore,
        scope.map(|s| s.rel_path.as_path()),
    );
    if path_order {
        walk.sort_by_file_name(|left, right| left.cmp(right));
    }
    for entry in walk.build() {
        if options.is_cancelled() {
            break;
        }
        let entry = entry?;
        if entry.file_type().is_some_and(|kind| kind.is_file())
            && let Ok(rel) = entry.path().strip_prefix(&workspace.root)
            && !visit_path(rel)?
        {
            break;
        }
    }
    Ok(())
}

fn constrain_query_to_scope(
    query: Box<dyn Query>,
    file_path_field: tantivy::schema::Field,
    scope_filter: Option<&WorkspaceScope>,
) -> Option<Box<dyn Query>> {
    let Some(scope) = scope_filter else {
        return Some(query);
    };

    let scope_path = index_path_string(&scope.rel_path);
    let path_query: Box<dyn Query> = if scope.is_file {
        Box::new(TermQuery::new(
            tantivy::Term::from_field_text(file_path_field, &scope_path),
            IndexRecordOption::Basic,
        ))
    } else {
        let prefix = format!("{}/", regex::escape(&scope_path));
        Box::new(RegexQuery::from_pattern(&format!("{prefix}.*"), file_path_field).ok()?)
    };

    Some(Box::new(BooleanQuery::new(vec![
        (Occur::Must, query),
        (Occur::Must, path_query),
    ])))
}

/// Lossy decoding keeps lines with invalid UTF-8, so binary files need an
/// explicit filter: like grep, stop searching a file once a NUL byte is read.
fn text_searcher() -> Searcher {
    SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build()
}

/// Bounds a matching line, keeping its first match inside the preview window.
fn regex_preview_line(matcher: &RegexMatcher, line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= crate::search::MAX_PREVIEW_LINE_BYTES {
        return trimmed.to_string();
    }
    let trimmed_start = line.len() - line.trim_start().len();
    let trimmed_end = trimmed_start + trimmed.len();
    let found = matcher.find(line.as_bytes()).ok().flatten().map(|found| {
        found.start().clamp(trimmed_start, trimmed_end) - trimmed_start
            ..found.end().clamp(trimmed_start, trimmed_end) - trimmed_start
    });
    crate::search::preview_line_window(trimmed, found).into_owned()
}

#[derive(Clone, Copy)]
enum RegexLines {
    Count,
    Hits(usize),
}

fn scan_regex_file(
    files: &RootHandle,
    path: &Path,
    matcher: &RegexMatcher,
    searcher: &mut Searcher,
    options: &SearchOptions,
    lines: RegexLines,
) -> Result<Option<FileSearchResult>> {
    if options.is_cancelled() {
        return Ok(None);
    }
    let Ok(file) = files.open(path) else {
        return Ok(None);
    };
    // Walked files and indexed candidates share the same relative path in JSON.
    #[cfg(windows)]
    let path = PathBuf::from(index_path_string(path));
    let mut hits = Vec::new();
    let mut hit_count = 0;
    let mut total_score = 0.0;
    searcher.search_file(
        matcher,
        &file,
        Bytes(|line_num, line| {
            if options.is_cancelled() {
                return Ok(false);
            }
            hit_count += 1;
            total_score += 1.0;
            if let RegexLines::Hits(limit) = lines {
                let line_num = usize::try_from(line_num).unwrap_or(usize::MAX);
                hits.push(SearchHit {
                    file_path: path.to_owned(),
                    start_line: line_num,
                    end_line: line_num,
                    preview: regex_preview_line(matcher, &String::from_utf8_lossy(line)),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
                return Ok(hit_count < limit);
            }
            Ok(true)
        }),
    )?;
    Ok((hit_count > 0).then(|| FileSearchResult {
        file_path: path.to_owned(),
        total_score,
        hit_count,
        more_hits_in_file: hit_count - hits.len(),
        hit_count_is_lower_bound: false,
        hits,
    }))
}

/// Parallel regex search over a known set of file paths.
fn regex_search_parallel(
    workspace: &Workspace,
    pattern: &str,
    file_paths: &[PathBuf],
    max_hits: usize,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;
    let merge_hits = |mut left: Vec<SearchHit>, right: Vec<SearchHit>| {
        left.extend(right);
        left.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
        });
        left.truncate(max_hits);
        left
    };
    // The first read walks the root. Every other candidate file reuses that walk.
    let files = RootHandle::new(&workspace.root);
    let search_file = |rel_path: &PathBuf| {
        scan_regex_file(
            &files,
            rel_path,
            &matcher,
            &mut text_searcher(),
            options,
            RegexLines::Hits(max_hits),
        )
        .ok()
        .flatten()
        .map(|group| group.hits)
        .unwrap_or_default()
    };

    if max_hits == usize::MAX {
        // Unbounded requests read every candidate; sort once instead of per merge.
        let mut hits = file_paths
            .par_iter()
            .flat_map_iter(search_file)
            .collect::<Vec<_>>();
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        hits.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
        });
        return Ok(hits);
    }

    // Candidate paths are sorted. Finishing each batch before the next keeps a
    // limited result equal to the first matches in path order, while a full
    // result set can still stop before reading later batches.
    let mut hits = Vec::new();
    for batch in file_paths.chunks(REGEX_PARALLEL_BATCH_FILES) {
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        let batch_hits = batch
            .par_iter()
            .map(search_file)
            .fold(Vec::new, merge_hits)
            .reduce(Vec::new, merge_hits);
        hits = merge_hits(hits, batch_hits);
        if hits.len() >= max_hits {
            break;
        }
    }
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    Ok(hits)
}

/// Visit files in path order so a bounded search can stop after the last requested line.
fn regex_search_walk(
    workspace: &Workspace,
    pattern: &str,
    max_hits: usize,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;
    let mut searcher = text_searcher();
    let files = RootHandle::new(&workspace.root);
    let mut hits = Vec::new();
    walk_regex_paths(
        workspace,
        path_matcher,
        options,
        true,
        |rel, type_filter| {
            let remaining = max_hits - hits.len();
            if let Some(group) = scan_regex_file(
                &files,
                rel,
                &matcher,
                &mut searcher,
                options,
                RegexLines::Hits(remaining),
            )? && (type_filter != PathTypeFilterMatch::ValidateText
                || unknown_file_is_indexable_text(&files, rel))
            {
                hits.extend(group.hits);
            }
            Ok(hits.len() < max_hits)
        },
    )?;
    Ok(hits)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathTypeFilterMatch {
    Match,
    ValidateText,
    Reject,
}

pub(crate) fn type_filter_matches_language(language: &str, filter: &str) -> bool {
    let expected = crate::chunking::resolve_type_alias(filter).unwrap_or(filter);
    language.eq_ignore_ascii_case(expected)
}

fn type_filter_match_for_path(
    path: &std::path::Path,
    type_filter: Option<&str>,
) -> PathTypeFilterMatch {
    let Some(filter) = type_filter else {
        return PathTypeFilterMatch::Match;
    };
    let expected = crate::chunking::resolve_type_alias(filter).unwrap_or(filter);
    match crate::chunking::language_for_path(path) {
        Some(language) if language.eq_ignore_ascii_case(expected) => PathTypeFilterMatch::Match,
        Some(_) => PathTypeFilterMatch::Reject,
        None if expected.eq_ignore_ascii_case("text") => PathTypeFilterMatch::ValidateText,
        None => PathTypeFilterMatch::Reject,
    }
}

fn unknown_file_is_indexable_text(files: &RootHandle, path: &std::path::Path) -> bool {
    let Ok(mut file) = files.open(path) else {
        return false;
    };
    crate::chunking::is_indexable_file_reader(path, &mut file).unwrap_or(false)
}

pub(crate) fn matches_live_file_type(
    files: &RootHandle,
    path: &std::path::Path,
    filter: Option<&str>,
) -> bool {
    match type_filter_match_for_path(path, filter) {
        PathTypeFilterMatch::Match => true,
        PathTypeFilterMatch::Reject => false,
        PathTypeFilterMatch::ValidateText => unknown_file_is_indexable_text(files, path),
    }
}

fn expand_regex_context(
    workspace: &Workspace,
    hits: &mut [SearchHit],
    context: usize,
    options: &SearchOptions,
) {
    let files = RootHandle::new(&workspace.root);
    expand_regex_context_with_paths(hits, context, Some(options), |path| files.open(path));
}

pub(crate) fn expand_regex_context_absolute(
    hits: &mut [SearchHit],
    context: usize,
    roots: &[PathBuf],
) {
    expand_regex_context_with_paths(hits, context, None, |path| {
        open_absolute_workspace_file(path, roots)
    });
}

pub(crate) fn expand_regex_context_absolute_with_options(
    hits: &mut [SearchHit],
    context: usize,
    roots: &[PathBuf],
    options: &SearchOptions,
) {
    expand_regex_context_with_paths(hits, context, Some(options), |path| {
        open_absolute_workspace_file(path, roots)
    });
}

fn open_absolute_workspace_file(
    path: &std::path::Path,
    roots: &[PathBuf],
) -> std::io::Result<fs::File> {
    let root = roots
        .iter()
        .find(|root| path.starts_with(root))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "path is outside searched workspaces",
            )
        })?;
    crate::workspace_file::open(root, path)
}

fn expand_regex_context_with_paths(
    hits: &mut [SearchHit],
    context: usize,
    options: Option<&SearchOptions>,
    open_file: impl Fn(&std::path::Path) -> std::io::Result<fs::File>,
) {
    if context == 0 || hits.is_empty() {
        return;
    }

    let mut hits_by_path = BTreeMap::<PathBuf, Vec<usize>>::new();
    for (index, hit) in hits.iter().enumerate() {
        hits_by_path
            .entry(hit.file_path.clone())
            .or_default()
            .push(index);
    }

    for (rel_path, hit_indices) in hits_by_path {
        if options.is_some_and(SearchOptions::is_cancelled) {
            return;
        }
        let Ok(file) = open_file(&rel_path) else {
            continue;
        };
        if file
            .metadata()
            .ok()
            .is_none_or(|metadata| metadata.len() > MAX_CONTEXT_FILE_BYTES)
        {
            continue;
        }
        let mut bytes = Vec::new();
        let Ok(bytes_read) = file
            .take(MAX_CONTEXT_FILE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
        else {
            continue;
        };
        if bytes_read as u64 > MAX_CONTEXT_FILE_BYTES {
            continue;
        }
        let content = crate::workspace_file::lossy_string(bytes);
        let lines = content.lines().collect::<Vec<_>>();
        if lines.is_empty() {
            continue;
        }
        for hit_index in hit_indices {
            if options.is_some_and(SearchOptions::is_cancelled) {
                return;
            }
            let hit = &mut hits[hit_index];
            let focus = hit.start_line.clamp(1, lines.len());
            let start = focus.saturating_sub(context).max(1);
            let end = focus.saturating_add(context).min(lines.len());
            // The search already windowed a long matching line around its match.
            let matched = std::mem::take(&mut hit.preview);
            hit.start_line = start;
            hit.end_line = end;
            hit.preview = lines[start.saturating_sub(1)..end]
                .iter()
                .enumerate()
                .map(|(offset, line)| {
                    if start + offset == focus
                        && line.len() > crate::search::MAX_PREVIEW_LINE_BYTES
                        && !matched.contains('\n')
                    {
                        std::borrow::Cow::Borrowed(matched.as_str())
                    } else {
                        crate::search::preview_line_window(line, None)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    use serial_test::serial;

    use super::*;
    use crate::EMBEDDING_DIMENSIONS;
    use crate::embedding::HashEmbeddingModel;
    use crate::indexer::index_workspace;
    use crate::workspace::{Workspace, WorkspaceScope};

    fn test_regex_search(
        workspace: &Workspace,
        pattern: &str,
        limit: Option<usize>,
        scope_filter: Option<&WorkspaceScope>,
        include_globs: &[String],
        exclude_globs: &[String],
        skip_gitignore: bool,
    ) -> Result<Vec<SearchHit>> {
        regex_search_with_options(
            workspace,
            pattern,
            &SearchOptions {
                limit,
                context: 0,
                scope_filter: scope_filter.cloned(),
                include_globs: include_globs.to_vec(),
                exclude_globs: exclude_globs.to_vec(),
                skip_gitignore,
                ..Default::default()
            },
        )
    }

    #[test]
    fn regex_preview_uses_the_untrimmed_match_span() {
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build(r"needle\s+$")
            .unwrap();
        let line = format!("{}needle   \n", "a".repeat(2_000));

        let preview = regex_preview_line(&matcher, &line);

        assert!(preview.starts_with('…'));
        assert!(preview.contains("needle"));
        assert!(preview.len() <= crate::search::MAX_PREVIEW_LINE_BYTES + '…'.len_utf8());
    }

    #[test]
    fn required_literal_runs_ignore_optional_and_alternative_text() {
        assert_eq!(
            required_literal_runs("func.*DDSQLizer").unwrap(),
            vec!["qlizer".to_string(), "func".to_string()]
        );
        assert_eq!(
            required_literal_runs("SELECT.*FROM.*WHERE").unwrap(),
            vec!["elect".to_string(), "where".to_string(), "from".to_string()]
        );
        assert_eq!(
            required_literal_runs("hello_world").unwrap(),
            vec!["hello".to_string(), "world".to_string()]
        );
        assert_eq!(
            required_literal_runs(r"cache(_token)?").unwrap(),
            vec!["cache".to_string()]
        );
        assert!(required_literal_runs("error|warning").is_none());
        assert!(required_literal_runs("[abcdef]{200}").is_none());
    }

    #[test]
    #[serial]
    fn regex_search_discards_results_when_pre_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join("match.rs"), "fn cancelled_match() {}\n").unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let options = SearchOptions {
            cancel_token: Some(std::sync::Arc::new(AtomicBool::new(true))),
            ..SearchOptions::default()
        };

        let hits = regex_search_with_options(&workspace, "cancelled_match", &options).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    #[serial]
    fn regex_search_matches_lines_with_invalid_utf8() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("latin1.py"),
            b"# header\ncaf\xe9 = \"rotate_latin1_secret\"\n# footer\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("blob.bin"),
            b"rotate_latin1_secret\0\x01\x02\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();

        for indexed in [false, true] {
            if indexed {
                index_workspace(&workspace, &HashEmbeddingModel::new(EMBEDDING_DIMENSIONS))
                    .unwrap();
            }
            let options = SearchOptions {
                context: 1,
                ..SearchOptions::default()
            };
            let hits =
                regex_search_with_options(&workspace, "rotate_latin1_secre.", &options).unwrap();
            assert_eq!(hits.len(), 1, "indexed={indexed}");
            assert_eq!(hits[0].start_line, 1, "indexed={indexed}");
            assert_eq!(hits[0].end_line, 3, "indexed={indexed}");
            assert!(hits[0].preview.contains("# footer"), "indexed={indexed}");
        }
    }

    #[test]
    #[serial]
    fn regex_search_respects_scope_filter() {
        let tmp = tempfile::tempdir().unwrap();
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
        let scope = WorkspaceScope {
            rel_path: PathBuf::from("scoped"),
            is_file: false,
        };

        let hits = test_regex_search(
            &workspace,
            "applyFilter",
            None,
            Some(&scope),
            &[],
            &[],
            false,
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
    fn regex_search_respects_include_exclude_globs() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("match.rs"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("match.md"),
            "pub fn applyFilter() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let include = vec!["*.md".to_string()];
        let exclude = vec!["match.md".to_string()];

        let include_only =
            test_regex_search(&workspace, "applyFilter", None, None, &include, &[], false).unwrap();
        assert_eq!(
            include_only
                .iter()
                .map(|hit| hit.file_path.clone())
                .collect::<std::collections::HashSet<_>>(),
            [PathBuf::from("match.md")]
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
        );

        let include_and_exclude = test_regex_search(
            &workspace,
            "applyFilter",
            None,
            None,
            &include,
            &exclude,
            false,
        )
        .unwrap();
        assert!(include_and_exclude.is_empty());
    }

    #[test]
    #[serial]
    fn indexed_regex_search_respects_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        for i in 0..600 {
            std::fs::write(
                tmp.path().join(format!("match_{i:03}.rs")),
                format!("pub fn applyFilter_{i}() -> bool {{ true }}\n"),
            )
            .unwrap();
        }
        let expected = ["match_000.rs", "match_001.rs", "match_002.rs"]
            .map(PathBuf::from)
            .to_vec();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        // Walk fallback first, then index-backed parallel verification.
        for indexed in [false, true] {
            if indexed {
                index_workspace(&workspace, &HashEmbeddingModel::new(EMBEDDING_DIMENSIONS))
                    .unwrap();
            }
            for _ in 0..10 {
                let hits = test_regex_search(
                    &workspace,
                    r"applyFilter_\d+",
                    Some(3),
                    None,
                    &[],
                    &[],
                    false,
                )
                .unwrap();
                let paths = hits
                    .into_iter()
                    .map(|hit| hit.file_path)
                    .collect::<Vec<_>>();
                assert_eq!(paths, expected, "indexed={indexed}");
            }
        }
    }

    #[test]
    #[serial]
    fn indexed_regex_scope_survives_global_candidate_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::create_dir_all(tmp.path().join("scoped")).unwrap();
        std::fs::create_dir_all(tmp.path().join("other")).unwrap();
        for i in 0..10_050 {
            std::fs::write(
                tmp.path().join("other").join(format!("noise_{i:05}.txt")),
                "targettoken targettoken targettoken targettoken\n",
            )
            .unwrap();
        }
        std::fs::write(tmp.path().join("scoped/match.txt"), "targettoken\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = test_regex_search(
            &workspace,
            "targettoken",
            Some(1),
            Some(&WorkspaceScope {
                rel_path: PathBuf::from("scoped"),
                is_file: false,
            }),
            &[],
            &[],
            false,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("scoped/match.txt"));
    }

    #[test]
    #[serial]
    fn regex_alternation_finds_files_matching_any_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        // Each file contains only ONE branch of the alternation.
        std::fs::write(
            tmp.path().join("e.rs"),
            "fn f() { let error_branch = 1; }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("w.rs"),
            "fn f() { let warning_branch = 2; }\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("c.rs"),
            "fn f() { let critical_branch = 3; }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        // The index prefilter must not drop files matching only one branch.
        let hits = test_regex_search(
            &workspace,
            "error_branch|warning_branch|critical_branch",
            None,
            None,
            &[],
            &[],
            false,
        )
        .unwrap();

        let files: std::collections::HashSet<String> = hits
            .iter()
            .map(|h| h.file_path.to_string_lossy().to_string())
            .collect();
        assert!(
            files.contains("e.rs"),
            "must find error branch file; got {files:?}"
        );
        assert!(
            files.contains("w.rs"),
            "must find warning branch file; got {files:?}"
        );
        assert!(
            files.contains("c.rs"),
            "must find critical branch file; got {files:?}"
        );
    }

    #[test]
    #[serial]
    fn indexed_regex_optional_group_does_not_require_optional_literal() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("cache.rs"),
            "const NAME: &str = \"cache\";\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits =
            test_regex_search(&workspace, r"cache(_token)?", None, None, &[], &[], false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("cache.rs"));
    }

    #[test]
    #[serial]
    fn indexed_regex_finds_literal_inside_identifier_token() {
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

        let hits = test_regex_search(&workspace, "ppl", None, None, &[], &[], false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("filter.rs"));
    }

    #[test]
    #[serial]
    fn indexed_regex_respects_gitignore_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(tmp.path().join("visible.rs"), "fn visible_marker() {}\n").unwrap();
        std::fs::write(tmp.path().join("ignored.rs"), "fn ignored_marker() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&crate::workspace::WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: None,
                watch_enabled: false,
                skip_gitignore: true,
                index_generation: 0,
            })
            .unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let default_hits =
            test_regex_search(&workspace, "marker", None, None, &[], &[], false).unwrap();
        assert_eq!(default_hits.len(), 1);
        assert_eq!(default_hits[0].file_path, PathBuf::from("visible.rs"));

        let all_hits = test_regex_search(&workspace, "marker", None, None, &[], &[], true).unwrap();
        assert_eq!(all_hits.len(), 2);
    }

    #[test]
    #[serial]
    fn indexed_regex_applies_type_filter_and_context() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("match.md"),
            "before\nrelease_marker = true\nafter\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("match.rs"),
            "before\nconst RELEASE_MARKER: bool = true;\nafter\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

        let hits = regex_search_with_options(
            &workspace,
            "release_marker",
            &SearchOptions {
                context: 1,
                type_filter: Some("md".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("match.md"));
        assert_eq!((hits[0].start_line, hits[0].end_line), (1, 3));
        assert_eq!(hits[0].preview, "before\nrelease_marker = true\nafter");
    }

    #[test]
    #[serial]
    fn regex_walk_applies_type_filter_and_bounds_extreme_context() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("match.md"),
            "before\nwalk_marker = true\nafter\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("match.rs"),
            "before\nconst WALK_MARKER: bool = true;\nafter\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();

        let hits = regex_search_with_options(
            &workspace,
            "walk_marker|other_branch",
            &SearchOptions {
                context: usize::MAX,
                type_filter: Some("markdown".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("match.md"));
        assert_eq!((hits[0].start_line, hits[0].end_line), (1, 3));
        assert_eq!(hits[0].preview, "before\nwalk_marker = true\nafter");
    }

    #[test]
    #[serial]
    fn regex_text_filter_includes_unknown_text_extensions_with_and_without_index() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            tmp.path().join("notes.memo"),
            "before\nunknown_extension_marker\nafter\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("binary.memo"),
            b"unknown_extension_marker\0binary",
        )
        .unwrap();
        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let options = SearchOptions {
            context: 1,
            type_filter: Some("text".to_string()),
            ..Default::default()
        };

        let walk_hits = regex_search_with_options(
            &workspace,
            "unknown_extension_marker|other_branch",
            &options,
        )
        .unwrap();
        assert_eq!(walk_hits.len(), 1);
        assert_eq!(walk_hits[0].file_path, PathBuf::from("notes.memo"));
        assert_eq!(
            walk_hits[0].preview,
            "before\nunknown_extension_marker\nafter"
        );

        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let indexed_hits =
            regex_search_with_options(&workspace, "unknown_extension_marker", &options).unwrap();
        assert_eq!(indexed_hits.len(), 1);
        assert_eq!(indexed_hits[0].file_path, PathBuf::from("notes.memo"));
        assert_eq!(
            indexed_hits[0].preview,
            "before\nunknown_extension_marker\nafter"
        );
        for query in ["unknown_extension_marker", "_"] {
            let hits = crate::search::literal_search(&workspace, query, &options).unwrap();
            assert_eq!(hits.len(), 1, "{query}: {hits:?}");
            assert_eq!(hits[0].file_path, PathBuf::from("notes.memo"));
        }
    }

    #[test]
    #[serial]
    fn indexed_regex_includes_minified_files_outside_lexical_index() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            root.path().join("indexed.rs"),
            "pub fn shared_regex_marker() {}\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("minified.js"),
            format!("{}shared_regex_marker", "a".repeat(50_001)),
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let hits =
            regex_search_with_options(&workspace, "shared_regex_marker", &SearchOptions::default())
                .unwrap();
        let paths = hits
            .iter()
            .map(|hit| hit.file_path.as_path())
            .collect::<HashSet<_>>();

        assert!(paths.contains(std::path::Path::new("indexed.rs")));
        assert!(paths.contains(std::path::Path::new("minified.js")));
    }

    #[test]
    #[serial]
    fn indexed_regex_finds_new_files_until_live_ignore_rules_exclude_them() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            root.path().join("indexed.rs"),
            "pub fn live_rule_marker() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        std::fs::write(
            root.path().join("created.rs"),
            "pub fn live_rule_marker() {}\n",
        )
        .unwrap();
        let regex_paths = || {
            regex_search_with_options(&workspace, "live_rule_marker", &SearchOptions::default())
                .unwrap()
                .into_iter()
                .map(|hit| hit.file_path)
                .collect::<HashSet<_>>()
        };

        // Files created since the last publication stay visible, as without an index.
        assert_eq!(
            regex_paths(),
            HashSet::from([PathBuf::from("created.rs"), PathBuf::from("indexed.rs")])
        );
        // New ignore rules take effect without waiting for another publication.
        std::fs::write(root.path().join(".gitignore"), "created.rs\n").unwrap();
        assert_eq!(regex_paths(), HashSet::from([PathBuf::from("indexed.rs")]));
    }

    #[test]
    #[serial]
    fn regex_uses_live_contents_and_ignore_rules_after_indexing() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let changed = root.path().join("changed.rs");
        std::fs::write(&changed, "fn old_value() {}\n").unwrap();
        std::fs::write(root.path().join("ignored.rs"), "fn edited_marker() {}\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        index_workspace(&workspace, &HashEmbeddingModel::new(EMBEDDING_DIMENSIONS)).unwrap();

        let paths = |pattern| {
            regex_search_with_options(&workspace, pattern, &SearchOptions::default())
                .unwrap()
                .into_iter()
                .map(|hit| hit.file_path)
                .collect::<Vec<_>>()
        };
        assert_eq!(paths("edited_marker"), [PathBuf::from("ignored.rs")]);
        std::fs::write(&changed, "fn edited_marker() {}\n").unwrap();
        std::fs::write(root.path().join(".gitignore"), "ignored.rs\n").unwrap();
        assert_eq!(paths("edited_marker"), [PathBuf::from("changed.rs")]);
        assert_eq!(paths("(?i:edited_marker)"), paths("edited_marker"));
    }

    #[test]
    #[serial]
    fn regex_file_names_count_every_matching_line_before_limiting_files() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(root.path().join("a.txt"), "marker\n").unwrap();
        std::fs::write(root.path().join("b.txt"), "marker\nmarker marker\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let options = SearchOptions {
            limit: Some(1),
            context: 3,
            ..SearchOptions::default()
        };
        for indexed in [false, true] {
            if indexed {
                index_workspace(&workspace, &HashEmbeddingModel::new(EMBEDDING_DIMENSIONS))
                    .unwrap();
            }
            let result =
                regex_file_name_search_with_options(&workspace, "marker", &options).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].file_path, PathBuf::from("b.txt"));
            assert_eq!(result[0].hit_count, 2);
            assert_eq!(result[0].total_score, 2.0);
            assert!(result[0].hits.is_empty());
        }
    }

    #[test]
    #[serial]
    fn direct_regex_file_scope_respects_nested_ignore_rules() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join(".gitignore"), "*.rs\n").unwrap();
        std::fs::write(root.path().join("src/.gitignore"), "!kept.rs\n").unwrap();
        for name in ["kept.rs", "hidden.rs"] {
            std::fs::write(root.path().join("src").join(name), "fn marker() {}\n").unwrap();
        }
        let workspace = Workspace::resolve(root.path()).unwrap();
        for (name, count) in [("kept.rs", 1), ("hidden.rs", 0)] {
            let options = SearchOptions {
                scope_filter: Some(WorkspaceScope {
                    rel_path: PathBuf::from("src").join(name),
                    is_file: true,
                }),
                ..SearchOptions::default()
            };
            let hits = regex_search_with_options(&workspace, "marker", &options).unwrap();
            assert_eq!(hits.len(), count);
        }
    }

    #[test]
    #[serial]
    fn indexed_regex_bypasses_warmed_coverage_during_incomplete_publication() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let source = root.path().join("indexed.rs");
        std::fs::write(&source, "pub fn committed_regex_marker() {}\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let options = SearchOptions::default();
        let before =
            regex_search_with_options(&workspace, "committed_regex_marker", &options).unwrap();
        assert_eq!(before.len(), 1);

        std::fs::write(&source, "pub fn pending_regex_marker() {}\n").unwrap();
        std::fs::write(
            workspace.indexing_incomplete_path(),
            "publication pending\n",
        )
        .unwrap();
        let hits = regex_search_with_options(&workspace, "pending_regex_marker", &options).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("indexed.rs"));
        assert!(hits[0].preview.contains("pending_regex_marker"));
    }

    #[test]
    #[serial]
    fn indexed_regex_includes_files_larger_than_indexing_limit() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(
            root.path().join("indexed.rs"),
            "pub fn ordinary_source() {}\n",
        )
        .unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        let before = regex_search_with_options(
            &workspace,
            "oversized_regex_marker",
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(before.is_empty());
        let mut oversized = "padding line\n".repeat(1_400_000);
        oversized.push_str("oversized_regex_marker\n");
        std::fs::write(root.path().join("oversized.log"), oversized).unwrap();
        index_workspace(&workspace, &model).unwrap();
        let hits = regex_search_with_options(
            &workspace,
            "oversized_regex_marker",
            &SearchOptions::default(),
        )
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, PathBuf::from("oversized.log"));
    }
}
