use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{Searcher, SearcherBuilder};
use rayon::prelude::*;
use tantivy::TantivyDocument;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;

use crate::indexer::open_tantivy_index;
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::workspace::{Workspace, WorkspaceScope};

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
    let max_hits = limit.unwrap_or(usize::MAX);
    let path_matcher = PathGlobMatcher::new(include_globs, exclude_globs)?;

    // Try to use index-backed pre-filtering via literal extraction.
    let candidate_files = index_prefilter_files(
        workspace,
        pattern,
        scope_filter,
        &path_matcher,
        skip_gitignore,
    );

    if let Some(paths) = candidate_files {
        tracing::trace!(
            "regex index prefilter: {} candidate files from index",
            paths.len()
        );
        regex_search_parallel(workspace, pattern, &paths, max_hits)
    } else {
        regex_search_walk(
            workspace,
            pattern,
            max_hits,
            scope_filter,
            &path_matcher,
            skip_gitignore,
        )
    }
}

/// Extract literal fragments from a regex pattern.
///
/// Splits the pattern on common regex metacharacters and returns fragments
/// that are long enough to be useful as Tantivy query terms (≥ 3 chars).
/// Returns them sorted longest-first so the rarest (most selective) terms
/// are queried first.
fn extract_literal_fragments(pattern: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            // Escape sequence: take the next character literally
            '\\' if i + 1 < chars.len() => {
                let next = chars[i + 1];
                // \d, \w, \s, etc. — break the fragment
                if next.is_alphanumeric() && "dDwWsSbB".contains(next) {
                    if current.len() >= 3 {
                        fragments.push(current.clone());
                    }
                    current.clear();
                } else {
                    current.push(next);
                }
                i += 2;
            }
            // Metacharacters that break literal sequences
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' => {
                if current.len() >= 3 {
                    fragments.push(current.clone());
                }
                current.clear();
                i += 1;
            }
            // Regular literal character
            _ => {
                current.push(ch);
                i += 1;
            }
        }
    }
    if current.len() >= 3 {
        fragments.push(current);
    }

    // Sort longest first (most selective)
    fragments.sort_by_key(|f| std::cmp::Reverse(f.len()));
    fragments.dedup();
    fragments
}

/// Use the Tantivy index to find files containing the literal fragments
/// extracted from the regex pattern. Returns None if no index or no
/// usable literals.
fn index_prefilter_files(
    workspace: &Workspace,
    pattern: &str,
    scope_filter: Option<&WorkspaceScope>,
    path_matcher: &PathGlobMatcher,
    _skip_gitignore: bool,
) -> Option<Vec<PathBuf>> {
    let fragments = extract_literal_fragments(pattern);
    if fragments.is_empty() {
        return None;
    }

    let tantivy_dir = workspace.tantivy_dir();
    if !tantivy_dir.exists() {
        return None;
    }

    let (idx, fields) = open_tantivy_index(&tantivy_dir).ok()?;
    let reader = idx.reader().ok()?;
    let searcher = reader.searcher();

    // Query the text field for the longest (most selective) literal fragment
    let parser = QueryParser::for_index(&idx, vec![fields.text]);

    let search_term = &fragments[0];
    let query = parser.parse_query(search_term).ok()?;

    // Get up to 10K candidate chunks — we only need their file_path
    let docs = searcher
        .search(&query, &TopDocs::with_limit(10_000).order_by_score())
        .ok()?;

    let mut candidate_files = HashSet::new();
    for (_score, addr) in docs {
        if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
            && let Some(path_val) = doc.get_first(fields.file_path)
            && let Some(path_str) = path_val.as_str()
        {
            let rel = PathBuf::from(path_str);
            if scope_filter.is_none_or(|s| s.matches(&rel)) && path_matcher.matches(&rel) {
                candidate_files.insert(rel);
            }
        }
    }

    // If we have additional literal fragments, further filter by querying
    // for each additional fragment and intersecting
    for frag in fragments.iter().skip(1).take(2) {
        if candidate_files.len() <= 100 {
            break; // Already small enough
        }
        if let Ok(q2) = parser.parse_query(frag)
            && let Ok(docs2) = searcher.search(&q2, &TopDocs::with_limit(10_000).order_by_score())
        {
            let mut frag_files = HashSet::new();
            for (_score, addr) in docs2 {
                if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
                    && let Some(path_val) = doc.get_first(fields.file_path)
                    && let Some(path_str) = path_val.as_str()
                {
                    frag_files.insert(PathBuf::from(path_str));
                }
            }
            candidate_files = candidate_files.intersection(&frag_files).cloned().collect();
        }
    }

    let mut paths: Vec<PathBuf> = candidate_files.into_iter().collect();
    paths.sort();
    Some(paths)
}

