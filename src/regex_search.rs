use anyhow::Result;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{Searcher, SearcherBuilder};
use ignore::WalkBuilder;

use crate::protocol::SearchHit;
use crate::workspace::Workspace;

pub fn regex_search(workspace: &Workspace, pattern: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let matcher = RegexMatcher::new(pattern)?;
    let mut searcher: Searcher = SearcherBuilder::new().line_number(true).build();

    let mut hits = Vec::new();

    let mut walk = WalkBuilder::new(&workspace.root);
    walk.hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .ignore(true)
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
                    score: 1.0,
                    sources: vec!["regex".to_string()],
                });
                Ok(true)
            }),
        )?;

        for hit in local_hits {
            hits.push(hit);
            if hits.len() >= limit {
                break 'walk;
            }
        }
    }

    Ok(hits)
}
