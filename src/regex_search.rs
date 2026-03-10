use anyhow::Result;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{Searcher, SearcherBuilder};
use ignore::WalkBuilder;

use crate::protocol::SearchHit;
use crate::workspace::{Workspace, WorkspaceScope};

pub fn regex_search(
    workspace: &Workspace,
    pattern: &str,
    limit: Option<usize>,
    scope_filter: Option<&WorkspaceScope>,
) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcher::new(pattern)?;
    let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();
    let max_hits = limit.unwrap_or(usize::MAX);

    let mut hits = Vec::new();

    let mut walk = WalkBuilder::new(&workspace.root);
    walk.hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .ignore(true)
        .require_git(false)
        .follow_links(false);

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

        let hits = regex_search(&workspace, "applyFilter", None, Some(&scope)).unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits.iter()
                .all(|hit| hit.file_path.starts_with(std::path::Path::new("scoped")))
        );
    }
}
