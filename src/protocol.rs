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
pub const DAEMON_PROTOCOL_VERSION: u32 = 7;

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
    /// Enqueue an index run and return immediately. Joins any in-flight
    /// `Index`/`StartIndex` run for the workspace instead of queuing another;
    /// callers poll `RuntimeStatus` (`index_in_flight`) for completion.
    StartIndex {
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
        #[serde(default)]
        disable_memory_expansion: bool,
    },
    RegexSearch {
        path: Option<PathBuf>,
        pattern: String,
        limit: Option<usize>,
        #[serde(default)]
        context: usize,
        #[serde(default)]
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
    CancelSearch {
        search_id: uuid::Uuid,
    },
    Remove {
        path: PathBuf,
    },
    Restart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequestEnvelope {
    pub protocol_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<uuid::Uuid>,
    #[serde(flatten)]
    pub request: DaemonRequest,
}

impl DaemonRequestEnvelope {
    pub fn new(request: DaemonRequest) -> Self {
        Self {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            request_id: None,
            request,
        }
    }

    pub fn with_request_id(request: DaemonRequest, request_id: uuid::Uuid) -> Self {
        Self {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            request_id: Some(request_id),
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
        /// Non-fatal workspace failures; default and omission preserve v5 compatibility.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<String>,
    },
    SearchProgress {
        stage: String,
        scanned: usize,
        total: usize,
    },
    /// Reply to `StartIndex`: the run was enqueued (`already_running: false`)
    /// or an in-flight run for the workspace will serve it.
    IndexStarted {
        accepted: bool,
        already_running: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        generation: Option<u64>,
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
    /// An explicit `Index`/`StartIndex` run is queued or running on the daemon.
    #[serde(default)]
    pub index_in_flight: bool,
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

    #[test]
    fn search_request_id_and_cancellation_round_trip() {
        let request_id = uuid::Uuid::new_v4();
        let search = DaemonRequestEnvelope::with_request_id(
            DaemonRequest::LiteralSearch {
                path: None,
                query: "needle".to_string(),
                limit: Some(10),
                context: 0,
                type_filter: None,
                include_globs: Vec::new(),
                exclude_globs: Vec::new(),
                scope_path: None,
                scope_is_file: false,
                skip_gitignore: false,
            },
            request_id,
        );
        let decoded: DaemonRequestEnvelope =
            serde_json::from_slice(&serde_json::to_vec(&search).unwrap()).unwrap();
        assert_eq!(decoded.protocol_version, DAEMON_PROTOCOL_VERSION);
        assert_eq!(decoded.request_id, Some(request_id));

        let cancel = DaemonRequestEnvelope::new(DaemonRequest::CancelSearch {
            search_id: request_id,
        });
        let cancel: DaemonRequestEnvelope =
            serde_json::from_slice(&serde_json::to_vec(&cancel).unwrap()).unwrap();
        assert!(cancel.request_id.is_none());
        assert!(matches!(
            cancel.request,
            DaemonRequest::CancelSearch { search_id } if search_id == request_id
        ));
    }

    #[test]
    fn legacy_regex_request_defaults_new_search_options() {
        let request: DaemonRequest = serde_json::from_value(serde_json::json!({
            "type": "regex_search",
            "path": null,
            "pattern": "marker",
            "limit": 10,
            "include_globs": [],
            "exclude_globs": [],
            "scope_path": null
        }))
        .unwrap();
        let DaemonRequest::RegexSearch {
            context,
            type_filter,
            ..
        } = request
        else {
            panic!("expected regex request");
        };
        assert_eq!(context, 0);
        assert!(type_filter.is_none());
    }

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
        assert_eq!(workspaces[0].hash_vector_count, 0);
        assert_eq!(workspaces[0].hash_coverage_percent, 0.0);
    }

    #[test]
    fn search_results_accept_v5_payload_without_warnings() {
        let response: DaemonResponse = serde_json::from_value(serde_json::json!({
            "type": "search_results",
            "hits": []
        }))
        .unwrap();

        let DaemonResponse::SearchResults { hits, warnings } = response else {
            panic!("expected search results");
        };
        assert!(hits.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn search_result_warnings_are_additive_on_wire() {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum LegacyResponse {
            SearchResults { hits: Vec<SearchHit> },
        }

        let clean = serde_json::to_value(DaemonResponse::SearchResults {
            hits: Vec::new(),
            warnings: Vec::new(),
        })
        .unwrap();
        assert!(clean.get("warnings").is_none());

        let partial = serde_json::to_value(DaemonResponse::SearchResults {
            hits: Vec::new(),
            warnings: vec!["one workspace failed".to_string()],
        })
        .unwrap();
        assert_eq!(partial["warnings"][0], "one workspace failed");

        let LegacyResponse::SearchResults { hits } =
            serde_json::from_value::<LegacyResponse>(partial).unwrap();
        assert!(hits.is_empty());
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
