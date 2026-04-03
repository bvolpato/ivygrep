use std::env;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config;
use crate::embedding::create_model;
use crate::indexer::{index_workspace, workspace_is_indexed};
use crate::path_glob::parse_glob_csv;
use crate::protocol::group_hits_by_file;
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::resolve_workspace_and_scope;

const JSONRPC_VERSION: &str = "2.0";
const TOOL_IG_SEARCH: &str = "ig_search";

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
    include: Option<String>,
    exclude: Option<String>,
    first_line_only: Option<bool>,
    file_name_only: Option<bool>,
    verbose: Option<bool>,
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

        if let Some(response) = handle_request(request) {
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
            "instructions": "Use ig_search(query, path) to run local semantic code search. If path is a subdirectory or file, results are restricted to that scope."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": [search_tool_schema()]})),
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
                "type": {"type": "string", "description": "Language filter (rust, python, typescript, ...)."},
                "regex": {"type": "boolean", "description": "Use regex mode instead of hybrid semantic search."},
                "include": {"type": "string", "description": "Comma-separated include globs, e.g. \"*.md,src/**/*.rs\"."},
                "exclude": {"type": "string", "description": "Comma-separated exclude globs, e.g. \"target/**,*.lock\"."},
                "first_line_only": {"type": "boolean", "description": "Return only the first non-empty preview line for each hit."},
                "file_name_only": {"type": "boolean", "description": "Return only file paths (no hit details)."},
                "verbose": {"type": "boolean", "description": "Include reason pointers in JSON output."}
            },
            "required": ["query"]
        }
    })
}

fn run_tool_call(params: Value) -> Result<Value> {
    let call: ToolCallParams = serde_json::from_value(params)?;
    if call.name != TOOL_IG_SEARCH {
        bail!("unknown tool: {}", call.name);
    }

    let args: IvygrepSearchArgs = serde_json::from_value(call.arguments)?;
    execute_ivygrep_search(args)
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

    let (workspace, scope_filter) = resolve_workspace_and_scope(Path::new(&input_path))?;
    let model = create_model(false);

    if !workspace_is_indexed(&workspace) {
        let _summary = index_workspace(&workspace, model.as_ref())?;
    }

    let include_globs = parse_glob_csv(args.include.as_deref());
    let exclude_globs = parse_glob_csv(args.exclude.as_deref());

    let hits = if args.regex.unwrap_or(false) {
        regex_search(
            &workspace,
            query,
            args.limit,
            scope_filter.as_ref(),
            &include_globs,
            &exclude_globs,
        )?
    } else {
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
            },
        )?
    };

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
            "workspace_root": workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": if args.regex.unwrap_or(false) { "regex" } else { "hybrid" },
            "result_count": grouped.len(),
            "include": include_globs,
            "exclude": exclude_globs,
            "file_paths": grouped.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
        })
    } else {
        json!({
            "workspace_root": workspace.root,
            "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
            "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
            "query": query,
            "mode": if args.regex.unwrap_or(false) { "regex" } else { "hybrid" },
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

fn read_message<R: BufRead>(reader: &mut R, mode: &mut FramingMode) -> Result<Option<Vec<u8>>> {
    // Read first non-empty line (skip blank lines between messages).
    let first_line = loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
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
                let bytes = reader.read_line(&mut line)?;
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
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
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
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
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
            include: Some("*.md".to_string()),
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
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
            include: Some("*.md".to_string()),
            exclude: Some("match.md".to_string()),
            first_line_only: Some(false),
            file_name_only: Some(true),
            verbose: Some(false),
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
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "ig_search");
        let schema = &tools[0]["inputSchema"];
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["regex"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("query")));
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
            include: None,
            exclude: None,
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
        })
        .unwrap();

        let result = tool_json_payload(&response);
        assert_eq!(result["mode"], "regex");
        let count = result["result_count"].as_u64().unwrap();
        assert!(count > 0, "regex search should find results");
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
}
