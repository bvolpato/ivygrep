use std::env;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Upper bound on a single JSON-RPC message / header line. Prevents a
/// malformed or malicious client (or `Content-Length` header) from triggering
/// an unbounded allocation or read.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use std::sync::Arc;
use std::sync::OnceLock;

use crate::config;
use crate::embedding::{EmbeddingModel, create_hash_model, create_neural_model};
use crate::indexer::{index_workspace, workspace_is_indexed};
use crate::path_glob::parse_glob_csv;
use crate::protocol::{
    DaemonRequest, DaemonResponse, FileSearchResult, SearchHit, group_hits_by_file,
};
use crate::regex_search::regex_search_with_options;
use crate::search::{SearchOptions, hybrid_search, literal_search};
use crate::symbols::{SymbolSearchMode, search_symbols_with_options};
use crate::workspace::{Workspace, WorkspaceMetadata, resolve_workspace_and_scope};

const JSONRPC_VERSION: &str = "2.0";
const LEGACY_PROTOCOL_VERSION: &str = "2024-11-05";
const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    LEGACY_PROTOCOL_VERSION,
];
const TOOL_IG_SEARCH: &str = "ig_search";
const TOOL_IG_STATUS: &str = "ig_status";
/// Result files returned by `ig_search` in hits mode when the caller omits
/// `limit`. Unbounded hit payloads (hundreds of chunks, ~40k tokens, emitted
/// twice) were the P0 failure mode for coding agents.
const DEFAULT_HITS_FILE_LIMIT: usize = 10;
/// Hits kept per result file in hits mode unless `hits_per_file` overrides it.
const DEFAULT_HITS_PER_FILE: usize = 3;
const MAX_HITS_PER_FILE: usize = 100;
/// Hits fetched per requested result file for ranked modes (hybrid, symbol
/// definitions). Keeps the default retrieval cost equal to the CLI default.
const RANKED_HIT_OVERFETCH: usize = 5;
/// Hits fetched per requested result file for enumerating modes (literal,
/// regex, references, callers), whose matches cluster inside few files.
const ENUMERATING_HIT_OVERFETCH: usize = 20;
const MIN_ENUMERATING_HIT_BUDGET: usize = 200;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug)]
struct DispatchError {
    code: i64,
    message: String,
}

impl DispatchError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn invalid_params(error: anyhow::Error) -> Self {
        Self {
            code: -32602,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IvygrepSearchArgs {
    query: Option<String>,
    path: Option<String>,
    output: Option<String>,
    budget_tokens: Option<usize>,
    since: Option<String>,
    limit: Option<usize>,
    hits_per_file: Option<usize>,
    context: Option<usize>,
    #[serde(rename = "type")]
    type_filter: Option<String>,
    regex: Option<bool>,
    literal: Option<bool>,
    symbol: Option<bool>,
    refs: Option<bool>,
    callers: Option<bool>,
    include: Option<String>,
    exclude: Option<String>,
    first_line_only: Option<bool>,
    file_name_only: Option<bool>,
    verbose: Option<bool>,
    skip_gitignore: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IvygrepStatusArgs {}

pub fn serve_stdio() -> Result<()> {
    config::ensure_app_dirs()?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut mode = FramingMode::Unknown;

    loop {
        let payload = match read_message(&mut reader, &mut mode)? {
            Some(payload) => payload,
            None => break,
        };

        let request: JsonRpcRequest = match serde_json::from_slice(&payload) {
            Ok(request) => request,
            Err(err) => {
                let response = JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION,
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("parse error: {err}"),
                    }),
                };
                write_message(&mut writer, &response, mode)?;
                continue;
            }
        };

        // Isolate handler panics: a panic deep in search must not crash the
        // whole MCP session. Capture it and return a JSON-RPC error instead.
        let request_id = request.id.clone();
        let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handle_request(request)
        })) {
            Ok(response) => response,
            Err(_) => request_id.map(|id| JsonRpcResponse {
                jsonrpc: JSONRPC_VERSION,
                id: Some(id),
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: "internal error: request handler panicked".to_string(),
                }),
            }),
        };

        if let Some(response) = response {
            write_message(&mut writer, &response, mode)?;
        }
    }

    Ok(())
}

fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = request.id.as_ref()?;
    let id = Some(id.clone());

    match dispatch(request.method.as_str(), request.params) {
        Ok(result) => Some(JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }),
        Err(err) => Some(JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: None,
            error: Some(JsonRpcError {
                code: err.code,
                message: err.message,
            }),
        }),
    }
}

fn dispatch(method: &str, params: Value) -> std::result::Result<Value, DispatchError> {
    match method {
        "initialize" => Ok(initialize_result(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": [search_tool_schema(), status_tool_schema()]})),
        "tools/call" => run_tool_call(params).map_err(DispatchError::invalid_params),
        "notifications/initialized" => Ok(json!({})),
        "shutdown" => Ok(json!({})),
        other => Err(DispatchError::method_not_found(other)),
    }
}

fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let protocol_version = match requested {
        Some(version) if SUPPORTED_PROTOCOL_VERSIONS.contains(&version) => version,
        Some(_) => LATEST_PROTOCOL_VERSION,
        None => LEGACY_PROTOCOL_VERSION,
    };

    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "ig",
            "title": "ivygrep",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Local hybrid semantic and lexical code search",
            "websiteUrl": env!("CARGO_PKG_HOMEPAGE")
        },
        "instructions": "Use ig_search with an absolute path to the active workspace so searches stay scoped to the intended repository. For implementation tasks, request output=context_pack with budget_tokens=8000 to receive one bounded pack containing primary code, dependencies, dependents, definitions, callers, references, tests, configuration, documentation, and recent co-change evidence. For iterative discovery, keep output=hits, use natural-language queries for concepts, and literal=true for exact identifiers. Hits mode returns at most limit files (default 10) with at most hits_per_file hits each (default 3); check truncated, total_matches, and more_hits_in_file, then narrow the query, scope path, or raise limit instead of re-running broad queries. Start with limit=5-10 and context=2. Use ig_status when indexing health is unclear. If ig_search returns status=indexing (not an error), the first index is still building in the background: wait retry_after_secs, then call again; do not retry immediately or fall back to scanning the filesystem. Workspaces are indexed on first use and watched for incremental updates."
    })
}

fn search_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_SEARCH,
        "title": "Search local code or build a task context pack",
        "description": "Hybrid semantic+lexical code search and token-budgeted task context. Auto-indexes on first query, stays local, respects .gitignore, and restricts results to the provided path scope. Use output=context_pack for implementation tasks; keep output=hits for iterative discovery. On a large repository the first call may return status=indexing with progress instead of results: the index keeps building in the background, so wait retry_after_secs and call again rather than retrying immediately.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural-language or keyword query."},
                "path": {"type": "string", "description": "Workspace path, subdirectory, or file path. Defaults to current directory."},
                "output": {
                    "type": "string",
                    "enum": ["hits", "context_pack"],
                    "default": "hits",
                    "description": "Return ranked search hits or one task-ready, relationship-expanded context pack. context_pack keeps the same single MCP tool call."
                },
                "budget_tokens": {
                    "type": "integer",
                    "minimum": 256,
                    "maximum": 131072,
                    "description": "Complete context-pack budget, including metadata and snippets. Valid only with output=context_pack."
                },
                "since": {
                    "type": "string",
                    "description": "Git ref for a diff-aware context pack. Includes merge-base changes plus staged, unstaged, and untracked files. Valid only with output=context_pack."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "description": "Maximum number of ranked result files in hits mode; defaults to 10 when omitted. Retrieval depth scales with it, so larger values may improve recall while adding lower-ranked files. Not a token, line, hit, or confidence limit; each file is further capped by hits_per_file. Ignored for output=context_pack, which is bounded by budget_tokens."
                },
                "hits_per_file": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 3,
                    "description": "Maximum hits returned per result file in hits mode. Files with more matches report more_hits_in_file. Raise it, or scope path to one file, to see every match in a file."
                },
                "context": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "default": 2,
                    "description": "Lines before and after each focused match. Changes snippet size, not retrieval ranking."
                },
                "type": {"type": "string", "description": "Language filter - accepts names (rust, python), extensions (rs, py, md), or aliases (c++, bash, js)."},
                "regex": {"type": "boolean", "description": "Use regex mode (index-prefiltered when possible; otherwise walks raw files). Prefer 'literal' for exact matches."},
                "literal": {"type": "boolean", "description": "Fast exact-match search backed by the index. Deterministic, orders of magnitude faster than regex."},
                "symbol": {"type": "boolean", "description": "Find exact symbol definitions."},
                "refs": {"type": "boolean", "description": "Find exact references to the named symbol."},
                "callers": {"type": "boolean", "description": "Find functions or methods that call the named symbol."},
                "include": {"type": "string", "description": "Comma-separated include globs, e.g. \"*.md,src/**/*.rs\"."},
                "exclude": {"type": "string", "description": "Comma-separated exclude globs, e.g. \"target/**,*.lock\"."},
                "first_line_only": {"type": "boolean", "description": "Return only the first non-empty preview line for each hit. Ranking is unchanged."},
                "file_name_only": {"type": "boolean", "description": "Return only file paths (no hit details). Ranking is unchanged."},
                "verbose": {"type": "boolean", "description": "Include reason pointers in JSON output."},
                "skip_gitignore": {"type": "boolean", "description": "Include files ignored by .gitignore."}
            },
            "required": ["query"],
            "additionalProperties": false
        },
        "outputSchema": search_output_schema(),
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn status_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_STATUS,
        "title": "Inspect ivygrep indexes",
        "description": "Returns the list of indexed projects (workspaces) and their current indexing status, detailing if they are ready to query.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        },
        "outputSchema": status_output_schema(),
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn search_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "workspace_root": {"type": "string"},
            "scope_path": {"type": ["string", "null"]},
            "scope_is_file": {"type": "boolean"},
            "query": {"type": "string"},
            "mode": {"type": "string", "enum": ["hybrid", "literal", "regex", "symbol", "references", "callers", "context"]},
            "result_count": {"type": "integer", "minimum": 0},
            "total_matches": {
                "type": "integer",
                "minimum": 0,
                "description": "Files matched before limit was applied (hits mode). A lower bound when retrieval hit its candidate budget."
            },
            "truncated": {
                "type": "boolean",
                "description": "True when matched files were cut to limit or retrieval hit its candidate budget; narrow the query or raise limit to see more."
            },
            "include": {"type": "array", "items": {"type": "string"}},
            "exclude": {"type": "array", "items": {"type": "string"}},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "total_score": {"type": "number"},
                        "hit_count": {"type": "integer", "minimum": 0, "description": "Hits matched in this file before hits_per_file was applied."},
                        "more_hits_in_file": {"type": "integer", "minimum": 1, "description": "Hits omitted from this file by hits_per_file among the retrieved matches. Absent when nothing was cut."},
                        "hit_count_is_lower_bound": {"type": "boolean", "description": "True when retrieval stopped at its candidate budget, so hit_count and more_hits_in_file are lower bounds for this file. Absent when counts are exact."},
                        "hits": {"type": "array", "items": {"type": "object"}}
                    },
                    "required": ["file_path", "total_score", "hit_count", "hits"]
                }
            },
            "file_paths": {"type": "array", "items": {"type": "string"}},
            "context_pack": context_pack_output_schema(),
            "status": {
                "type": "string",
                "enum": ["indexing"],
                "description": "Present only when the workspace's first index is still running; no results are returned. Retry after retry_after_secs."
            },
            "progress": {
                "type": "object",
                "properties": {
                    "phase": {"type": "string"},
                    "done": {"type": ["integer", "null"], "minimum": 0},
                    "total": {"type": ["integer", "null"], "minimum": 0},
                    "percent": {"type": ["number", "null"], "minimum": 0, "maximum": 100}
                },
                "required": ["phase", "done", "total", "percent"],
                "additionalProperties": false
            },
            "elapsed_secs": {"type": "integer", "minimum": 0},
            "retry_after_secs": {"type": "integer", "minimum": 1},
            "message": {"type": "string"}
        },
        "oneOf": [
            {
                "required": [
                    "workspace_root",
                    "scope_path",
                    "scope_is_file",
                    "query",
                    "mode",
                    "result_count",
                    "include",
                    "exclude"
                ],
                "oneOf": [
                    {"required": ["results"]},
                    {"required": ["file_paths"]},
                    {"required": ["context_pack"]}
                ]
            },
            {
                "required": [
                    "status",
                    "workspace_root",
                    "progress",
                    "elapsed_secs",
                    "retry_after_secs",
                    "message"
                ]
            }
        ],
        "additionalProperties": false
    })
}

