use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::EMBEDDING_DIMENSIONS;
use crate::config;
use crate::embedding::HashEmbeddingModel;
use crate::indexer::{index_workspace, workspace_is_indexed};
use crate::protocol::SearchHit;
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::resolve_workspace_and_scope;

const JSONRPC_VERSION: &str = "2.0";
const TOOL_IVYGREP_SEARCH: &str = "ivygrep_search";

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
    first_line_only: Option<bool>,
    file_name_only: Option<bool>,
    verbose: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct FileSearchResult {
    file_path: PathBuf,
    total_score: f32,
    hit_count: usize,
    hits: Vec<SearchHit>,
}

pub fn serve_stdio() -> Result<()> {
    config::ensure_app_dirs()?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    loop {
        let payload = match read_message(&mut reader)? {
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
                write_message(&mut writer, &response)?;
                continue;
            }
        };

        if let Some(response) = handle_request(request) {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    if request.id.is_none() {
        return None;
    }

    let id = request.id;

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
                "name": "ivygrep",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Use ivygrep_search(query, path) to run local semantic code search. If path is a subdirectory or file, results are restricted to that scope."
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
        "name": TOOL_IVYGREP_SEARCH,
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
                "first_line_only": {"type": "boolean", "description": "Show only first non-empty line of each hit preview."},
                "file_name_only": {"type": "boolean", "description": "Return only file paths."},
                "verbose": {"type": "boolean", "description": "Include reason pointers in output."}
            },
            "required": ["query"]
        }
    })
}

fn run_tool_call(params: Value) -> Result<Value> {
    let call: ToolCallParams = serde_json::from_value(params)?;
    if call.name != TOOL_IVYGREP_SEARCH {
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
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    if !workspace_is_indexed(&workspace) {
        let _summary = index_workspace(&workspace, &model)?;
    }

    let hits = if args.regex.unwrap_or(false) {
        regex_search(&workspace, query, args.limit, scope_filter.as_ref())?
    } else {
        hybrid_search(
            &workspace,
            query,
            &model,
            &SearchOptions {
                limit: args.limit,
                context: args.context.unwrap_or(2),
                type_filter: args.type_filter.clone(),
                scope_filter: scope_filter.clone(),
            },
        )?
    };

    let mut grouped = group_hits_by_file(&hits, args.limit);
    if !args.verbose.unwrap_or(false) {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.reason.clear();
            }
        }
    }

    let structured = json!({
        "workspace_root": workspace.root,
        "scope_path": scope_filter.as_ref().map(|scope| scope.rel_path.clone()),
        "scope_is_file": scope_filter.as_ref().is_some_and(|scope| scope.is_file),
        "query": query,
        "mode": if args.regex.unwrap_or(false) { "regex" } else { "hybrid" },
        "results": grouped,
    });

    let text = render_tool_text(
        &grouped,
        args.first_line_only.unwrap_or(false),
        args.file_name_only.unwrap_or(false),
        args.verbose.unwrap_or(false),
    );

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured,
        "isError": false
    }))
}

fn group_hits_by_file(hits: &[SearchHit], limit: Option<usize>) -> Vec<FileSearchResult> {
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

fn render_tool_text(
    grouped: &[FileSearchResult],
    first_line_only: bool,
    file_name_only: bool,
    verbose: bool,
) -> String {
    if grouped.is_empty() {
        return "No results.".to_string();
    }

    if file_name_only {
        return grouped
            .iter()
            .map(|file| file.file_path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut out = Vec::new();
    for file in grouped {
        out.push(format!(
            "{}  score={:.4}  matches={}",
            file.file_path.to_string_lossy(),
            file.total_score,
            file.hit_count
        ));

        for hit in &file.hits {
            let source = if hit.sources.is_empty() {
                String::new()
            } else {
                format!(" [{}]", hit.sources.join("+"))
            };

            out.push(format!(
                "  {}-{}{} score={:.4}",
                hit.start_line, hit.end_line, source, hit.score
            ));

            if verbose && !hit.reason.is_empty() {
                out.push(format!("    reason: {}", hit.reason.trim()));
            }

            let rendered_preview = if first_line_only {
                hit.preview
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string()
            } else {
                hit.preview.trim().to_string()
            };

            for line in rendered_preview.lines() {
                out.push(format!("    {line}"));
            }
        }

        out.push(String::new());
    }

    out.join("\n")
}

fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        let lower = trimmed.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            let len = value.trim().parse::<usize>()?;
            content_length = Some(len);
        }
    }

    let len = content_length.context("missing Content-Length header")?;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn write_message<W: Write>(writer: &mut W, response: &JsonRpcResponse) -> Result<()> {
    let payload = serde_json::to_vec(response)?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)?;
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
            first_line_only: Some(false),
            file_name_only: Some(false),
            verbose: Some(false),
        })
        .unwrap();

        let result = response.get("structuredContent").unwrap();
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
}
