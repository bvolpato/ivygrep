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
use crate::indexer::{
    index_workspace, maybe_complete_neural_for_small_workspace, workspace_is_indexed,
};
use crate::path_glob::parse_glob_csv;
use crate::protocol::group_hits_by_file;
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search, literal_search};
use crate::workspace::resolve_workspace_and_scope;

const JSONRPC_VERSION: &str = "2.0";
const TOOL_IG_SEARCH: &str = "ig_search";
const TOOL_IG_STATUS: &str = "ig_status";

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

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct IvygrepSearchArgs {
    query: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
    context: Option<usize>,
    #[serde(rename = "type")]
    type_filter: Option<String>,
    regex: Option<bool>,
    literal: Option<bool>,
    include: Option<String>,
    exclude: Option<String>,
    first_line_only: Option<bool>,
    file_name_only: Option<bool>,
    verbose: Option<bool>,
    skip_gitignore: Option<bool>,
}

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
                code: -32000,
                message: err.to_string(),
            }),
        }),
    }
}

fn dispatch(method: &str, params: Value) -> Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "ig",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Use ig_search(query, path) to run local semantic code search. If path is a subdirectory or file, results are restricted to that scope. Use ig_status() to see the indexing status of your workspaces."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": [search_tool_schema(), status_tool_schema()]})),
        "tools/call" => run_tool_call(params),
        "notifications/initialized" => Ok(json!({})),
        "shutdown" => Ok(json!({})),
        other => bail!("unsupported method: {other}"),
    }
}

fn search_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_SEARCH,
        "description": "Hybrid semantic+lexical code search. Auto-indexes on first query. Respects .gitignore and restricts results to the provided path scope.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural-language or keyword query."},
                "path": {"type": "string", "description": "Workspace path, subdirectory, or file path. Defaults to current directory."},
                "limit": {"type": "integer", "minimum": 1, "description": "Max number of returned files."},
                "context": {"type": "integer", "minimum": 0, "description": "Context lines around focused line."},
                "type": {"type": "string", "description": "Language filter - accepts names (rust, python), extensions (rs, py, md), or aliases (c++, bash, js)."},
                "regex": {"type": "boolean", "description": "Use regex mode (walks raw files, no index — slow on large repos). Prefer 'literal' for exact matches."},
                "literal": {"type": "boolean", "description": "Fast exact-match search backed by the index. Deterministic, orders of magnitude faster than regex."},
                "include": {"type": "string", "description": "Comma-separated include globs, e.g. \"*.md,src/**/*.rs\"."},
                "exclude": {"type": "string", "description": "Comma-separated exclude globs, e.g. \"target/**,*.lock\"."},
                "first_line_only": {"type": "boolean", "description": "Return only the first non-empty preview line for each hit."},
                "file_name_only": {"type": "boolean", "description": "Return only file paths (no hit details)."},
                "verbose": {"type": "boolean", "description": "Include reason pointers in JSON output."},
                "skip_gitignore": {"type": "boolean", "description": "Include files ignored by .gitignore."}
            },
            "required": ["query"]
        }
    })
}