fn context_pack_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task": {"type": "string"},
            "workspace": {"type": "string"},
            "change_scope": {
                "type": "object",
                "properties": {
                    "since": {"type": "string"},
                    "base_commit": {"type": "string"},
                    "dirty_worktree": {"type": "boolean"},
                    "total_changes": {"type": "integer", "minimum": 0},
                    "changes_truncated": {"type": "boolean"},
                    "changes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": {"type": "string"},
                                "old_path": {"type": "string"},
                                "status": {
                                    "type": "string",
                                    "enum": ["added", "modified", "deleted", "renamed", "copied", "type_changed", "unmerged", "unknown"]
                                },
                                "sources": {
                                    "type": "array",
                                    "items": {"type": "string", "enum": ["since", "staged", "worktree", "untracked"]}
                                }
                            },
                            "required": ["file_path", "status", "sources"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["dirty_worktree", "total_changes", "changes_truncated", "changes"],
                "additionalProperties": false
            },
            "referenced_paths": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "line": {"type": "integer", "minimum": 1}
                    },
                    "required": ["file_path"],
                    "additionalProperties": false
                }
            },
            "budget_tokens": {"type": "integer", "minimum": 256, "maximum": 131072},
            "used_tokens": {"type": "integer", "minimum": 0},
            "candidate_count": {"type": "integer", "minimum": 0},
            "truncated": {"type": "boolean"},
            "anchor_symbols": {"type": "array", "items": {"type": "string"}},
            "coverage": {
                "type": "object",
                "properties": {
                    "files": {"type": "integer", "minimum": 0},
                    "primary": {"type": "integer", "minimum": 0},
                    "definitions": {"type": "integer", "minimum": 0},
                    "dependencies": {"type": "integer", "minimum": 0},
                    "dependents": {"type": "integer", "minimum": 0},
                    "callers": {"type": "integer", "minimum": 0},
                    "references": {"type": "integer", "minimum": 0},
                    "tests": {"type": "integer", "minimum": 0},
                    "config": {"type": "integer", "minimum": 0},
                    "documentation": {"type": "integer", "minimum": 0}
                },
                "required": [
                    "files", "primary", "definitions", "dependencies", "dependents",
                    "callers", "references", "tests", "config", "documentation"
                ],
                "additionalProperties": false
            },
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file_path": {"type": "string"},
                        "start_line": {"type": "integer", "minimum": 1},
                        "end_line": {"type": "integer", "minimum": 1},
                        "roles": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": [
                                    "primary", "definition", "dependency", "dependent",
                                    "caller", "reference", "test", "config",
                                    "documentation", "related"
                                ]
                            }
                        },
                        "reasons": {"type": "array", "items": {"type": "string"}},
                        "sources": {"type": "array", "items": {"type": "string"}},
                        "preview": {"type": "string"},
                        "estimated_tokens": {"type": "integer", "minimum": 0}
                    },
                    "required": [
                        "file_path", "start_line", "end_line", "roles", "reasons",
                        "sources", "preview", "estimated_tokens"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "required": [
            "task", "workspace", "referenced_paths", "budget_tokens", "used_tokens", "candidate_count",
            "truncated", "anchor_symbols", "coverage", "items"
        ],
        "additionalProperties": false
    })
}

fn status_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "workspaces": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "workspace_root": {"type": "string"},
                        "ready_to_query": {"type": "boolean"},
                        "status": {"type": "string"},
                        "chunk_count": {"type": "integer", "minimum": 0},
                        "file_count": {"type": "integer", "minimum": 0},
                        "indexing_in_progress": {"type": "boolean"},
                        "enhancing_in_progress": {"type": "boolean"},
                        "watch_enabled": {"type": "boolean"},
                        "watcher_alive": {"type": "boolean"},
                        "indexing_stalled": {"type": "boolean"},
                        "enhancing_stalled": {"type": "boolean"}
                    },
                    "required": [
                        "workspace_root",
                        "ready_to_query",
                        "status",
                        "chunk_count",
                        "file_count",
                        "indexing_in_progress",
                        "enhancing_in_progress",
                        "watch_enabled",
                        "watcher_alive",
                        "indexing_stalled",
                        "enhancing_stalled"
                    ],
                    "additionalProperties": false
                }
            }
        },
        "required": ["workspaces"],
        "additionalProperties": false
    })
}

fn run_tool_call(params: Value) -> Result<Value> {
    let call: ToolCallParams = serde_json::from_value(params)?;
    if call.name == TOOL_IG_SEARCH {
        let result = serde_json::from_value(call.arguments)
            .map_err(anyhow::Error::from)
            .and_then(execute_ivygrep_search);
        Ok(result.unwrap_or_else(tool_error_result))
    } else if call.name == TOOL_IG_STATUS {
        let arguments = if call.arguments.is_null() {
            json!({})
        } else {
            call.arguments
        };
        let result = serde_json::from_value::<IvygrepStatusArgs>(arguments)
            .map_err(anyhow::Error::from)
            .and_then(|_| execute_ivygrep_status());
        Ok(result.unwrap_or_else(tool_error_result))
    } else {
        bail!("unknown tool: {}", call.name);
    }
}

fn tool_error_result(error: anyhow::Error) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": error.to_string()
            }
        ],
        "isError": true
    })
}

fn tool_success_result(payload: Value, pretty: bool) -> Result<Value> {
    let text = if pretty {
        serde_json::to_string_pretty(&payload)?
    } else {
        serde_json::to_string(&payload)?
    };
    Ok(tool_success_result_with_text(payload, text))
}

/// Build a tool result whose `structuredContent` is the machine-readable
/// payload and whose text block is a separate rendering. Emitting the same JSON
/// in both places doubled every search payload on the wire.
fn tool_success_result_with_text(payload: Value, text: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": payload,
        "isError": false
    })
}

fn execute_ivygrep_status() -> Result<Value> {
    let workspaces = crate::workspace::list_workspaces()?;

    let mut projects = Vec::new();
    for ws in workspaces {
        let ready_to_query = ws.chunk_count > 0 && ws.last_indexed_at_unix.is_some();
        // A run accepted by the daemon but still parked behind the workspace
        // lease or CPU permits has no job heartbeat yet; ask the daemon so the
        // workspace is not reported as idle while its first index is queued.
        let indexing_in_progress =
            ws.indexing_in_progress || (!ready_to_query && daemon_index_in_flight(&ws.root));
        let status_msg = if ready_to_query {
            if ws.enhancing_in_progress {
                "Ready to query (Background enhancement in progress)"
            } else if ws.enhancing_stalled {
                "Ready to query (Background enhancement stalled)"
            } else if !ws.has_neural_vectors {
                "Ready to query (Lexical only)"
            } else {
                "Ready to query"
            }
        } else if indexing_in_progress {
            "Indexing in progress (Not ready)"
        } else if ws.indexing_stalled {
            "Indexing stalled (Needs attention)"
        } else {
            "Not indexed"
        };

        projects.push(json!({
            "workspace_root": ws.root,
            "ready_to_query": ready_to_query,
            "status": status_msg,
            "chunk_count": ws.chunk_count,
            "file_count": ws.file_count,
            "indexing_in_progress": indexing_in_progress,
            "enhancing_in_progress": ws.enhancing_in_progress,
            "watch_enabled": ws.watch_enabled,
            "watcher_alive": ws.watcher_alive,
            "indexing_stalled": ws.indexing_stalled,
            "enhancing_stalled": ws.enhancing_stalled,
        }));
    }

    let payload = json!({
        "workspaces": projects
    });

    tool_success_result(payload, true)
}

/// Whether the daemon has an explicit index run queued or running for `root`.
/// Never spawns a daemon; an absent daemon means nothing is in flight.
fn daemon_index_in_flight(root: &Path) -> bool {
    if !crate::ipc::socket_exists() {
        return false;
    }
    let request = DaemonRequest::RuntimeStatus {
        path: Some(root.to_path_buf()),
    };
    matches!(
        crate::daemon::request_blocking(&request, false),
        Ok(Some(DaemonResponse::RuntimeStatus {
            workspace: Some(status),
            ..
        })) if status.index_in_flight
    )
}

