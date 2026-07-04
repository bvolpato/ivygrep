use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceStatus;

fn is_false(value: &bool) -> bool {
    !*value
}

/// Compile-time version tag so the CLI can detect stale daemon processes.
pub const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Wire protocol version for daemon request compatibility.
pub const DAEMON_PROTOCOL_VERSION: u32 = 4;

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
    #[serde(default, skip_serializing_if = "is_false")]
    pub neural_requested: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub neural_executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Version,
    RuntimeStatus {
        path: Option<PathBuf>,
    },
    Status,
    ServeWeb {
        host: String,
        port: u16,
        initial_query: Option<String>,
        initial_path: Option<PathBuf>,
    },
    Index {
        path: PathBuf,
        watch: bool,
        #[serde(default)]
        skip_gitignore: bool,
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
        #[serde(default)]
        skip_gitignore: bool,
        #[serde(default)]
        force_neural: bool,
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
        #[serde(default)]
        skip_gitignore: bool,
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
        #[serde(default)]
        skip_gitignore: bool,
    },
    Remove {
        path: PathBuf,
    },
    Restart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequestEnvelope {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub request: DaemonRequest,
}

impl DaemonRequestEnvelope {
    pub fn new(request: DaemonRequest) -> Self {
        Self {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            request,
        }
    }
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
    Version {
        #[serde(default)]
        version: Option<String>,
    },
    WebStarted {
        url: String,
    },
    RuntimeStatus {
        #[serde(default)]
        version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<WorkspaceRuntimeStatus>,
    },
    SearchResults {
        hits: Vec<SearchHit>,
    },
    SearchProgress {
        stage: String,
        scanned: usize,
        total: usize,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRuntimeStatus {
    pub id: String,
    pub watch_enabled: bool,
    pub watcher_alive: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(file: &str, score: f32, line: usize) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(file),
            start_line: line,
            end_line: line + 5,
            preview: format!("line {line}"),
            reason: String::new(),
            score,
            sources: vec!["test".to_string()],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn status_response_accepts_workspace_without_compaction_metadata() {
        let response: DaemonResponse = serde_json::from_value(serde_json::json!({
            "type": "status",
            "version": "0.11.2",
            "workspaces": [{
                "id": "workspace",
                "root": "/tmp/workspace",
                "last_indexed_at_unix": 1,
                "watch_enabled": true,
                "chunk_count": 2,
                "file_count": 1,
                "index_size_bytes": 3,
                "index_components": {
                    "metadata_bytes": 1,
                    "graph_bytes": 0,
                    "lexical_bytes": 1,
                    "hash_vectors_bytes": 1,
                    "neural_vectors_bytes": 0,
                    "other_bytes": 0
                },
                "vector_key_count": 2,
                "has_neural_vectors": false,
                "neural_vector_count": 0,
                "neural_coverage_percent": 0.0,
                "neural_dimensions": 0,
                "reranker_candidate_limit": 100
            }]
        }))
        .unwrap();

        let DaemonResponse::Status {
            version,
            workspaces,
        } = response
        else {
            panic!("expected status response");
        };

        assert_eq!(version.as_deref(), Some("0.11.2"));
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].compaction.format_version, 0);
        assert!(!workspaces[0].compaction.healthy);
    }

    #[test]
    fn groups_hits_by_file() {
        let hits = vec![
            hit("a.rs", 1.0, 10),
            hit("b.rs", 2.0, 20),
            hit("a.rs", 0.5, 30),
        ];
        let groups = group_hits_by_file(&hits, None);
        assert_eq!(groups.len(), 2);
        // b.rs has higher total score (2.0) than a.rs (1.5)
        assert_eq!(groups[0].file_path, PathBuf::from("b.rs"));
        assert_eq!(groups[0].hit_count, 1);
        assert_eq!(groups[1].file_path, PathBuf::from("a.rs"));
        assert_eq!(groups[1].hit_count, 2);
    }

    #[test]
    fn sorts_hits_within_file_by_score_descending() {
        let hits = vec![hit("a.rs", 0.5, 30), hit("a.rs", 1.0, 10)];
        let groups = group_hits_by_file(&hits, None);
        assert_eq!(groups[0].hits[0].start_line, 10);
        assert_eq!(groups[0].hits[1].start_line, 30);
    }

    #[test]
    fn truncates_to_limit() {
        let hits = vec![
            hit("a.rs", 1.0, 10),
            hit("b.rs", 2.0, 20),
            hit("c.rs", 3.0, 30),
        ];
        let groups = group_hits_by_file(&hits, Some(2));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].file_path, PathBuf::from("c.rs"));
        assert_eq!(groups[1].file_path, PathBuf::from("b.rs"));
    }

    #[test]
    fn empty_hits_returns_empty() {
        let groups = group_hits_by_file(&[], None);
        assert!(groups.is_empty());
    }

    #[test]
    fn total_score_is_sum_of_hit_scores() {
        let hits = vec![hit("a.rs", 1.5, 10), hit("a.rs", 2.5, 20)];
        let groups = group_hits_by_file(&hits, None);
        assert!((groups[0].total_score - 4.0).abs() < f32::EPSILON);
    }
}