fn status_tool_schema() -> Value {
    json!({
        "name": TOOL_IG_STATUS,
        "description": "Returns the list of indexed projects (workspaces) and their current indexing status, detailing if they are ready to query.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    })
}

fn run_tool_call(params: Value) -> Result<Value> {
    let call: ToolCallParams = serde_json::from_value(params)?;
    if call.name == TOOL_IG_SEARCH {
        let args: IvygrepSearchArgs = serde_json::from_value(call.arguments)?;
        execute_ivygrep_search(args)
    } else if call.name == TOOL_IG_STATUS {
        execute_ivygrep_status()
    } else {
        bail!("unknown tool: {}", call.name);
    }
}

fn execute_ivygrep_status() -> Result<Value> {
    let workspaces = crate::workspace::list_workspaces()?;

    let mut projects = Vec::new();
    for ws in workspaces {
        let ready_to_query = ws.chunk_count > 0 && ws.last_indexed_at_unix.is_some();
        let status_msg = if ready_to_query {
            if ws.enhancing_in_progress {
                "Ready to query (Neural enhancement in progress)"
            } else if ws.enhancing_stalled {
                "Ready to query (Neural enhancement stalled)"
            } else if !ws.has_neural_vectors {
                "Ready to query (Lexical only)"
            } else {
                "Ready to query"
            }
        } else if ws.indexing_in_progress {
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
            "indexing_in_progress": ws.indexing_in_progress,
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

    let text = serde_json::to_string_pretty(&payload)?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": false
    }))
}

/// The neural query model for this MCP process, loaded once and reused across
/// requests.
///
/// `serve_stdio` is a long-lived server, so reconstructing the ONNX/Candle model
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

fn execute_ivygrep_search(args: IvygrepSearchArgs) -> Result<Value> {
    let query = args
        .query
        .as_deref()
        .context("missing required argument: query")?;

    let input_path = match args.path {
        Some(path) => PathBuf::from(path),
        None => env::current_dir()?,
    };

    let (current_workspace, scope_filter) = resolve_workspace_and_scope(Path::new(&input_path))?;

    // MCP search is intentionally scoped to a single workspace (the provided
    // `path`). Cross-workspace ("all indices") search is not supported here:
    // an agent could otherwise read source from unrelated indexed projects
    // outside its intended working directory.
    if !workspace_is_indexed(&current_workspace) {
        // Auto-index with the fast HASH model (mirrors the daemon's Index
        // handler). Neural embeddings are built later by a background
        // subprocess, so the first query returns quickly even on very large
        // repos instead of blocking on inline ONNX inference across the whole
        // tree — which on big repos never completes in any usable time and,
        // run by several MCP clients at once, saturates the host. See #56.
        let index_model = create_hash_model();
        let _summary = index_workspace(&current_workspace, index_model.as_ref())?;
    }
    let workspace = current_workspace.clone();
    let _ = workspace.cleanup_stale_legacy_runtime_files();

    let include_globs = parse_glob_csv(args.include.as_deref());
    let exclude_globs = parse_glob_csv(args.exclude.as_deref());

    let mut hits = if args.literal.unwrap_or(false) {
        literal_search(
            &workspace,
            query,
            &SearchOptions {
                limit: args.limit,
                context: args.context.unwrap_or(2),
                type_filter: args.type_filter.clone(),
                include_globs: include_globs.clone(),
                exclude_globs: exclude_globs.clone(),
                scope_filter: scope_filter.clone(),
                skip_gitignore: args.skip_gitignore.unwrap_or(false),
                progress_tx: None,
                cancel_token: None,
            },
        )?
    } else if args.regex.unwrap_or(false) {
        regex_search(
            &workspace,
            query,
            args.limit,
            scope_filter.as_ref(),
            &include_globs,
            &exclude_globs,
            args.skip_gitignore.unwrap_or(false),
        )?
    } else {
        // Neural model is loaded once per process and only needed to embed the
        // query (literal/regex modes skip it entirely). See #57.
        let model = mcp_query_model();
        let _ = maybe_complete_neural_for_small_workspace(&workspace);
        // Build neural vectors for larger workspaces in the background (a niced
        // subprocess) so search quality improves over time without blocking
        // this request. Small workspaces are completed inline above.
        if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
            && workspace.needs_neural_enhancement()
        {
            let _ = workspace.trigger_background_enhancement();
        }
        hybrid_search(
            &workspace,
            query,
            Some(model.as_ref()),
            &SearchOptions {
                limit: args.limit,
                context: args.context.unwrap_or(2),
                type_filter: args.type_filter.clone(),
                include_globs: include_globs.clone(),
                exclude_globs: exclude_globs.clone(),
                scope_filter: scope_filter.clone(),
                skip_gitignore: args.skip_gitignore.unwrap_or(false),
                progress_tx: None,
                cancel_token: None,
            },
        )?
    };

    if !args.literal.unwrap_or(false) && !args.regex.unwrap_or(false) {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    if let Some(l) = args.limit {
        hits.truncate(l);
    }

    let mut grouped = group_hits_by_file(&hits, args.limit);
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

    let payload = if file_name_only {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": if args.literal.unwrap_or(false) { "literal" } else if args.regex.unwrap_or(false) { "regex" } else { "hybrid" },
            "result_count": grouped.len(),
            "include": include_globs,
            "exclude": exclude_globs,
            "file_paths": grouped.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
        })
    } else {
        json!({
            "workspace_root": current_workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": if args.literal.unwrap_or(false) { "literal" } else if args.regex.unwrap_or(false) { "regex" } else { "hybrid" },
            "result_count": grouped.len(),
            "include": include_globs,
            "exclude": exclude_globs,
            "results": grouped,
        })
    };

    let text = serde_json::to_string(&payload)?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": false
    }))
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
        std::fs::create_dir_all(root.join(".git")).unwrap();
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
            limit: None,
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
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
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
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
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
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

        let include_and_exclude = execute_ivygrep_search(IvygrepSearchArgs {
            query: Some("applyFilter".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
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
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "ig");
        let version = result["serverInfo"]["version"].as_str().unwrap();
        assert!(!version.is_empty());
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
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("query")));
        assert_eq!(tools[1]["name"], "ig_status");
    }

    #[test]
    fn mcp_unknown_method_returns_error() {
        let result = dispatch("tools/nonexistent", json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unsupported method")
        );
    }

    #[test]
    #[serial]
    fn mcp_search_regex_mode() {
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
            query: Some(r"calculate_\w+".to_string()),
            path: Some(root.to_string_lossy().to_string()),
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: Some(true),
            literal: None,
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
        let count = result["result_count"].as_u64().unwrap();
        assert!(count > 0, "regex search should find results");
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
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: None,
            literal: Some(true),
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
    fn mcp_auto_index_builds_hash_vectors_not_neural() {
        // Regression guard for #56: the MCP auto-index must use the fast HASH
        // model (256-dim `vectors.usearch`), not the neural model (384-dim)
        // inline. Building neural inline blocks the first query for minutes on
        // large repos and melts the host when several MCP clients do it at once.
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
            limit: Some(5),
            context: Some(2),
            type_filter: None,
            regex: Some(false),
            literal: None,
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
        )
        .expect("hash vector store (vectors.usearch) should open at 256 dims");
        assert_eq!(
            store.dimensions(),
            256,
            "MCP auto-index must build 256-dim hash vectors, not 384-dim neural inline"
        );

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

    fn tool_json_payload(response: &Value) -> Value {
        let content = response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|v| v.as_str())
            .expect("tool response content text");
        serde_json::from_str(content).expect("valid JSON payload")
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
    }
}
