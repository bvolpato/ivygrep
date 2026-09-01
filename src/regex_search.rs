use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{Searcher, SearcherBuilder};
use rayon::prelude::*;
use regex_syntax::hir::{Hir, HirKind};
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, RegexQuery, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::schema::Value;

use crate::indexer::{open_sqlite_readonly, open_tantivy_index};
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::search::SearchOptions;
use crate::workspace::{Workspace, WorkspaceScope, index_path_string};

const MAX_CONTEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_REGEX_COVERAGE_CACHE_ENTRIES: usize = 32;
const MAX_REGEX_UNINDEXED_FILES: usize = 4_096;

#[derive(Clone, Eq, Hash, PartialEq)]
struct RegexCoverageKey {
    workspace_id: String,
    index_generation: u64,
    base_generation: u64,
    skip_gitignore: bool,
}

fn regex_coverage_cache() -> &'static Mutex<HashMap<RegexCoverageKey, Arc<Vec<PathBuf>>>> {
    static CACHE: OnceLock<Mutex<HashMap<RegexCoverageKey, Arc<Vec<PathBuf>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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

    let uncovered = uncovered_regex_paths(workspace, use_overlay, &shadowed_paths, options)?;
    for rel in uncovered.iter() {
        if options.is_cancelled() {
            return Some(Vec::new());
        }
        let type_match = type_filter_match_for_path(rel, options.type_filter.as_deref());
        if scope_filter.is_none_or(|scope| scope.matches(rel))
            && path_matcher.matches(rel)
            && type_match != PathTypeFilterMatch::Reject
            && (type_match != PathTypeFilterMatch::ValidateText
                || unknown_file_is_indexable_text(&workspace.root, rel))
        {
            candidate_files.insert(rel.clone());
        }
    }

    let mut paths: Vec<PathBuf> = candidate_files.into_iter().collect();
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

fn uncovered_regex_paths(
    workspace: &Workspace,
    use_overlay: bool,
    shadowed_paths: &HashSet<String>,
    options: &SearchOptions,
) -> Option<Arc<Vec<PathBuf>>> {
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
    let cache_key = RegexCoverageKey {
        workspace_id: workspace.id.clone(),
        index_generation,
        base_generation,
        skip_gitignore: options.skip_gitignore,
    };
    if let Some(paths) = regex_coverage_cache().lock().ok()?.get(&cache_key) {
        return Some(Arc::clone(paths));
    }

    let sqlite_path = if use_overlay {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    let sqlite = open_sqlite_readonly(&sqlite_path).ok()?;
    let mut indexed_paths = HashSet::new();
    collect_sqlite_paths(
        &sqlite,
        "SELECT DISTINCT file_path FROM chunks",
        &mut indexed_paths,
    )?;
    if use_overlay {
        let base =
            open_sqlite_readonly(&workspace.base_index_dir.as_ref()?.join("metadata.sqlite3"))
                .ok()?;
        let mut base_paths = HashSet::new();
        collect_sqlite_paths(
            &base,
            "SELECT DISTINCT file_path FROM chunks",
            &mut base_paths,
        )?;
        indexed_paths.extend(
            base_paths
                .into_iter()
                .filter(|path| !shadowed_paths.contains(path)),
        );
    }

    let mut uncovered = Vec::new();
    for entry in crate::walker::source_walker(&workspace.root, options.skip_gitignore).build() {
        if options.is_cancelled() {
            return Some(Arc::new(Vec::new()));
        }
        let entry = entry.ok()?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative = entry.path().strip_prefix(&workspace.root).ok()?;
        if !indexed_paths.contains(&index_path_string(relative)) {
            uncovered.push(relative.to_path_buf());
            if uncovered.len() > MAX_REGEX_UNINDEXED_FILES {
                return None;
            }
        }
    }
    uncovered.sort();
    let uncovered = Arc::new(uncovered);
    let mut cache = regex_coverage_cache().lock().ok()?;
    if cache.len() >= MAX_REGEX_COVERAGE_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(cache_key, Arc::clone(&uncovered));
    Some(uncovered)
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

/// Parallel regex search over a known set of file paths.
fn regex_search_parallel(
    workspace: &Workspace,
    pattern: &str,
    file_paths: &[PathBuf],
    max_hits: usize,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let hit_count = AtomicUsize::new(0);
    let done = AtomicBool::new(false);
    let results = Mutex::new(Vec::new());

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;

    file_paths.par_iter().for_each(|rel_path| {
        if done.load(Ordering::Relaxed) || options.is_cancelled() {
            return;
        }

        let Ok(file) = crate::workspace_file::open(&workspace.root, rel_path) else {
            return;
        };

        let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();

        let mut local_hits = Vec::new();
        let _ = searcher.search_file(
            &matcher,
            &file,
            UTF8(|line_num, line| {
                if options.is_cancelled() {
                    return Ok(false);
                }
                let line_num = usize::try_from(line_num).unwrap_or(usize::MAX);
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num,
                    end_line: line_num,
                    preview: line.trim().to_string(),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                    neural_requested: false,
                    neural_executed: false,
                });
                Ok(local_hits.len() < max_hits
                    && !done.load(Ordering::Relaxed)
                    && !options.is_cancelled())
            }),
        );

        if !options.is_cancelled() && !local_hits.is_empty() {
            let n = local_hits.len();
            let mut guard = results.lock().unwrap();
            guard.extend(local_hits);
            let previous = hit_count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    Some(count.saturating_add(n))
                })
                .unwrap_or_else(|count| count);
            let total = previous.saturating_add(n);
            if total >= max_hits {
                done.store(true, Ordering::Relaxed);
            }
        }
    });

    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    let mut hits = results.into_inner().unwrap();
    // Parallel collection order is nondeterministic; sort by (path, line) so a
    // limited result set is stable across runs rather than an arbitrary subset.
    hits.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.start_line.cmp(&b.start_line))
    });
    hits.truncate(max_hits);
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
    let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();

    let mut hits = Vec::new();

    let walk = crate::walker::source_walker(&workspace.root, options.skip_gitignore);

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
            UTF8(|line_num, line| {
                if options.is_cancelled() {
                    return Ok(false);
                }
                let line_num = usize::try_from(line_num).unwrap_or(usize::MAX);
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num,
                    end_line: line_num,
                    preview: line.trim().to_string(),
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
        let mut content = String::new();
        let Ok(bytes_read) = file
            .take(MAX_CONTEXT_FILE_BYTES.saturating_add(1))
            .read_to_string(&mut content)
        else {
            continue;
        };
        if bytes_read as u64 > MAX_CONTEXT_FILE_BYTES {
            continue;
        }
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
            hit.start_line = start;
            hit.end_line = end;
            hit.preview = lines[start.saturating_sub(1)..end].join("\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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

        for i in 0..20 {
            std::fs::write(
                tmp.path().join(format!("match_{i}.rs")),
                format!("pub fn applyFilter_{i}() -> bool {{ true }}\n"),
            )
            .unwrap();
        }

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();

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
        assert_eq!(hits.len(), 3);
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