/// The neural query model for this MCP process, loaded once and reused across
/// requests.
///
/// `serve_stdio` is a long-lived server, so reconstructing the Candle model
/// per request reloads the weights every search — hundreds of ms of avoidable
/// latency and memory churn. Cache it here, mirroring the daemon's
/// `DaemonState.lazy_model` / `cached_hash_model()`.
///
/// Only a *successfully initialized neural* model is cached. If neural init
/// fails (transient model download/load error, or the `neural` feature is not
/// compiled in) we return a fresh hash model for this call and leave the cache
/// empty so the next request retries neural — otherwise a single startup
/// failure would silently pin every future search to hash embeddings until the
/// process restarts. Hash-model construction is cheap (no I/O), so retrying it
/// per call costs nothing meaningful.
fn mcp_query_model() -> Arc<dyn EmbeddingModel> {
    static MODEL: OnceLock<Arc<dyn EmbeddingModel>> = OnceLock::new();
    if let Some(model) = MODEL.get() {
        return model.clone();
    }
    match create_neural_model() {
        Ok(model) => {
            let model: Arc<dyn EmbeddingModel> = Arc::from(model);
            // First successful neural init wins; the MCP stdio loop is serial,
            // so a lost race here is not a concern.
            let _ = MODEL.set(model.clone());
            model
        }
        Err(_) => Arc::from(create_hash_model()),
    }
}

fn mcp_search_model(workspace: &Workspace) -> Arc<dyn EmbeddingModel> {
    if workspace.has_neural_vectors() {
        mcp_query_model()
    } else {
        Arc::from(create_hash_model())
    }
}

/// Outcome of preparing a workspace for an MCP search.
enum WorkspaceReadiness {
    Ready,
    /// The daemon is still building the index; carries the structured
    /// `status: indexing` payload for the tool result.
    Indexing(Value),
}

const MCP_INDEX_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
const DEFAULT_INDEX_RETRY_AFTER_SECS: u64 = 10;

/// Re-index when the caller wants ignored files but the index excludes them.
fn needs_ignored_refresh(workspace: &Workspace, include_ignored: bool) -> Result<bool> {
    let metadata = workspace.read_metadata()?;
    Ok(include_ignored
        && !metadata
            .as_ref()
            .is_some_and(|metadata| metadata.skip_gitignore))
}

fn ensure_mcp_workspace_ready(
    workspace: &Workspace,
    include_ignored: bool,
) -> Result<WorkspaceReadiness> {
    let metadata = workspace.read_metadata()?;
    let include_ignored = include_ignored
        || metadata
            .as_ref()
            .is_some_and(|metadata| metadata.skip_gitignore);

    if workspace_is_indexed(workspace)
        && workspace.is_watcher_alive()
        && !needs_ignored_refresh(workspace, include_ignored)?
    {
        return Ok(WorkspaceReadiness::Ready);
    }

    // Enqueue on the daemon and wait a bounded time. A first index of a large
    // repository can take minutes; MCP clients time out tool calls long before
    // that, so never block the call on the whole run.
    let start_request = DaemonRequest::StartIndex {
        path: workspace.root.clone(),
        watch: true,
        skip_gitignore: include_ignored,
    };
    let wait_started = std::time::Instant::now();
    match crate::daemon::request_blocking(&start_request, true)? {
        Some(DaemonResponse::IndexStarted { .. }) => {
            return wait_for_daemon_index(workspace, include_ignored, wait_started);
        }
        Some(DaemonResponse::Error { message }) => {
            // The daemon is up and rejected the request; do not duplicate its
            // work locally. An already queryable index still serves searches,
            // unless the request needs ignored files the index never saw.
            if workspace_is_indexed(workspace)
                && !needs_ignored_refresh(workspace, include_ignored)?
            {
                tracing::warn!(
                    "MCP daemon index request rejected; searching existing index: {message}"
                );
                return Ok(WorkspaceReadiness::Ready);
            }
            bail!(
                "ivygrep daemon rejected index request for {}: {message}",
                workspace.root.display()
            );
        }
        Some(response) => {
            tracing::warn!("unexpected MCP daemon indexing response: {response:?}");
        }
        None => {}
    }

    // No response. Either the daemon is unreachable (no socket, autospawn
    // disabled, transport failure before the request was sent) or the request
    // was accepted but the reply was lost (timeout, dropped connection). Only
    // the first case may index in-process: if a daemon still answers, its
    // detached run may already be active and a local run would duplicate it.
    if crate::daemon::request_blocking(&DaemonRequest::Version, false)?.is_some() {
        tracing::warn!(
            "MCP index request for {} got no reply from a live daemon; polling its status instead of indexing locally",
            workspace.root.display()
        );
        return wait_for_daemon_index(workspace, include_ignored, wait_started);
    }
    index_workspace_locally(workspace, include_ignored)?;
    Ok(WorkspaceReadiness::Ready)
}

/// Poll the daemon until its run for `workspace` clears or the bounded wait
/// (`IVYGREP_MCP_INDEX_WAIT_SECS`) elapses. Never indexes locally: the daemon
/// owns the run, and a second `StartIndex` joins it instead of duplicating it.
fn wait_for_daemon_index(
    workspace: &Workspace,
    include_ignored: bool,
    wait_started: std::time::Instant,
) -> Result<WorkspaceReadiness> {
    let deadline = wait_started + config::mcp_index_wait();
    let status_request = DaemonRequest::RuntimeStatus {
        path: Some(workspace.root.clone()),
    };
    let mut resubmitted = false;
    loop {
        let in_flight = match crate::daemon::request_blocking(&status_request, false)? {
            Some(DaemonResponse::RuntimeStatus {
                workspace: Some(status),
                ..
            }) => status.index_in_flight,
            Some(DaemonResponse::RuntimeStatus { .. }) => false,
            Some(DaemonResponse::Error { message }) => {
                bail!(
                    "ivygrep daemon status failed for {}: {message}",
                    workspace.root.display()
                )
            }
            Some(response) => {
                tracing::warn!("unexpected MCP daemon status response: {response:?}");
                false
            }
            None => {
                // A queryable index only counts if it already covers what the
                // request asked for; a gitignore-respecting index cannot serve
                // a skip_gitignore search.
                if workspace_is_indexed(workspace)
                    && !needs_ignored_refresh(workspace, include_ignored)?
                {
                    return Ok(WorkspaceReadiness::Ready);
                }
                bail!(
                    "ivygrep daemon became unavailable while indexing {}; call again to resume",
                    workspace.root.display()
                );
            }
        };

        if !in_flight {
            if workspace_is_indexed(workspace)
                && !needs_ignored_refresh(workspace, include_ignored)?
            {
                return Ok(WorkspaceReadiness::Ready);
            }
            let failure = crate::jobs::job_status(
                workspace,
                crate::jobs::JobKind::Indexing,
                crate::jobs::INDEXING_HEARTBEAT_TTL_SECS,
            )
            .record
            .filter(|record| !record.active)
            .and_then(|record| record.last_error);
            if let Some(error) = failure {
                bail!(
                    "ivygrep daemon index failed for {}: {error}",
                    workspace.root.display()
                );
            }
            if resubmitted {
                bail!(
                    "ivygrep daemon index for {} finished without a queryable index; call again",
                    workspace.root.display()
                );
            }
            // The run that was in flight had different options (for example a
            // CLI `--no-watch` index) and did not satisfy this request; queue
            // ours now that it can lead.
            resubmitted = true;
            let start_request = DaemonRequest::StartIndex {
                path: workspace.root.clone(),
                watch: true,
                skip_gitignore: include_ignored,
            };
            match crate::daemon::request_blocking(&start_request, false)? {
                Some(DaemonResponse::IndexStarted { .. }) => {}
                Some(DaemonResponse::Error { message }) => {
                    bail!(
                        "ivygrep daemon rejected index request for {}: {message}",
                        workspace.root.display()
                    )
                }
                _ => bail!(
                    "ivygrep daemon became unavailable while indexing {}; call again to resume",
                    workspace.root.display()
                ),
            }
        }

        if std::time::Instant::now() >= deadline {
            return Ok(WorkspaceReadiness::Indexing(indexing_status_payload(
                workspace,
                wait_started.elapsed(),
            )));
        }
        std::thread::sleep(
            MCP_INDEX_POLL_INTERVAL
                .min(deadline.saturating_duration_since(std::time::Instant::now())),
        );
    }
}

/// In-process index used only when no daemon is reachable.
fn index_workspace_locally(workspace: &Workspace, include_ignored: bool) -> Result<()> {
    workspace.ensure_dirs()?;
    let mut metadata = workspace
        .read_metadata()?
        .unwrap_or_else(|| WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_indexed_at_unix: None,
            watch_enabled: true,
            skip_gitignore: include_ignored,
            index_generation: 0,
        });
    metadata.watch_enabled = true;
    if include_ignored {
        metadata.skip_gitignore = true;
    }
    workspace.write_metadata(&metadata)?;

    let index_model = create_hash_model();
    index_workspace(workspace, index_model.as_ref())?;
    Ok(())
}

/// Progress of the daemon's index run, read from the same job ledger and
/// progress file that `ig --status` and the CLI first-run spinner use.
fn indexing_status_payload(workspace: &Workspace, waited: std::time::Duration) -> Value {
    let job = crate::jobs::job_status(
        workspace,
        crate::jobs::JobKind::Indexing,
        crate::jobs::INDEXING_HEARTBEAT_TTL_SECS,
    );
    let active_record = job.record.as_ref().filter(|_| job.active());
    let raw_progress = std::fs::read_to_string(workspace.indexing_progress_path())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| active_record.and_then(|record| record.details.get("progress").cloned()))
        .or_else(|| {
            active_record
                .map(|record| record.phase.clone())
                .filter(|phase| !phase.is_empty())
        });
    let (phase, done, total) = match raw_progress.as_deref() {
        Some(progress) => match parse_file_progress(progress) {
            Some((done, total)) => ("indexing".to_string(), Some(done), Some(total)),
            None => (progress.to_string(), None, None),
        },
        None => ("queued".to_string(), None, None),
    };
    let now = crate::jobs::now_unix();
    let elapsed_secs = active_record
        .and_then(|record| record.started_at_unix)
        .map(|started| now.saturating_sub(started))
        .map_or(waited.as_secs(), |since_start| {
            since_start.max(waited.as_secs())
        });
    let percent = match (done, total) {
        (Some(done), Some(total)) if total > 0 => {
            Some(((done as f64 / total as f64) * 1000.0).round() / 10.0)
        }
        _ => None,
    };
    let retry_after_secs = estimate_retry_after_secs(done, total, elapsed_secs);
    json!({
        "status": "indexing",
        "workspace_root": workspace.root,
        "progress": {
            "phase": phase,
            "done": done,
            "total": total,
            "percent": percent,
        },
        "elapsed_secs": elapsed_secs,
        "retry_after_secs": retry_after_secs,
        "message": "Index in progress; call again later. Lexical search becomes available when the first index commits.",
    })
}

/// Parse the indexer's `done/total` progress string.
fn parse_file_progress(progress: &str) -> Option<(u64, u64)> {
    let (done, total) = progress.split_once('/')?;
    Some((done.trim().parse().ok()?, total.trim().parse().ok()?))
}

