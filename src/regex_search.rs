use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
// Lossy decoding keeps matches on lines with invalid UTF-8; UTF8 aborts the file.
use grep_searcher::sinks::Lossy;
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
use crate::protocol::SearchHit;
use crate::search::SearchOptions;
use crate::walker::SourcePathMatcher;
use crate::workspace::{Workspace, WorkspaceScope, index_path_string};

const MAX_CONTEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_COVERAGE_CACHE_ENTRIES: usize = 32;
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

/// Files outside the lexical index that queries must read directly.
pub(crate) struct UnindexedPaths {
    /// Snapshot files stored without chunks, such as minified bundles. The
    /// indexer decided their visibility for this publication.
    pub(crate) recorded: Vec<PathBuf>,
    /// Regex only: files the walk found that the snapshot never recorded, such
    /// as files over the indexing size limit or created since publication.
    unrecorded: Vec<PathBuf>,
}

/// `None` records that a publication has too many unindexed files to
/// enumerate, so later queries skip the work instead of repeating it.
type Coverage = Option<Arc<UnindexedPaths>>;

fn coverage_cache() -> &'static Mutex<HashMap<CoverageKey, Coverage>> {
    static CACHE: OnceLock<Mutex<HashMap<CoverageKey, Coverage>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remember_coverage(key: CoverageKey, coverage: Coverage) -> Coverage {
    if let Ok(mut cache) = coverage_cache().lock() {
        if cache.len() >= MAX_COVERAGE_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, coverage.clone());
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
        regex_search_walk(
            workspace,
            pattern,
            max_hits,
            options.scope_filter.as_ref(),
            &path_matcher,
            options,
        )
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

fn required_literal_runs(pattern: &str) -> Option<Vec<String>> {
    let hir = regex_syntax::Parser::new().parse(pattern).ok()?;
    let mut literals = Vec::new();
    collect_required_literals(&hir, &mut literals)?;
    let mut runs = literals
        .into_iter()
        .flat_map(|literal| {
            literal
                .split(|byte: &u8| !byte.is_ascii_alphanumeric())
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
    let required_runs = required_literal_runs(pattern)?;
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

    candidate_files.extend(unindexed_matching_paths(
        workspace,
        scope_filter,
        path_matcher,
        options,
    )?);

    let mut paths: Vec<PathBuf> = candidate_files.into_iter().collect();
    paths.sort();
    Some(paths)
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

fn unindexed_matching_paths(
    workspace: &Workspace,
    scope_filter: Option<&WorkspaceScope>,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Option<Vec<PathBuf>> {
    let coverage = cached_unindexed_paths(workspace, CoverageConsumer::Regex, options)?;
    let candidates = coverage
        .recorded
        .iter()
        .map(|path| (path, true))
        .chain(coverage.unrecorded.iter().map(|path| (path, false)));
    // Unrecorded paths come from a walk cached per publication. Recheck live
    // ignore rules, so an exclude added since that walk hides them even when
    // reindexing finds nothing to publish.
    let mut source_paths = None;
    let mut paths = Vec::new();
    for (rel, recorded) in candidates {
        if options.is_cancelled() {
            return Some(Vec::new());
        }
        let type_match = type_filter_match_for_path(rel, options.type_filter.as_deref());
        if scope_filter.is_none_or(|scope| scope.matches(rel))
            && path_matcher.matches(rel)
            && type_match != PathTypeFilterMatch::Reject
            && (recorded
                || source_paths
                    .get_or_insert_with(|| {
                        SourcePathMatcher::new(&workspace.root, options.skip_gitignore)
                    })
                    .allows(rel)
                    .ok()?)
            && (type_match != PathTypeFilterMatch::ValidateText
                || unknown_file_is_indexable_text(&workspace.root, rel))
        {
            paths.push(rel.clone());
        }
    }
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
    cached_unindexed_paths(workspace, CoverageConsumer::Literal, options)
}

fn cached_unindexed_paths(
    workspace: &Workspace,
    consumer: CoverageConsumer,
    options: &SearchOptions,
) -> Coverage {
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();
    if use_overlay && workspace.worktree_overlay_is_stale().ok()? {
        return None;
    }
    let key = CoverageKey {
        workspace_id: workspace.id.clone(),
        consumer,
        skip_gitignore: options.skip_gitignore,
        publication: publication_stamp(workspace)?,
    };
    if let Some(coverage) = coverage_cache().lock().ok()?.get(&key) {
        return coverage.clone();
    }

    let snapshot = MerkleSnapshot::load(&workspace.merkle_snapshot_path()).ok()?;
    let covered = chunk_backed_paths(workspace, use_overlay)?;
    // Stores commit before the snapshot is saved. Never pair chunks from one
    // publication with the file list of another.
    if publication_stamp(workspace)? != key.publication {
        return None;
    }
    let mut recorded = Vec::new();
    for (path, hash) in &snapshot.files {
        if covered.contains(path) || (!options.skip_gitignore && hash.ends_with("-1")) {
            continue;
        }
        recorded.push(PathBuf::from(path));
        if recorded.len() > MAX_UNINDEXED_FILES {
            return remember_coverage(key, None);
        }
    }
    let unrecorded = match consumer {
        CoverageConsumer::Literal => {
            // Empty files and files with a NUL in the sniffed prefix cannot
            // produce a literal hit. Drop them once per publication instead of
            // reading every binary asset on each query.
            recorded = recorded
                .into_par_iter()
                .filter(|path| !options.is_cancelled() && may_contain_text(&workspace.root, path))
                .collect();
            Vec::new()
        }
        CoverageConsumer::Regex => {
            match unrecorded_regex_paths(workspace, &snapshot, recorded.len(), options)? {
                Some(unrecorded) => unrecorded,
                None => return remember_coverage(key, None),
            }
        }
    };
    if options.is_cancelled() {
        return None;
    }
    remember_coverage(
        key,
        Some(Arc::new(UnindexedPaths {
            recorded,
            unrecorded,
        })),
    )
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
fn may_contain_text(root: &std::path::Path, path: &std::path::Path) -> bool {
    let Ok(file) = crate::workspace_file::open(root, path) else {
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

/// Files the snapshot never recorded, found by walking with the query's ignore
/// rules: files over the indexing size limit, files created since publication,
/// and ignored files when a query skips ignore rules the index applied. The
/// outer `None` skips caching, for example after a walk error. The inner `None`
/// records too many unindexed files.
fn unrecorded_regex_paths(
    workspace: &Workspace,
    snapshot: &MerkleSnapshot,
    recorded: usize,
    options: &SearchOptions,
) -> Option<Option<Vec<PathBuf>>> {
    let mut unrecorded = Vec::new();
    for entry in crate::walker::source_walker(&workspace.root, options.skip_gitignore).build() {
        if options.is_cancelled() {
            return None;
        }
        let entry = entry.ok()?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative = entry.path().strip_prefix(&workspace.root).ok()?;
        if snapshot.files.contains_key(&index_path_string(relative)) {
            continue;
        }
        unrecorded.push(relative.to_path_buf());
        if recorded + unrecorded.len() > MAX_UNINDEXED_FILES {
            return Some(None);
        }
    }
    unrecorded.sort();
    Some(Some(unrecorded))
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
    let search_file = |rel_path: &PathBuf| {
        if options.is_cancelled() {
            return Vec::new();
        }
        let Ok(file) = crate::workspace_file::open(&workspace.root, rel_path) else {
            return Vec::new();
        };
        let mut searcher = text_searcher();
        let mut local_hits = Vec::new();
        let _ = searcher.search_file(
            &matcher,
            &file,
            Lossy(|line_num, line| {
                if options.is_cancelled() {
                    return Ok(false);
                }
                let line_num = usize::try_from(line_num).unwrap_or(usize::MAX);
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num,
                    end_line: line_num,
                    preview: regex_preview_line(&matcher, line),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
                Ok(local_hits.len() < max_hits && !options.is_cancelled())
            }),
        );
        local_hits
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

/// Fallback: sequential filesystem walk (for workspaces without an index).
fn regex_search_walk(
    workspace: &Workspace,
    pattern: &str,
    max_hits: usize,
    scope_filter: Option<&WorkspaceScope>,
    path_matcher: &PathGlobMatcher,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;
    let mut searcher = text_searcher();

    let mut hits = Vec::new();

    let mut walk = crate::walker::source_walker(&workspace.root, options.skip_gitignore);
    // Sorted traversal makes a limited result the first matches in path order.
    walk.sort_by_file_name(|left, right| left.cmp(right));

    'walk: for entry in walk.build() {
        if options.is_cancelled() {
            return Ok(Vec::new());
        }
        let entry = entry?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let full_path = entry.path().to_path_buf();
        let rel_path = match full_path.strip_prefix(&workspace.root) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => full_path.clone(),
        };
        if scope_filter.is_some_and(|scope| !scope.matches(&rel_path)) {
            continue;
        }
        if !path_matcher.matches(&rel_path) {
            continue;
        }
        let type_filter_match =
            type_filter_match_for_path(&rel_path, options.type_filter.as_deref());
        if type_filter_match == PathTypeFilterMatch::Reject {
            continue;
        }

        let remaining = max_hits.saturating_sub(hits.len());
        if remaining == 0 {
            break;
        }
        let mut local_hits = Vec::new();
        let Ok(file) = crate::workspace_file::open(&workspace.root, &rel_path) else {
            continue;
        };
        searcher.search_file(
            &matcher,
            &file,
            Lossy(|line_num, line| {
                if options.is_cancelled() {
                    return Ok(false);
                }
                let line_num = usize::try_from(line_num).unwrap_or(usize::MAX);
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num,
                    end_line: line_num,
                    preview: regex_preview_line(&matcher, line),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
                Ok(local_hits.len() < remaining && !options.is_cancelled())
            }),
        )?;

        if options.is_cancelled() {
            return Ok(Vec::new());
        }

        if type_filter_match == PathTypeFilterMatch::ValidateText
            && !local_hits.is_empty()
            && !unknown_file_is_indexable_text(&workspace.root, &rel_path)
        {
            continue;
        }

        for hit in local_hits {
            hits.push(hit);
            if hits.len() >= max_hits {
                break 'walk;
            }
        }
    }

    Ok(hits)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathTypeFilterMatch {
    Match,
    ValidateText,
    Reject,
}

fn type_filter_matches_language(language: &str, filter: &str) -> bool {
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

fn unknown_file_is_indexable_text(root: &std::path::Path, path: &std::path::Path) -> bool {
    let Ok(mut file) = crate::workspace_file::open(root, path) else {
        return false;
    };
    crate::chunking::is_indexable_file_reader(path, &mut file).unwrap_or(false)
}

fn expand_regex_context(
    workspace: &Workspace,
    hits: &mut [SearchHit],
    context: usize,
    options: &SearchOptions,
) {
    expand_regex_context_with_paths(hits, context, Some(options), |path| {
        crate::workspace_file::open(&workspace.root, path)
    });
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
            vec!["ddsqlizer".to_string(), "func".to_string()]
        );
        assert_eq!(
            required_literal_runs("SELECT.*FROM.*WHERE").unwrap(),
            vec![
                "select".to_string(),
                "where".to_string(),
                "from".to_string()
            ]
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
        // The cached walk must not keep a file an exclude added before reindexing hides.
        std::fs::write(root.path().join(".gitignore"), "created.rs\n").unwrap();
        assert_eq!(regex_paths(), HashSet::from([PathBuf::from("indexed.rs")]));
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
        let mut oversized = "padding line\n".repeat(1_400_000);
        oversized.push_str("oversized_regex_marker\n");
        std::fs::write(root.path().join("oversized.log"), oversized).unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
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
