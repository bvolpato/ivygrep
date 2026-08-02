use std::path::Path;

use anyhow::{Result, bail};

use crate::protocol::SearchHit;
use crate::workspace::{Workspace, list_workspaces};

pub(crate) struct SearchWorkspaceSet {
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn select_search_workspaces(
    current: &Workspace,
    all_indices: bool,
) -> Result<SearchWorkspaceSet> {
    if !all_indices {
        return Ok(SearchWorkspaceSet {
            workspaces: vec![current.clone()],
            warnings: Vec::new(),
        });
    }

    select_all_indexed_workspaces(Workspace::resolve)
}

pub(crate) fn select_all_indexed_workspaces<F>(mut resolve: F) -> Result<SearchWorkspaceSet>
where
    F: FnMut(&Path) -> Result<Workspace>,
{
    let roots = list_workspaces()?
        .into_iter()
        .filter(|status| status.last_indexed_at_unix.is_some())
        .map(|status| status.root);
    Ok(resolve_workspace_roots(roots, &mut resolve))
}

fn resolve_workspace_roots(
    roots: impl IntoIterator<Item = std::path::PathBuf>,
    mut resolve: impl FnMut(&Path) -> Result<Workspace>,
) -> SearchWorkspaceSet {
    let mut workspaces = Vec::new();
    let mut warnings = Vec::new();
    for root in roots {
        match resolve(&root) {
            Ok(workspace) => workspaces.push(workspace),
            Err(err) => warnings.push(format!(
                "could not open indexed workspace {}: {err:#}",
                root.display()
            )),
        }
    }
    SearchWorkspaceSet {
        workspaces,
        warnings,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum HitOrdering {
    Score,
    Preserve,
}

#[derive(Debug)]
pub(crate) struct SearchOutcome {
    pub(crate) hits: Vec<SearchHit>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) struct SearchBatch {
    hits: Vec<SearchHit>,
    warnings: Vec<String>,
    successful_workspaces: usize,
}

impl SearchBatch {
    pub(crate) fn new(warnings: Vec<String>) -> Self {
        Self {
            hits: Vec::new(),
            warnings,
            successful_workspaces: 0,
        }
    }

    pub(crate) fn record(
        &mut self,
        workspace_root: &Path,
        absolute_paths: bool,
        result: Result<Vec<SearchHit>>,
    ) {
        match result {
            Ok(mut hits) => {
                self.successful_workspaces += 1;
                if absolute_paths {
                    for hit in &mut hits {
                        hit.file_path = workspace_root.join(&hit.file_path);
                    }
                }
                self.hits.append(&mut hits);
            }
            Err(err) => self.warnings.push(format!(
                "search failed for {}: {err:#}",
                workspace_root.display()
            )),
        }
    }

    pub(crate) fn finish(
        mut self,
        limit: Option<usize>,
        ordering: HitOrdering,
    ) -> Result<SearchOutcome> {
        if self.successful_workspaces == 0 && !self.warnings.is_empty() {
            bail!("search failed: {}", self.warnings.join("; "));
        }

        if matches!(ordering, HitOrdering::Score) {
            self.hits.sort_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left.file_path.cmp(&right.file_path))
                    .then_with(|| left.start_line.cmp(&right.start_line))
            });
        }
        if let Some(limit) = limit {
            self.hits.truncate(limit);
        }

        Ok(SearchOutcome {
            hits: self.hits,
            warnings: self.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;

    use super::*;

    fn hit(path: &str, score: f32) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(path),
            start_line: 3,
            end_line: 4,
            preview: String::new(),
            reason: String::new(),
            score,
            sources: vec!["test".to_string()],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn partial_batch_keeps_hits_and_reports_failed_workspace() {
        let mut batch = SearchBatch::new(Vec::new());
        batch.record(Path::new("/one"), true, Ok(vec![hit("src/lib.rs", 1.0)]));
        batch.record(Path::new("/two"), true, Err(anyhow!("broken index")));

        let outcome = batch.finish(None, HitOrdering::Score).unwrap();
        assert_eq!(outcome.hits[0].file_path, PathBuf::from("/one/src/lib.rs"));
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("/two"));
        assert!(outcome.warnings[0].contains("broken index"));
    }

    #[test]
    fn batch_fails_when_every_workspace_fails() {
        let mut batch = SearchBatch::new(Vec::new());
        batch.record(Path::new("/one"), true, Err(anyhow!("missing index")));

        let err = batch.finish(None, HitOrdering::Score).unwrap_err();
        assert!(err.to_string().contains("missing index"));
    }

    #[test]
    fn score_order_is_deterministic() {
        let mut batch = SearchBatch::new(Vec::new());
        batch.record(
            Path::new("/repo"),
            false,
            Ok(vec![hit("b.rs", 1.0), hit("a.rs", 1.0), hit("c.rs", 2.0)]),
        );

        let outcome = batch.finish(Some(2), HitOrdering::Score).unwrap();
        assert_eq!(outcome.hits[0].file_path, PathBuf::from("c.rs"));
        assert_eq!(outcome.hits[1].file_path, PathBuf::from("a.rs"));
    }

    #[test]
    fn workspace_resolution_failures_are_reported() {
        let root = PathBuf::from("/missing");
        let selection = resolve_workspace_roots([root], |_| Err(anyhow!("stale registry entry")));

        assert!(selection.workspaces.is_empty());
        assert_eq!(selection.warnings.len(), 1);
        assert!(selection.warnings[0].contains("/missing"));
        assert!(selection.warnings[0].contains("stale registry entry"));
    }
}