/// Parallel regex search over a known set of file paths.
fn regex_search_parallel(
    workspace: &Workspace,
    pattern: &str,
    file_paths: &[PathBuf],
    max_hits: usize,
) -> Result<Vec<SearchHit>> {
    let hit_count = AtomicUsize::new(0);
    let done = AtomicBool::new(false);
    let results = Mutex::new(Vec::new());

    let regex_pattern = pattern.to_string();

    file_paths.par_iter().for_each(|rel_path| {
        if done.load(Ordering::Relaxed) {
            return;
        }

        let full_path = workspace.root.join(rel_path);
        if !full_path.is_file() {
            return;
        }

        let matcher = match RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build(&regex_pattern)
        {
            Ok(m) => m,
            Err(_) => return,
        };
        let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();

        let mut local_hits = Vec::new();
        let _ = searcher.search_path(
            &matcher,
            &full_path,
            UTF8(|line_num, line| {
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num as usize,
                    end_line: line_num as usize,
                    preview: line.trim().to_string(),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                });
                Ok(!done.load(Ordering::Relaxed))
            }),
        );

        if !local_hits.is_empty() {
            let n = local_hits.len();
            let mut guard = results.lock().unwrap();
            guard.extend(local_hits);
            let total = hit_count.fetch_add(n, Ordering::Relaxed) + n;
            if total >= max_hits {
                done.store(true, Ordering::Relaxed);
            }
        }
    });

    let mut hits = results.into_inner().unwrap();
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
    skip_gitignore: bool,
) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)?;
    let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();

    let mut hits = Vec::new();

    let walk = crate::walker::source_walker(&workspace.root, skip_gitignore);

    'walk: for entry in walk.build() {
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

        let mut local_hits = Vec::new();
        searcher.search_path(
            &matcher,
            &full_path,
            UTF8(|line_num, line| {
                local_hits.push(SearchHit {
                    file_path: rel_path.clone(),
                    start_line: line_num as usize,
                    end_line: line_num as usize,
                    preview: line.trim().to_string(),
                    reason: "regex line match".to_string(),
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                });
                Ok(true)
            }),
        )?;

        for hit in local_hits {
            hits.push(hit);
            if hits.len() >= max_hits {
                break 'walk;
            }
        }
    }

    Ok(hits)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serial_test::serial;

    use super::*;
    use crate::workspace::{Workspace, WorkspaceScope};

    #[test]
    fn test_extract_literal_fragments() {
        assert_eq!(
            extract_literal_fragments("func.*DDSQLizer"),
            vec!["DDSQLizer".to_string(), "func".to_string()]
        );
        assert_eq!(
            extract_literal_fragments("SELECT.*FROM.*WHERE"),
            vec![
                "SELECT".to_string(),
                "WHERE".to_string(),
                "FROM".to_string()
            ]
        );
        assert_eq!(extract_literal_fragments("a{2}"), Vec::<String>::new());
        assert_eq!(
            extract_literal_fragments("hello_world"),
            vec!["hello_world".to_string()]
        );
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

        let hits = regex_search(
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
            regex_search(&workspace, "applyFilter", None, None, &include, &[], false).unwrap();
        assert_eq!(
            include_only
                .iter()
                .map(|hit| hit.file_path.clone())
                .collect::<std::collections::HashSet<_>>(),
            [PathBuf::from("match.md")]
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
        );

        let include_and_exclude = regex_search(
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
}
