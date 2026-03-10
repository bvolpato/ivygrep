use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub preview: String,
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
        path: PathBuf,
        query: String,
        limit: Option<usize>,
        context: usize,
        type_filter: Option<String>,
    },
    RegexSearch {
        path: PathBuf,
        pattern: String,
        limit: Option<usize>,
    },
    Remove {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Ack { message: String },
    Status { workspaces: Vec<WorkspaceStatus> },
    SearchResults { hits: Vec<SearchHit> },
    Error { message: String },
}