/// Remaining-time estimate from observed throughput, clamped to 5-60 s;
/// falls back to a fixed delay when no throughput is known yet.
fn estimate_retry_after_secs(done: Option<u64>, total: Option<u64>, elapsed_secs: u64) -> u64 {
    match (done, total) {
        (Some(done), Some(total)) if done > 0 && total > done && elapsed_secs > 0 => {
            let remaining = (total - done) as f64 * elapsed_secs as f64 / done as f64;
            (remaining.ceil() as u64).clamp(5, 60)
        }
        _ => DEFAULT_INDEX_RETRY_AFTER_SECS,
    }
}

/// Non-error tool result for a workspace whose first index is still running.
/// `content[0].text` is human-readable (followed by the JSON payload) so agents
/// without `structuredContent` support still see what to do.
fn indexing_tool_result(payload: Value) -> Result<Value> {
    let progress = &payload["progress"];
    let phase = progress["phase"].as_str().unwrap_or("indexing");
    let counts = match (progress["done"].as_u64(), progress["total"].as_u64()) {
        (Some(done), Some(total)) => {
            let percent = progress["percent"].as_f64().unwrap_or(0.0);
            format!(" {done}/{total} files ({percent:.1}%)")
        }
        _ => String::new(),
    };
    let text = format!(
        "Indexing {}: {phase}{counts}, {}s elapsed. Not ready yet; call ig_search again in ~{}s. Lexical search becomes available when the first index commits.\n{}",
        payload["workspace_root"].as_str().unwrap_or("workspace"),
        payload["elapsed_secs"].as_u64().unwrap_or(0),
        payload["retry_after_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_INDEX_RETRY_AFTER_SECS),
        serde_json::to_string(&payload)?
    );
    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": payload,
        "isError": false
    }))
}

fn execute_ivygrep_search(args: IvygrepSearchArgs) -> Result<Value> {
    let query = args
        .query
        .as_deref()
        .context("missing required argument: query")?;
    if query.trim().is_empty() {
        bail!("query must not be empty");
    }
    let output = args.output.as_deref().unwrap_or("hits");
    if !matches!(output, "hits" | "context_pack") {
        bail!("output must be hits or context_pack");
    }
    let wants_context_pack = output == "context_pack";
    let requested_modes = [
        args.literal.unwrap_or(false),
        args.regex.unwrap_or(false),
        args.symbol.unwrap_or(false),
        args.refs.unwrap_or(false),
        args.callers.unwrap_or(false),
    ];
    if requested_modes
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        bail!("literal, regex, symbol, refs, and callers modes are mutually exclusive");
    }
    if args.limit == Some(0) || args.limit.is_some_and(|limit| limit > 1000) {
        bail!("limit must be between 1 and 1000");
    }
    if args
        .hits_per_file
        .is_some_and(|hits| !(1..=MAX_HITS_PER_FILE).contains(&hits))
    {
        bail!("hits_per_file must be between 1 and {MAX_HITS_PER_FILE}");
    }
    if args.context.is_some_and(|context| context > 100) {
        bail!("context must be between 0 and 100");
    }
    if args
        .budget_tokens
        .is_some_and(|budget| !(256..=131_072).contains(&budget))
    {
        bail!("budget_tokens must be between 256 and 131072");
    }
    if !wants_context_pack && args.budget_tokens.is_some() {
        bail!("budget_tokens requires output=context_pack");
    }
    if !wants_context_pack && args.since.is_some() {
        bail!("since requires output=context_pack");
    }
    if wants_context_pack && args.hits_per_file.is_some() {
        bail!("hits_per_file requires output=hits");
    }
    if wants_context_pack
        && (requested_modes.into_iter().any(|enabled| enabled)
            || args.first_line_only.unwrap_or(false)
            || args.file_name_only.unwrap_or(false))
    {
        bail!(
            "output=context_pack cannot be combined with literal, regex, symbol, refs, callers, first_line_only, or file_name_only"
        );
    }

    let input_path = match args.path {
        Some(path) => PathBuf::from(path),
        None => env::current_dir()?,
    };

    let (current_workspace, scope_filter) = resolve_workspace_and_scope(Path::new(&input_path))?;

    // MCP search is intentionally scoped to one workspace. Ensure that
    // workspace is indexed and watched before searching so edits made by a
    // coding agent become searchable without restarting the MCP process.
    if let WorkspaceReadiness::Indexing(payload) =
        ensure_mcp_workspace_ready(&current_workspace, args.skip_gitignore.unwrap_or(false))?
    {
        return indexing_tool_result(payload);
    }
    let workspace = current_workspace.clone();
    let _ = workspace.cleanup_stale_legacy_runtime_files();

    let literal = args.literal.unwrap_or(false);
    let regex = args.regex.unwrap_or(false);
    let symbol_mode = if args.symbol.unwrap_or(false) {
        Some(SymbolSearchMode::Definitions)
    } else if args.refs.unwrap_or(false) {
        Some(SymbolSearchMode::References)
    } else if args.callers.unwrap_or(false) {
        Some(SymbolSearchMode::Callers)
    } else {
        None
    };
    // `limit` counts result files. Retrieval APIs count hits, so over-fetch a
    // bounded hit budget and group/cap afterwards.
    let file_limit = args.limit.unwrap_or(DEFAULT_HITS_FILE_LIMIT);
    let hits_per_file = args.hits_per_file.unwrap_or(DEFAULT_HITS_PER_FILE);
    let ranked_mode = !literal
        && !regex
        && !matches!(
            symbol_mode,
            Some(SymbolSearchMode::References | SymbolSearchMode::Callers)
        );
    let hit_budget = hits_mode_hit_budget(file_limit, ranked_mode);
    let search_limit = Some(hit_budget);

    let include_globs = parse_glob_csv(args.include.as_deref());
    let exclude_globs = parse_glob_csv(args.exclude.as_deref());
    let search_options = SearchOptions {
        limit: search_limit,
        context: args.context.unwrap_or(2),
        type_filter: args.type_filter.clone(),
        include_globs: include_globs.clone(),
        exclude_globs: exclude_globs.clone(),
        scope_filter: scope_filter.clone(),
        skip_gitignore: args.skip_gitignore.unwrap_or(false),
        force_neural: false,
        progress_tx: None,
        cancel_token: None,
    };

    if wants_context_pack {
        let model = mcp_search_model(&workspace);
        let bundle = crate::context::build_context_bundle_with_options(
            &workspace,
            query,
            Some(model.as_ref()),
            &SearchOptions {
                limit: None,
                ..search_options.clone()
            },
            args.budget_tokens.unwrap_or(8_000),
            &crate::context::ContextBuildOptions {
                since: args.since.as_deref(),
            },
        )?;
        let query_uses_neural = crate::search::query_uses_neural(query, false);
        if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
            && workspace.needs_search_enhancement(query_uses_neural)
        {
            let _ = workspace.trigger_background_search_enhancement(query_uses_neural);
        }
        let payload = json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": "context",
            "result_count": bundle.items.len(),
            "include": include_globs,
            "exclude": exclude_globs,
            "context_pack": bundle,
        });
        return tool_success_result(payload, false);
    }

    let daemon_request = if symbol_mode.is_some() {
        None
    } else if literal {
        Some(DaemonRequest::LiteralSearch {
            path: Some(workspace.root.clone()),
            query: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
        })
    } else if regex {
        Some(DaemonRequest::RegexSearch {
            path: Some(workspace.root.clone()),
            pattern: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
        })
    } else {
        Some(DaemonRequest::Search {
            path: Some(workspace.root.clone()),
            query: query.to_string(),
            limit: search_limit,
            context: args.context.unwrap_or(2),
            type_filter: args.type_filter.clone(),
            include_globs: include_globs.clone(),
            exclude_globs: exclude_globs.clone(),
            scope_path: scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            scope_is_file: scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            skip_gitignore: args.skip_gitignore.unwrap_or(false),
            force_neural: false,
            disable_memory_expansion: false,
        })
    };
    let mut search_warnings = Vec::new();
    let daemon_hits = if let Some(daemon_request) = daemon_request {
        // Tag the search so a client-side timeout cancels it on the daemon
        // instead of leaving the work running for a caller that gave up.
        match crate::daemon::request_blocking_with_id(
            &daemon_request,
            Some(uuid::Uuid::new_v4()),
            false,
        )? {
            Some(DaemonResponse::SearchResults { hits, warnings }) => {
                search_warnings = warnings;
                Some(hits)
            }
            Some(DaemonResponse::Error { message }) => {
                tracing::warn!("MCP daemon search unavailable, searching locally: {message}");
                None
            }
            Some(response) => {
                tracing::warn!("unexpected MCP daemon search response: {response:?}");
                None
            }
            None => None,
        }
    } else {
        None
    };

    let mut hits = if let Some(hits) = daemon_hits {
        hits
    } else if let Some(mode) = symbol_mode {
        search_symbols_with_options(&workspace, query, mode, &search_options)?
    } else if literal {
        literal_search(&workspace, query, &search_options)?
    } else if regex {
        regex_search_with_options(&workspace, query, &search_options)?
    } else {
        // Load a neural query model only after neural vectors exist; a new
        // index returns hash results without downloading/loading model assets.
        let model = mcp_search_model(&workspace);
        let hits = hybrid_search(&workspace, query, Some(model.as_ref()), &search_options)?;
        // Exact queries build hash vectors; natural-language queries also build neural vectors.
        let query_uses_neural = crate::search::query_uses_neural(query, false);
        if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
            && workspace.needs_search_enhancement(query_uses_neural)
        {
            let _ = workspace.trigger_background_search_enhancement(query_uses_neural);
        }
        hits
    };

    if !literal && !regex {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let BoundedHits {
        files: mut grouped,
        total_matches,
        truncated,
    } = bound_hits_by_file(&hits, file_limit, hits_per_file, hit_budget);
    let verbose = args.verbose.unwrap_or(false);
    let first_line_only = args.first_line_only.unwrap_or(false);
    let file_name_only = args.file_name_only.unwrap_or(false);

    if !verbose {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.reason.clear();
            }
        }
    }

    if first_line_only {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.preview = hit
                    .preview
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        }
    }

    let mode_name = if literal {
        "literal"
    } else if regex {
        "regex"
    } else {
        match symbol_mode {
            Some(SymbolSearchMode::Definitions) => "symbol",
            Some(SymbolSearchMode::References) => "references",
            Some(SymbolSearchMode::Callers) => "callers",
            None => "hybrid",
        }
    };
    let summary = HitsSummary {
        workspace_root: &current_workspace.root,
        query,
        mode: mode_name,
        total_matches,
        truncated,
        warnings: &search_warnings,
        verbose,
    };
    let text = if file_name_only {
        render_file_paths_text(&summary, &grouped)
    } else {
        render_hits_text(&summary, &grouped)
    };
    let payload = if file_name_only {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": mode_name,
            "result_count": grouped.len(),
            "total_matches": total_matches,
            "truncated": truncated,
            "include": include_globs,
            "exclude": exclude_globs,
            "warnings": search_warnings,
            "file_paths": grouped.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
        })
    } else {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": mode_name,
            "result_count": grouped.len(),
            "total_matches": total_matches,
            "truncated": truncated,
            "include": include_globs,
            "exclude": exclude_globs,
            "warnings": search_warnings,
            "results": grouped,
        })
    };

    Ok(tool_success_result_with_text(payload, text))
}

