use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceStatus;

/// Compile-time version tag so the CLI can detect stale daemon processes.
pub const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub preview: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub score: f32,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Status,
    Index {
        path: PathBuf,
        watch: bool,
    },
    Search {
        path: Option<PathBuf>,
        query: String,
        limit: Option<usize>,
        context: usize,
        type_filter: Option<String>,
        #[serde(default)]
        include_globs: Vec<String>,
        #[serde(default)]
        exclude_globs: Vec<String>,
        scope_path: Option<PathBuf>,
        #[serde(default)]
        scope_is_file: bool,
    },
    RegexSearch {
        path: Option<PathBuf>,
        pattern: String,
        limit: Option<usize>,
        #[serde(default)]
        include_globs: Vec<String>,
        #[serde(default)]
        exclude_globs: Vec<String>,
        scope_path: Option<PathBuf>,
        #[serde(default)]
        scope_is_file: bool,
    },
    LiteralSearch {
        path: Option<PathBuf>,
        query: String,
        limit: Option<usize>,
        context: usize,
        type_filter: Option<String>,
        #[serde(default)]
        include_globs: Vec<String>,
        #[serde(default)]
        exclude_globs: Vec<String>,
        scope_path: Option<PathBuf>,
        #[serde(default)]
        scope_is_file: bool,
    },
    Remove {
        path: PathBuf,
    },
    Restart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Ack {
        message: String,
    },
    Status {
        workspaces: Vec<WorkspaceStatus>,
        #[serde(default)]
        version: Option<String>,
    },
    SearchResults {
        hits: Vec<SearchHit>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSearchResult {
    pub file_path: PathBuf,
    pub total_score: f32,
    pub hit_count: usize,
    pub hits: Vec<SearchHit>,
}

pub fn group_hits_by_file(hits: &[SearchHit], limit: Option<usize>) -> Vec<FileSearchResult> {
    let mut grouped = HashMap::<PathBuf, FileSearchResult>::new();

    for hit in hits {
        let entry = grouped
            .entry(hit.file_path.clone())
            .or_insert_with(|| FileSearchResult {
                file_path: hit.file_path.clone(),
                total_score: 0.0,
                hit_count: 0,
                hits: vec![],
            });
        entry.total_score += hit.score;
        entry.hit_count += 1;
        entry.hits.push(hit.clone());
    }

    let mut files = grouped.into_values().collect::<Vec<_>>();
    for file in &mut files {
        file.hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.start_line.cmp(&b.start_line))
        });
    }

    files.sort_by(|a, b| {
        b.total_score
            .total_cmp(&a.total_score)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    if let Some(limit) = limit {
        files.truncate(limit);
    }

    files
}