/// Hit budget requested from the search layer for one hits-mode call.
///
/// Ranked modes return score-ordered chunks spread across files, so a small
/// multiple of `file_limit` fills the file list; the floor matches the CLI
/// default so the default MCP call costs the same as `ig <query>`. Enumerating
/// modes (literal, regex, references, callers) cluster many hits per file and
/// need a deeper budget to reach `file_limit` distinct files.
fn hits_mode_hit_budget(file_limit: usize, ranked_mode: bool) -> usize {
    let budget = if ranked_mode {
        file_limit
            .saturating_mul(RANKED_HIT_OVERFETCH)
            .max(crate::search::DEFAULT_SEARCH_LIMIT)
    } else {
        file_limit
            .saturating_mul(ENUMERATING_HIT_OVERFETCH)
            .max(MIN_ENUMERATING_HIT_BUDGET)
    };
    budget.min(crate::search::MAX_SEARCH_RESULT_LIMIT)
}

struct BoundedHits {
    files: Vec<FileSearchResult>,
    /// Distinct files matched before `file_limit` was applied. A lower bound
    /// when retrieval saturated `hit_budget`.
    total_matches: usize,
    /// Files were dropped, or retrieval saturated its hit budget so more files
    /// may exist.
    truncated: bool,
}

fn bound_hits_by_file(
    hits: &[SearchHit],
    file_limit: usize,
    hits_per_file: usize,
    hit_budget: usize,
) -> BoundedHits {
    let mut files = group_hits_by_file(hits, None);
    let total_matches = files.len();
    let budget_saturated = hits.len() >= hit_budget;
    let truncated = total_matches > file_limit || budget_saturated;
    files.truncate(file_limit);
    for file in &mut files {
        // With the retrieval budget exhausted, a dense file may hold matches
        // that were never retrieved; its counts are lower bounds, not totals.
        file.hit_count_is_lower_bound = budget_saturated;
        if file.hits.len() > hits_per_file {
            file.more_hits_in_file = file.hits.len() - hits_per_file;
            file.hits.truncate(hits_per_file);
        }
    }
    BoundedHits {
        files,
        total_matches,
        truncated,
    }
}

struct HitsSummary<'a> {
    workspace_root: &'a Path,
    query: &'a str,
    mode: &'a str,
    total_matches: usize,
    truncated: bool,
    warnings: &'a [String],
    verbose: bool,
}

fn render_hits_header(summary: &HitsSummary<'_>, shown: usize, out: &mut String) {
    use std::fmt::Write as _;
    if shown == 0 {
        let _ = writeln!(
            out,
            "No {} matches for \"{}\" in {}",
            summary.mode,
            summary.query,
            summary.workspace_root.display()
        );
    } else {
        // Retrieval that saturated its hit budget makes total_matches a lower
        // bound; say so instead of printing a misleading "3 of 3".
        let lower_bound = if summary.truncated && summary.total_matches <= shown {
            "+"
        } else {
            ""
        };
        let _ = write!(
            out,
            "{shown} of {}{lower_bound} files for \"{}\" ({}) in {}",
            summary.total_matches,
            summary.query,
            summary.mode,
            summary.workspace_root.display()
        );
        if summary.truncated {
            out.push_str("; truncated: narrow the query or scope path, or raise limit");
        }
        out.push('\n');
    }
    for warning in summary.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
}

/// Compact text rendering of grouped hits for LLM clients that read the text
/// block rather than `structuredContent`. Carries paths, line ranges, and
/// previews without JSON keys, escaping, scores, or per-hit path repetition.
fn render_hits_text(summary: &HitsSummary<'_>, files: &[FileSearchResult]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    render_hits_header(summary, files.len(), &mut out);
    for file in files {
        let lower_bound = if file.hit_count_is_lower_bound {
            "+"
        } else {
            ""
        };
        let _ = write!(
            out,
            "\n{}  ({}{lower_bound} hit{}",
            file.file_path.display(),
            file.hit_count,
            if file.hit_count == 1 && !file.hit_count_is_lower_bound {
                ""
            } else {
                "s"
            }
        );
        if file.more_hits_in_file > 0 {
            let _ = write!(
                out,
                ", {} shown, {}{lower_bound} more",
                file.hits.len(),
                file.more_hits_in_file
            );
        }
        out.push_str(")\n");
        for hit in &file.hits {
            if hit.start_line == hit.end_line {
                let _ = writeln!(out, "  L{}", hit.start_line);
            } else {
                let _ = writeln!(out, "  L{}-{}", hit.start_line, hit.end_line);
            }
            if summary.verbose && !hit.reason.is_empty() {
                let _ = writeln!(out, "    reason: {}", hit.reason.trim());
            }
            for line in hit.preview.lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
    }
    out
}

fn render_file_paths_text(summary: &HitsSummary<'_>, files: &[FileSearchResult]) -> String {
    let mut out = String::new();
    render_hits_header(summary, files.len(), &mut out);
    for file in files {
        out.push_str(&file.file_path.to_string_lossy());
        out.push('\n');
    }
    out
}

/// Detected framing mode for the stdio transport.
#[derive(Clone, Copy, PartialEq)]
enum FramingMode {
    /// Auto-detect on first line (initial state).
    Unknown,
    /// Newline-delimited JSON-RPC (mcp-cli, MCP Inspector).
    JsonLine,
    /// LSP-style Content-Length header framing.
    ContentLength,
}

/// Read a single line, bounded to MAX_MESSAGE_BYTES so a client that never
/// sends a newline can't grow memory without limit. Returns bytes read (0 = EOF).
fn read_line_capped<R: BufRead>(reader: &mut R, line: &mut String) -> Result<usize> {
    line.clear();
    let mut buf = Vec::new();
    let n = reader
        .take((MAX_MESSAGE_BYTES as u64) + 1)
        .read_until(b'\n', &mut buf)?;
    if buf.len() > MAX_MESSAGE_BYTES {
        bail!("request line exceeds maximum of {MAX_MESSAGE_BYTES} bytes");
    }
    line.push_str(&String::from_utf8_lossy(&buf));
    Ok(n)
}

fn read_message<R: BufRead>(reader: &mut R, mode: &mut FramingMode) -> Result<Option<Vec<u8>>> {
    // Read first non-empty line (skip blank lines between messages).
    let first_line = loop {
        let mut line = String::new();
        let bytes = read_line_capped(reader, &mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            break (trimmed, line);
        }
    };
    let (trimmed, _raw) = first_line;

    // Auto-detect framing: if first meaningful line starts with '{', it's bare JSON.
    if *mode == FramingMode::Unknown {
        if trimmed.starts_with('{') {
            *mode = FramingMode::JsonLine;
        } else {
            *mode = FramingMode::ContentLength;
        }
    }

    match *mode {
        FramingMode::JsonLine => {
            // The trimmed line IS the JSON payload.
            Ok(Some(trimmed.into_bytes()))
        }
        FramingMode::ContentLength => {
            // Parse header lines for Content-Length.
            let mut content_length: Option<usize> = None;
            let lower = trimmed.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                content_length = Some(value.trim().parse::<usize>()?);
            }

            // Read remaining headers until empty line.
            loop {
                let mut line = String::new();
                let bytes = read_line_capped(reader, &mut line)?;
                if bytes == 0 {
                    return Ok(None);
                }
                let t = line.trim_end_matches(['\r', '\n']);
                if t.is_empty() {
                    break;
                }
                let lower = t.to_ascii_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    content_length = Some(value.trim().parse::<usize>()?);
                }
            }

            let len = content_length.context("missing Content-Length header")?;
            if len > MAX_MESSAGE_BYTES {
                bail!("Content-Length {len} exceeds maximum of {MAX_MESSAGE_BYTES} bytes");
            }
            let mut payload = vec![0u8; len];
            reader.read_exact(&mut payload)?;
            Ok(Some(payload))
        }
        FramingMode::Unknown => unreachable!(),
    }
}

fn write_message<W: Write>(
    writer: &mut W,
    response: &JsonRpcResponse,
    mode: FramingMode,
) -> Result<()> {
    let payload = serde_json::to_vec(response)?;
    match mode {
        FramingMode::JsonLine | FramingMode::Unknown => {
            writer.write_all(&payload)?;
            writer.write_all(b"\n")?;
        }
        FramingMode::ContentLength => {
            write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
            writer.write_all(&payload)?;
        }
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    fn read_message_rejects_oversized_content_length() {
        // A huge Content-Length must be rejected, not allocated.
        let msg = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES as u64 + 1);
        let mut reader = std::io::BufReader::new(msg.as_bytes());
        let mut mode = FramingMode::Unknown;
        let result = read_message(&mut reader, &mut mode);
        assert!(result.is_err(), "oversized Content-Length must be rejected");
    }

    #[test]
    fn read_message_accepts_normal_content_length() {
        let body = "{\"jsonrpc\":\"2.0\"}";
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = std::io::BufReader::new(msg.as_bytes());
        let mut mode = FramingMode::Unknown;
        let payload = read_message(&mut reader, &mut mode).unwrap().unwrap();
        assert_eq!(payload, body.as_bytes());
    }

    #[test]
    #[serial]
    fn mcp_search_auto_indexes_and_respects_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let scoped = root.join("scoped");
        let other = root.join("other");
        std::fs::create_dir_all(&root).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::create_dir_all(&scoped).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        std::fs::write(
            scoped.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();
        std::fs::write(
            other.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(scoped.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: None,
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        let files = result
            .get("results")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
            .collect::<Vec<_>>();

        assert!(!files.is_empty());
        assert!(files.iter().all(|path| path.starts_with("scoped/")));
    }

    #[test]
    #[serial]
    fn mcp_raw_type_alias_matches_canonical_semantic_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("lib.rs"),
            r#"pub struct AccountManager;

impl AccountManager {
    /// Performs durable credential refresh with retry and backoff after an expired session.
    pub fn refresh_credentials(&self, token: &str) -> Result<(), String> {
        let _ = token;
        Ok(())
    }
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("decoy.md"),
            "request validation configuration is documented here\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }
        let workspace = Workspace::resolve(&root).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();
        crate::indexer::enhance_workspace_hash(&workspace, hash_model.as_ref()).unwrap();
        std::fs::write(workspace.watcher_pid_path(), std::process::id().to_string()).unwrap();

        let search = |type_filter: &str| {
            execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("secure account renewal strategy".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit: Some(10),
                hits_per_file: None,
                context: Some(2),
                type_filter: Some(type_filter.to_string()),
                regex: None,
                literal: None,
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: Some(false),
                verbose: None,
                skip_gitignore: None,
            })
            .map(|response| {
                let payload = tool_json_payload(&response);
                let results = payload["results"].as_array().unwrap();
                let paths = results
                    .iter()
                    .filter_map(|result| result["file_path"].as_str())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let sources = results
                    .iter()
                    .flat_map(|result| result["hits"].as_array().unwrap())
                    .flat_map(|hit| hit["sources"].as_array().unwrap())
                    .filter_map(serde_json::Value::as_str)
                    .map(ToString::to_string)
                    .collect::<std::collections::BTreeSet<_>>();
                (paths, sources)
            })
            .unwrap()
        };

        let (alias_paths, alias_sources) = search("rs");
        let (canonical_paths, canonical_sources) = search("rust");
        assert_eq!(alias_paths, canonical_paths);
        assert_eq!(alias_paths, vec!["lib.rs"]);
        assert_eq!(alias_sources, canonical_sources);
        assert!(alias_sources.contains("semantic"));
        assert!(alias_sources.contains("hash"));
    }

    #[test]
    #[serial]
    fn mcp_search_returns_budgeted_context_graph_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("tests")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"context-fixture\"\nversion = \"0.1.0\"\ndescription = \"refresh token expiration fixture\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::clock::now;\npub fn rotate_refresh_token() { now(); }\n",
        )
        .unwrap();
        std::fs::write(root.join("src/clock.rs"), "pub fn now() -> u64 { 42 }\n").unwrap();
        std::fs::write(
            root.join("src/session.rs"),
            "use crate::auth::rotate_refresh_token;\npub fn refresh() { rotate_refresh_token(); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("tests/auth_test.rs"),
            "#[test]\nfn regression_case() { assert!(true); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("README.md"),
            "Refresh token rotation is implemented in [auth](src/auth.rs).\n",
        )
        .unwrap();
        for args in [
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["add", "."],
            vec!["commit", "-qm", "base"],
            vec!["branch", "-M", "main"],
            vec!["switch", "-qc", "feature"],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::clock::now;\npub fn rotate_refresh_token() { now(); /* branch fix */ }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("rotate refresh token expiration".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: Some("context_pack".to_string()),
            budget_tokens: Some(4_000),
            since: Some("main".to_string()),
            limit: None,
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: None,
            file_name_only: None,
            verbose: None,
            skip_gitignore: None,
        })
        .unwrap();

        let payload = tool_json_payload(&response);
        assert_eq!(payload["mode"], "context");
        assert_eq!(payload["context_pack"]["budget_tokens"], 4_000);
        assert_eq!(payload["context_pack"]["change_scope"]["since"], "main");
        assert_eq!(
            payload["context_pack"]["change_scope"]["dirty_worktree"],
            true
        );
        assert!(
            payload["context_pack"]["used_tokens"].as_u64().unwrap() <= 4_000,
            "{payload:#}"
        );
        let coverage = &payload["context_pack"]["coverage"];
        assert!(
            coverage["dependencies"].as_u64().unwrap() >= 1,
            "{payload:#}"
        );
        assert!(coverage["dependents"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(coverage["tests"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(coverage["config"].as_u64().unwrap() >= 1, "{payload:#}");
        assert!(
            coverage["documentation"].as_u64().unwrap() >= 1,
            "{payload:#}"
        );
        assert_eq!(response["structuredContent"], payload);

        let filtered = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "rotate refresh token expiration",
                    "path": root,
                    "output": "context_pack",
                    "budget_tokens": 4000,
                    "include": "src/**"
                }
            }),
        )
        .unwrap();
        let filtered = tool_json_payload(&filtered);
        assert!(
            filtered["context_pack"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["file_path"].as_str().unwrap().starts_with("src/")),
            "{filtered:#}"
        );
        assert_eq!(filtered["context_pack"]["coverage"]["tests"], 0);
        assert_eq!(filtered["context_pack"]["coverage"]["config"], 0);
        assert_eq!(filtered["context_pack"]["coverage"]["documentation"], 0);
    }

    #[test]
    #[serial]
    fn mcp_context_pack_respects_gitignore_after_all_files_index() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::write(root.join(".gitignore"), "src/secret.rs\n").unwrap();
        std::fs::write(
            root.join("src/auth.rs"),
            "use crate::secret::load_seed;\npub fn rotate_refresh_token() { load_seed(); }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/secret.rs"),
            "pub fn load_seed() -> &'static str { \"private\" }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let indexed = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("load_seed".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(10),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: None,
            file_name_only: None,
            verbose: None,
            skip_gitignore: Some(true),
        })
        .unwrap();
        let indexed = tool_json_payload(&indexed);
        assert!(
            indexed["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["file_path"] == "src/secret.rs"),
            "{indexed:#}"
        );

        for (skip_gitignore, expects_secret) in [(None, false), (Some(true), true)] {
            let response = execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("rotate refresh token".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: Some("context_pack".to_string()),
                budget_tokens: Some(4_000),
                since: None,
                limit: None,
                hits_per_file: None,
                context: Some(2),
                type_filter: None,
                regex: None,
                literal: None,
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: None,
                verbose: None,
                skip_gitignore,
            })
            .unwrap();
            let payload = tool_json_payload(&response);
            let contains_secret = payload["context_pack"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["file_path"] == "src/secret.rs");
            assert_eq!(contains_secret, expects_secret, "{payload:#}");
        }
    }

    #[test]
    #[serial]
    fn mcp_search_omits_reason_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        let hits = result
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|files| files.first())
            .and_then(|file| file.get("hits"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        assert!(!hits.is_empty());
        assert!(hits.iter().all(|hit| hit.get("reason").is_none()));
    }

    #[test]
    #[serial]
    fn mcp_search_respects_include_exclude_globs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("match.md"),
            "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let include_only = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.md".to_string()),
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let include_payload = tool_json_payload(&include_only);
        let file_paths = include_payload
            .get("file_paths")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            file_paths,
            vec![Value::String("match.md".to_string())],
            "include glob should keep only markdown results"
        );

        let brace_filtered = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.{rs,md},*.txt".to_string()),
            exclude: Some("*.rs".to_string()),
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();
        assert_eq!(
            tool_json_payload(&brace_filtered)["file_paths"],
            serde_json::json!(["match.md"])
        );

        let include_and_exclude = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: Some("*.md".to_string()),
            exclude: Some("match.md".to_string()),
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let excluded_payload = tool_json_payload(&include_and_exclude);
        assert_eq!(
            excluded_payload
                .get("file_paths")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or_default(),
            0
        );
    }

    #[test]
    fn mcp_initialize_returns_protocol_version_and_capabilities() {
        let result = dispatch("initialize", json!({})).unwrap();
        assert_eq!(result["protocolVersion"], LEGACY_PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "ig");
        let version = result["serverInfo"]["version"].as_str().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn mcp_initialize_negotiates_current_protocol_version() {
        let result = dispatch(
            "initialize",
            json!({"protocolVersion": LATEST_PROTOCOL_VERSION}),
        )
        .unwrap();
        assert_eq!(result["protocolVersion"], LATEST_PROTOCOL_VERSION);

        let fallback = dispatch(
            "initialize",
            json!({"protocolVersion": "unsupported-version"}),
        )
        .unwrap();
        assert_eq!(fallback["protocolVersion"], LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn mcp_tools_list_returns_ig_search() {
        let result = dispatch("tools/list", json!({})).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "ig_search");
        let schema = &tools[0]["inputSchema"];
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["regex"].is_object());
        assert!(schema["properties"]["symbol"].is_object());
        assert!(schema["properties"]["refs"].is_object());
        assert!(schema["properties"]["callers"].is_object());
        assert_eq!(schema["properties"]["output"]["default"], "hits");
        assert_eq!(
            schema["properties"]["output"]["enum"],
            json!(["hits", "context_pack"])
        );
        assert_eq!(schema["properties"]["budget_tokens"]["minimum"], 256);
        assert_eq!(schema["properties"]["budget_tokens"]["maximum"], 131_072);
        assert!(
            schema["properties"]["budget_tokens"]
                .get("default")
                .is_none()
        );
        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 1000);
        assert!(
            schema["properties"]["limit"]["description"]
                .as_str()
                .unwrap()
                .contains("defaults to 10")
        );
        assert_eq!(schema["properties"]["hits_per_file"]["minimum"], 1);
        assert_eq!(schema["properties"]["hits_per_file"]["maximum"], 100);
        assert_eq!(schema["properties"]["hits_per_file"]["default"], 3);
        assert_eq!(schema["properties"]["context"]["minimum"], 0);
        assert_eq!(schema["properties"]["context"]["maximum"], 100);
        assert_eq!(schema["properties"]["context"]["default"], 2);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("query")));
        assert!(tools[0]["outputSchema"].is_object());
        assert_eq!(
            tools[0]["outputSchema"]["properties"]["context_pack"]["additionalProperties"],
            false
        );
        let output_properties = &tools[0]["outputSchema"]["properties"];
        assert_eq!(output_properties["total_matches"]["type"], "integer");
        assert_eq!(output_properties["truncated"]["type"], "boolean");
        assert_eq!(
            output_properties["results"]["items"]["properties"]["more_hits_in_file"]["type"],
            "integer"
        );
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(tools[1]["name"], "ig_status");
        assert!(tools[1]["outputSchema"].is_object());
        assert_eq!(tools[1]["inputSchema"]["additionalProperties"], false);
        assert_eq!(tools[1]["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn mcp_known_tool_errors_are_recoverable_tool_results() {
        let response = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {}
            }),
        )
        .unwrap();
        assert_eq!(response["isError"], true);
        assert!(
            response["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("missing required argument: query")
        );

        let conflicting_modes = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle",
                    "literal": true,
                    "regex": true
                }
            }),
        )
        .unwrap();
        assert_eq!(conflicting_modes["isError"], true);
        assert!(
            conflicting_modes["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("mutually exclusive")
        );

        for (arguments, expected) in [
            (
                json!({"query": "needle", "output": "unknown"}),
                "output must be hits or context_pack",
            ),
            (
                json!({"query": "needle", "budget_tokens": 8000}),
                "budget_tokens requires output=context_pack",
            ),
            (
                json!({"query": "needle", "output": "context_pack", "budget_tokens": 255}),
                "budget_tokens must be between 256 and 131072",
            ),
            (
                json!({"query": "needle", "output": "context_pack", "literal": true}),
                "output=context_pack cannot be combined",
            ),
        ] {
            let response = dispatch(
                "tools/call",
                json!({"name": "ig_search", "arguments": arguments}),
            )
            .unwrap();
            assert_eq!(response["isError"], true);
            assert!(
                response["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(expected),
                "{response:#}"
            );
        }

        let unknown_argument = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle",
                    "limt": 5
                }
            }),
        )
        .unwrap();
        assert_eq!(unknown_argument["isError"], true);
        assert!(
            unknown_argument["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("unknown field `limt`")
        );

        let unknown_status_argument = dispatch(
            "tools/call",
            json!({
                "name": "ig_status",
                "arguments": {"unexpected": true}
            }),
        )
        .unwrap();
        assert_eq!(unknown_status_argument["isError"], true);
    }

    #[test]
    fn mcp_returns_standard_json_rpc_error_codes() {
        let unknown_method = handle_request(JsonRpcRequest {
            id: Some(json!(1)),
            method: "tools/nonexistent".to_string(),
            params: json!({}),
        })
        .unwrap();
        let error = unknown_method.error.unwrap();
        assert_eq!(error.code, -32601);
        assert!(error.message.contains("method not found"));

        for params in [json!({}), json!({"name": "unknown_tool", "arguments": {}})] {
            let invalid_params = handle_request(JsonRpcRequest {
                id: Some(json!(2)),
                method: "tools/call".to_string(),
                params,
            })
            .unwrap();
            assert_eq!(invalid_params.error.unwrap().code, -32602);
        }
    }

    #[test]
    #[serial]
    fn mcp_search_regex_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.md"),
            "before\ncalculate_tax amount\nafter\n",
        )
        .unwrap();
        std::fs::write(
            root.join("match.rs"),
            "before\npub fn calculate_tax() {}\nafter\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some(r"calculate_\w+".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: Some("markdown".to_string()),
            regex: Some(true),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        assert_eq!(result["mode"], "regex");
        assert_eq!(result["result_count"], 1);
        let file = &result["results"][0];
        assert_eq!(file["file_path"], "match.md");
        assert_eq!(file["hits"][0]["start_line"], 1);
        assert_eq!(file["hits"][0]["end_line"], 3);
        assert_eq!(
            file["hits"][0]["preview"],
            "before\ncalculate_tax amount\nafter"
        );
    }

    #[test]
    #[serial]
    fn mcp_search_literal_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let response = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate_tax".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: Some(true),
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let result = tool_json_payload(&response);
        assert_eq!(result["mode"], "literal");
        let count = result["result_count"].as_u64().unwrap();
        assert!(count > 0, "literal search should find results");
    }

    #[test]
    #[serial]
    fn mcp_search_symbol_modes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join("match.rs"),
            "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n\
             pub fn checkout() -> f64 { calculate_tax(10.0) }\n\
             pub fn applyFilter(value: bool) -> bool { value }\n\
             pub fn render() -> bool { applyFilter(true) }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        for (symbol, refs, callers, expected_mode) in [
            (true, false, false, "symbol"),
            (false, true, false, "references"),
            (false, false, true, "callers"),
        ] {
            let response = execute_ivygrep_search(IvygrepSearchArgs {
                query: Some("calculate_tax".to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit: Some(5),
                hits_per_file: None,
                context: Some(2),
                type_filter: None,
                regex: None,
                literal: None,
                symbol: Some(symbol),
                refs: Some(refs),
                callers: Some(callers),
                include: None,
                exclude: None,
                first_line_only: Some(false),
                file_name_only: Some(false),
                verbose: Some(false),
                skip_gitignore: None,
            })
            .unwrap();
            let result = tool_json_payload(&response);
            assert_eq!(result["mode"], expected_mode);
            assert!(result["result_count"].as_u64().unwrap() > 0);
        }

        for query in ["applyFilter", "applyFilter()"] {
            for (refs, callers, expected_mode) in
                [(true, false, "references"), (false, true, "callers")]
            {
                let response = execute_ivygrep_search(IvygrepSearchArgs {
                    query: Some(query.to_string()),
                    path: Some(root.to_string_lossy().to_string()),
                    output: None,
                    budget_tokens: None,
                    since: None,
                    limit: Some(5),
                    hits_per_file: None,
                    context: Some(2),
                    type_filter: None,
                    regex: None,
                    literal: None,
                    symbol: None,
                    refs: Some(refs),
                    callers: Some(callers),
                    include: None,
                    exclude: None,
                    first_line_only: Some(false),
                    file_name_only: Some(false),
                    verbose: Some(false),
                    skip_gitignore: None,
                })
                .unwrap();
                let result = tool_json_payload(&response);
                assert_eq!(result["mode"], expected_mode);
                assert!(result["result_count"].as_u64().unwrap() > 0);
            }
        }
    }

    #[test]
    fn mcp_search_rejects_conflicting_modes() {
        let error = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate_tax".to_string()),
            path: None,
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: Some(true),
            symbol: None,
            refs: Some(true),
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap_err();
        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    #[serial]
    fn mcp_auto_index_defers_vector_enrichment() {
        // MCP auto-index must commit lexical stores without building ANN
        // vectors inline. Multi-million chunk hash HNSW construction takes
        // minutes and must run in background enhancement.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        for i in 0..6 {
            std::fs::write(
                root.join(format!("file_{i}.rs")),
                format!("pub fn calculate_tax_{i}(amount: f64) -> f64 {{ amount * 0.2 }}\n"),
            )
            .unwrap();
        }

        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        // Don't spawn the background neural enhancement subprocess during tests.
        unsafe { std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1") };

        let _ = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("calculate tax".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            output: None,
            budget_tokens: None,
            since: None,
            limit: Some(5),
            hits_per_file: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
            symbol: None,
            refs: None,
            callers: None,
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
            skip_gitignore: None,
        })
        .unwrap();

        let workspace = crate::workspace::Workspace::resolve(&root).unwrap();
        let store = crate::vector_store::VectorStore::open_readonly(
            &workspace.vector_path(),
            256,
            crate::vector_store::ScalarKind::F16,
            crate::vector_store::VectorTier::Hash,
        )
        .expect("hash vector store (vectors.usearch) should open at 256 dims");
        assert_eq!(
            store.dimensions(),
            256,
            "MCP auto-index must initialize hash store at 256 dimensions"
        );
        assert_eq!(store.size(), 0, "MCP auto-index must defer hash ANN build");

        unsafe { std::env::remove_var("IVYGREP_NO_AUTOSPAWN") };
    }

    #[test]
    fn mcp_query_model_caches_neural_but_not_hash_fallback() {
        // #57: a successfully-loaded neural model is cached once per process.
        // The hash fallback is intentionally NOT cached, so a transient neural
        // failure can't pin the process to hash embeddings forever.
        let a = mcp_query_model();
        let b = mcp_query_model();
        if a.dimensions() == 384 {
            // Neural model available — must be the same cached instance.
            assert!(
                Arc::ptr_eq(&a, &b),
                "neural query model should be cached, not reloaded per call"
            );
        } else {
            // Neural unavailable (not compiled in / load failed): hash fallback
            // is rebuilt each call so neural can be retried later. Just ensure a
            // usable model comes back.
            assert_eq!(a.dimensions(), b.dimensions());
        }
    }

    #[test]
    #[serial]
    fn mcp_search_uses_hash_model_until_neural_vectors_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        std::fs::write(tmp.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();

        let workspace = Workspace::resolve(tmp.path()).unwrap();
        let index_model = create_hash_model();
        index_workspace(&workspace, index_model.as_ref()).unwrap();

        assert!(workspace.needs_neural_enhancement());
        assert_eq!(mcp_search_model(&workspace).dimensions(), 256);
    }

    fn synthetic_hit(file: &str, line: usize, score: f32) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(file),
            start_line: line,
            end_line: line + 2,
            preview: format!("line {line}\nbody {line}\nend {line}"),
            reason: String::new(),
            score,
            sources: vec!["literal".to_string()],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn hits_mode_hit_budget_scales_with_files_and_mode() {
        assert_eq!(
            hits_mode_hit_budget(DEFAULT_HITS_FILE_LIMIT, true),
            crate::search::DEFAULT_SEARCH_LIMIT
        );
        assert_eq!(
            hits_mode_hit_budget(1, true),
            crate::search::DEFAULT_SEARCH_LIMIT
        );
        assert_eq!(hits_mode_hit_budget(40, true), 200);
        assert_eq!(
            hits_mode_hit_budget(DEFAULT_HITS_FILE_LIMIT, false),
            MIN_ENUMERATING_HIT_BUDGET
        );
        assert_eq!(hits_mode_hit_budget(25, false), 500);
        assert_eq!(
            hits_mode_hit_budget(1000, false),
            crate::search::MAX_SEARCH_RESULT_LIMIT
        );
        assert_eq!(
            hits_mode_hit_budget(1000, true),
            crate::search::MAX_SEARCH_RESULT_LIMIT
        );
    }

    #[test]
    fn bound_hits_by_file_applies_file_limit_and_per_file_cap() {
        let mut hits = (1..=6)
            .map(|line| synthetic_hit("busy.rs", line * 10, 1.0))
            .collect::<Vec<_>>();
        hits.push(synthetic_hit("quiet.rs", 1, 1.0));
        hits.push(synthetic_hit("other.rs", 1, 1.0));

        let saturated = bound_hits_by_file(&hits, 2, 3, hits.len());
        assert!(saturated.truncated);
        assert!(
            saturated
                .files
                .iter()
                .all(|file| file.hit_count_is_lower_bound),
            "an exhausted retrieval budget makes every per-file count a lower bound"
        );
        let serialized = serde_json::to_value(&saturated.files).unwrap();
        assert_eq!(serialized[0]["hit_count_is_lower_bound"], true);

        let bounded = bound_hits_by_file(&hits, 2, 3, 1000);
        assert_eq!(bounded.total_matches, 3);
        assert!(bounded.truncated, "third file was dropped");
        assert!(
            bounded
                .files
                .iter()
                .all(|file| !file.hit_count_is_lower_bound)
        );
        assert_eq!(bounded.files.len(), 2);
        assert_eq!(bounded.files[0].file_path, PathBuf::from("busy.rs"));
        assert_eq!(bounded.files[0].hit_count, 6);
        assert_eq!(bounded.files[0].hits.len(), 3);
        assert_eq!(bounded.files[0].more_hits_in_file, 3);
        assert_eq!(bounded.files[1].more_hits_in_file, 0);
        let serialized = serde_json::to_value(&bounded.files).unwrap();
        assert_eq!(serialized[0]["more_hits_in_file"], 3);
        assert!(
            serialized[1].get("more_hits_in_file").is_none(),
            "uncut files omit more_hits_in_file: {serialized:#}"
        );
        assert!(
            serialized[0].get("hit_count_is_lower_bound").is_none(),
            "exact counts omit the lower-bound flag: {serialized:#}"
        );

        let complete = bound_hits_by_file(&hits, 10, 10, 1000);
        assert_eq!(complete.total_matches, 3);
        assert!(!complete.truncated);
        assert_eq!(complete.files[0].hits.len(), 6);
        assert_eq!(complete.files[0].more_hits_in_file, 0);

        // Retrieval that saturated its hit budget may hide more files.
        let saturated = bound_hits_by_file(&hits, 10, 10, hits.len());
        assert_eq!(saturated.total_matches, 3);
        assert!(saturated.truncated);
    }

    #[test]
    fn render_hits_text_is_compact_and_self_contained() {
        let mut hits = (1..=4)
            .map(|line| synthetic_hit("src/busy.rs", line * 10, 1.0))
            .collect::<Vec<_>>();
        hits.push(synthetic_hit("src/quiet.rs", 7, 1.0));
        let bounded = bound_hits_by_file(&hits, 10, 3, 1000);
        let summary = HitsSummary {
            workspace_root: Path::new("/repo"),
            query: "needle",
            mode: "literal",
            total_matches: bounded.total_matches,
            truncated: bounded.truncated,
            warnings: &["one workspace failed".to_string()],
            verbose: false,
        };
        let text = render_hits_text(&summary, &bounded.files);
        assert!(
            text.starts_with("2 of 2 files for \"needle\" (literal) in /repo\n"),
            "{text}"
        );
        assert!(text.contains("warning: one workspace failed\n"), "{text}");
        assert!(
            text.contains("src/busy.rs  (4 hits, 3 shown, 1 more)\n"),
            "{text}"
        );
        assert!(text.contains("src/quiet.rs  (1 hit)\n"), "{text}");
        assert!(
            text.contains("  L10-12\n    line 10\n    body 10\n"),
            "{text}"
        );
        assert!(!text.contains("\"file_path\""), "{text}");
        let json = serde_json::to_string(&bounded.files).unwrap();
        assert!(
            text.len() * 4 < json.len() * 3,
            "text ({}) should be materially smaller than JSON ({})",
            text.len(),
            json.len()
        );

        let saturated = render_hits_text(
            &HitsSummary {
                truncated: true,
                warnings: &[],
                ..summary
            },
            &bounded.files,
        );
        assert!(
            saturated.starts_with("2 of 2+ files for \"needle\" (literal) in /repo; truncated"),
            "{saturated}"
        );

        let empty = render_hits_text(
            &HitsSummary {
                total_matches: 0,
                truncated: false,
                warnings: &[],
                ..summary
            },
            &[],
        );
        assert_eq!(empty, "No literal matches for \"needle\" in /repo\n");
    }

    #[test]
    #[serial]
    fn mcp_hits_mode_bounds_files_and_hits_per_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        // One busy file with six matching lines plus fourteen single-match files.
        let busy = (0..6)
            .map(|i| format!("pub fn needle_token_{i}() -> u32 {{ {i} }}\n"))
            .collect::<String>();
        std::fs::write(root.join("busy.rs"), busy).unwrap();
        for i in 0..14 {
            std::fs::write(
                root.join(format!("file_{i:02}.rs")),
                format!("pub fn other_{i}() -> u32 {{ needle_token_{i}() }}\n"),
            )
            .unwrap();
        }
        std::fs::write(
            root.join("unique.rs"),
            "pub fn lonely_marker_fn() -> u32 { 7 }\n",
        )
        .unwrap();

        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
        }

        let search = |query: &str, limit: Option<usize>, hits_per_file: Option<usize>| {
            execute_ivygrep_search(IvygrepSearchArgs {
                query: Some(query.to_string()),
                path: Some(root.to_string_lossy().to_string()),
                output: None,
                budget_tokens: None,
                since: None,
                limit,
                hits_per_file,
                context: Some(0),
                type_filter: None,
                regex: None,
                literal: Some(true),
                symbol: None,
                refs: None,
                callers: None,
                include: None,
                exclude: None,
                first_line_only: None,
                file_name_only: None,
                verbose: None,
                skip_gitignore: None,
            })
            .unwrap()
        };

        // (a) omitted limit: server-side default of 10 files, not 500 hits.
        let response = search("needle_token", None, None);
        let payload = tool_json_payload(&response);
        let results = payload["results"].as_array().unwrap();
        assert_eq!(results.len(), DEFAULT_HITS_FILE_LIMIT, "{payload:#}");
        assert_eq!(payload["result_count"], DEFAULT_HITS_FILE_LIMIT);
        // (c) truncation signals.
        assert_eq!(payload["total_matches"], 15, "{payload:#}");
        assert_eq!(payload["truncated"], true);
        // (b) per-file cap on the busiest file.
        let busy = &results[0];
        assert_eq!(busy["file_path"], "busy.rs", "{payload:#}");
        assert_eq!(busy["hit_count"], 6);
        assert_eq!(
            busy["hits"].as_array().unwrap().len(),
            DEFAULT_HITS_PER_FILE
        );
        assert_eq!(busy["more_hits_in_file"], 3);
        assert!(
            results[1..]
                .iter()
                .all(|file| file.get("more_hits_in_file").is_none()),
            "{payload:#}"
        );
        // (d) text block is a compact rendering, not the JSON payload again.
        let text = tool_text(&response);
        let structured_len = serde_json::to_string(&response["structuredContent"])
            .unwrap()
            .len();
        assert!(!text.trim_start().starts_with('{'), "{text}");
        assert!(!text.contains("\"file_path\""), "{text}");
        assert!(!text.contains("structuredContent"), "{text}");
        assert!(
            text.contains("busy.rs  (6 hits, 3 shown, 3 more)"),
            "{text}"
        );
        assert!(text.contains("truncated"), "{text}");
        assert!(
            text.len() * 2 < structured_len,
            "text {} bytes vs structuredContent {structured_len} bytes",
            text.len()
        );

        // limit counts files, not hits: two files, busy.rs still capped.
        let payload = tool_json_payload(&search("needle_token", Some(2), None));
        let paths = payload["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["file_path"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(paths.len(), 2, "{payload:#}");
        assert_eq!(payload["results"][0]["hits"].as_array().unwrap().len(), 3);
        assert_eq!(payload["truncated"], true);

        // hits_per_file lifts the cap.
        let payload = tool_json_payload(&search("needle_token", Some(1), Some(10)));
        assert_eq!(payload["results"][0]["file_path"], "busy.rs");
        assert_eq!(payload["results"][0]["hits"].as_array().unwrap().len(), 6);
        assert!(payload["results"][0].get("more_hits_in_file").is_none());

        // Nothing cut: truncated is false and total_matches equals result_count.
        let payload = tool_json_payload(&search("lonely_marker_fn", None, None));
        assert_eq!(payload["result_count"], 1);
        assert_eq!(payload["total_matches"], 1);
        assert_eq!(payload["truncated"], false);

        // hits_per_file is validated and hits-mode only.
        let error = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {"query": "needle_token", "path": root, "hits_per_file": 0}
            }),
        )
        .unwrap();
        assert_eq!(error["isError"], true);
        assert!(
            error["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("hits_per_file must be between 1 and 100"),
            "{error:#}"
        );
        let error = dispatch(
            "tools/call",
            json!({
                "name": "ig_search",
                "arguments": {
                    "query": "needle_token",
                    "path": root,
                    "output": "context_pack",
                    "hits_per_file": 3
                }
            }),
        )
        .unwrap();
        assert_eq!(error["isError"], true);
        assert!(
            error["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("hits_per_file requires output=hits"),
            "{error:#}"
        );

        unsafe { std::env::remove_var("IVYGREP_NO_AUTOSPAWN") };
    }

    fn tool_text(response: &Value) -> &str {
        response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|v| v.as_str())
            .expect("tool response content text")
    }

    fn tool_json_payload(response: &Value) -> Value {
        let content = tool_text(response);
        if let Some(payload) = response.get("structuredContent") {
            assert!(payload.is_object(), "structuredContent must be an object");
            assert!(!content.trim().is_empty(), "text content must not be empty");
            return payload.clone();
        }
        serde_json::from_str(content).expect("valid JSON payload")
    }

    #[test]
    fn indexing_status_result_is_non_error_with_progress_estimate() {
        assert_eq!(parse_file_progress("1200/4000"), Some((1200, 4000)));
        assert_eq!(parse_file_progress("scanning"), None);
        // 1200 files in 30 s leaves 2800 files at 40/s: 70 s, clamped to 60.
        assert_eq!(estimate_retry_after_secs(Some(1200), Some(4000), 30), 60);
        assert_eq!(estimate_retry_after_secs(Some(3900), Some(4000), 30), 5);
        assert_eq!(
            estimate_retry_after_secs(None, None, 30),
            DEFAULT_INDEX_RETRY_AFTER_SECS
        );

        let result = indexing_tool_result(json!({
            "status": "indexing",
            "workspace_root": "/tmp/repo",
            "progress": {"phase": "indexing", "done": 1200, "total": 4000, "percent": 30.0},
            "elapsed_secs": 30,
            "retry_after_secs": 60,
            "message": "Index in progress; call again later."
        }))
        .unwrap();
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["status"], "indexing");
        let text = result["content"][0]["text"].as_str().unwrap();
        let (summary, payload) = text.split_once('\n').unwrap();
        assert!(summary.contains("1200/4000 files (30.0%)"), "{summary}");
        assert!(summary.contains("again in ~60s"), "{summary}");
        let payload: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(payload, result["structuredContent"]);
    }

    #[test]
    fn search_output_schema_accepts_indexing_status_branch() {
        let schema = search_output_schema();
        let branches = schema["oneOf"].as_array().unwrap();
        assert_eq!(branches.len(), 2);
        assert!(
            branches[1]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("status"))
        );
        assert_eq!(schema["properties"]["status"]["enum"], json!(["indexing"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn mcp_status_returns_projects() {
        let response = dispatch(
            "tools/call",
            json!({
                "name": "ig_status",
                "arguments": {}
            }),
        )
        .unwrap();
        let payload = tool_json_payload(&response);
        let workspaces = payload
            .get("workspaces")
            .and_then(|v| v.as_array())
            .unwrap();
        let _ = workspaces.len();
        assert_eq!(response["isError"], false);
        assert!(response["structuredContent"].is_object());
    }
}
