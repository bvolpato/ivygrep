use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use crate::daemon::DaemonState;
use crate::protocol::{
    BUILD_VERSION, DaemonRequest, DaemonResponse, SearchHit, group_hits_by_file,
};
use crate::workspace::{Workspace, list_workspaces};

const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 50;

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

#[derive(Debug, Clone)]
pub(crate) struct WebConfig {
    pub host: String,
    pub port: u16,
    pub initial_query: Option<String>,
    pub initial_path: Option<PathBuf>,
}

struct HttpRequest {
    method: String,
    target: String,
}

pub(crate) fn bind_addr(host: &str, port: u16) -> Result<SocketAddr> {
    (host, port)
        .to_socket_addrs()
        .with_context(|| format!("invalid --host value {host:?}"))?
        .next()
        .ok_or_else(|| anyhow!("--host value {host:?} did not resolve to an address"))
}

pub(crate) fn initial_url(config: &WebConfig, local_addr: SocketAddr) -> String {
    let host = match local_addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    let mut params = Vec::new();
    if let Some(query) = config
        .initial_query
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        params.push(format!("q={}", percent_encode(query)));
    }
    if let Some(path) = config.initial_path.as_ref() {
        params.push(format!(
            "workspace={}",
            percent_encode(&path.display().to_string())
        ));
    }
    let mut url = format!("http://{host}:{}/", local_addr.port());
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    url
}

pub(crate) async fn serve(
    listener: TcpListener,
    state: DaemonState,
    config: WebConfig,
) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        let config = config.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state, config).await {
                tracing::warn!("web request failed: {err:#}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: DaemonState,
    config: WebConfig,
) -> Result<()> {
    let Some(request) = read_http_request(&mut stream).await? else {
        return Ok(());
    };
    if request.method != "GET" {
        return write_json(
            &mut stream,
            "405 Method Not Allowed",
            &json!({"error": "method not allowed"}),
        )
        .await;
    }

    let (path, params) = parse_target(&request.target)?;
    if path == "/" || path == "/index.html" {
        return write_html(&mut stream, &render_app_html(&config)).await;
    }
    if let Some(asset) = embedded_asset(&path) {
        return write_response(&mut stream, "200 OK", asset.content_type, asset.bytes).await;
    }

    match path.as_str() {
        "/api/status" => {
            let response = crate::daemon::handle_web_request(state, DaemonRequest::Status).await;
            write_json(&mut stream, "200 OK", &serde_json::to_value(response)?).await
        }
        "/api/search" => {
            let value = run_search(state, &params).await;
            write_json(&mut stream, "200 OK", &value).await
        }
        "/api/search/stream" => write_search_stream(&mut stream, state, &params).await,
        "/api/file" => match read_tracked_file(&params) {
            Ok(value) => write_json(&mut stream, "200 OK", &value).await,
            Err(err) => {
                write_json(
                    &mut stream,
                    "400 Bad Request",
                    &json!({"error": err.to_string()}),
                )
                .await
            }
        },
        "/api/open" => match open_tracked_file(&params) {
            Ok(value) => write_json(&mut stream, "200 OK", &value).await,
            Err(err) => {
                write_json(
                    &mut stream,
                    "400 Bad Request",
                    &json!({"error": err.to_string()}),
                )
                .await
            }
        },
        "/api/tree" => match read_tracked_tree(&params) {
            Ok(value) => write_json(&mut stream, "200 OK", &value).await,
            Err(err) => {
                write_json(
                    &mut stream,
                    "400 Bad Request",
                    &json!({"error": err.to_string()}),
                )
                .await
            }
        },
        _ => write_json(&mut stream, "404 Not Found", &json!({"error": "not found"})).await,
    }
}

async fn read_http_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buf.len() > MAX_HTTP_HEADER_BYTES {
            bail!("HTTP headers exceed {MAX_HTTP_HEADER_BYTES} bytes");
        }
    }

    let text = std::str::from_utf8(&buf).context("HTTP request headers are not UTF-8")?;
    let request_line = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("missing HTTP request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || target.is_empty() {
        bail!("malformed HTTP request line");
    }
    Ok(Some(HttpRequest { method, target }))
}

fn parse_target(target: &str) -> Result<(String, HashMap<String, Vec<String>>)> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut params = HashMap::<String, Vec<String>>::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        params
            .entry(percent_decode(key)?)
            .or_default()
            .push(percent_decode(value)?);
    }
    Ok((path.to_string(), params))
}

fn param<'a>(params: &'a HashMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(|values| values.first())
        .map(String::as_str)
}

fn parse_usize_param(params: &HashMap<String, Vec<String>>, key: &str, default: usize) -> usize {
    param(params, key)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn parse_bool_param(params: &HashMap<String, Vec<String>>, key: &str) -> bool {
    matches!(param(params, key), Some("1" | "true" | "yes"))
}

fn csv_param(params: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
    params
        .get(key)
        .into_iter()
        .flatten()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn build_search_request(params: &HashMap<String, Vec<String>>) -> Result<DaemonRequest> {
    let workspace = param(params, "workspace").unwrap_or_default().trim();
    let path = (!workspace.is_empty() && workspace != "__all__").then(|| PathBuf::from(workspace));
    build_search_request_for_path(params, path)
}

fn build_search_request_for_path(
    params: &HashMap<String, Vec<String>>,
    path: Option<PathBuf>,
) -> Result<DaemonRequest> {
    let query = param(params, "q").unwrap_or_default().trim().to_string();
    if query.is_empty() {
        bail!("missing q");
    }
    let limit = Some(parse_usize_param(params, "limit", DEFAULT_SEARCH_LIMIT).clamp(1, 500));
    let context = parse_usize_param(params, "context", 2).min(20);
    let type_filter = param(params, "type")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let include_globs = csv_param(params, "include");
    let exclude_globs = csv_param(params, "exclude");
    let scope_path = param(params, "scope")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let scope_is_file = parse_bool_param(params, "scope_is_file");
    let skip_gitignore = parse_bool_param(params, "skip_gitignore");

    Ok(match param(params, "mode").unwrap_or("hybrid") {
        "literal" => DaemonRequest::LiteralSearch {
            path,
            query,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        },
        "regex" => DaemonRequest::RegexSearch {
            path,
            pattern: query,
            limit,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        },
        _ => DaemonRequest::Search {
            path,
            query,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
            force_neural: false,
        },
    })
}

async fn execute_search(
    state: DaemonState,
    request: DaemonRequest,
) -> Result<Vec<SearchHit>, String> {
    match crate::daemon::handle_web_request(state, request).await {
        DaemonResponse::SearchResults { hits } => Ok(hits),
        DaemonResponse::Error { message } => Err(message),
        other => Err(format!("unexpected daemon response: {other:?}")),
    }
}

fn search_value(hits: &[SearchHit], started: Instant) -> Value {
    let groups = group_hits_by_file(hits, None);
    json!({
        "hits": hits,
        "groups": groups,
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0
    })
}

async fn run_search(state: DaemonState, params: &HashMap<String, Vec<String>>) -> Value {
    let started = Instant::now();
    let request = match build_search_request(params) {
        Ok(request) => request,
        Err(err) => return json!({"error": err.to_string(), "hits": [], "groups": []}),
    };
    match execute_search(state, request).await {
        Ok(hits) => search_value(&hits, started),
        Err(message) => json!({"error": message, "hits": [], "groups": []}),
    }
}

async fn write_search_stream(
    stream: &mut TcpStream,
    state: DaemonState,
    params: &HashMap<String, Vec<String>>,
) -> Result<()> {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        )
        .await?;

    if searches_all_workspaces(params) {
        return write_all_workspace_search_stream(stream, state, params).await;
    }

    write_sse(stream, "status", &json!({"stage": "searching"})).await?;
    let value = run_search(state, params).await;
    write_sse(stream, "results", &value).await?;
    write_sse(stream, "done", &json!({"ok": true})).await
}

fn searches_all_workspaces(params: &HashMap<String, Vec<String>>) -> bool {
    matches!(
        param(params, "workspace"),
        None | Some("") | Some("__all__")
    )
}

async fn write_all_workspace_search_stream(
    stream: &mut TcpStream,
    state: DaemonState,
    params: &HashMap<String, Vec<String>>,
) -> Result<()> {
    if let Err(err) = build_search_request(params) {
        write_sse(
            stream,
            "results",
            &json!({"error": err.to_string(), "hits": [], "groups": []}),
        )
        .await?;
        return write_sse(stream, "done", &json!({"ok": false})).await;
    }

    let started = Instant::now();
    let limit = parse_usize_param(params, "limit", DEFAULT_SEARCH_LIMIT).clamp(1, 500);
    let roots = tracked_roots()?;
    let total = roots.len();
    write_sse(
        stream,
        "status",
        &json!({"stage": "searching", "finished": 0, "total": total}),
    )
    .await?;
    if total == 0 {
        write_sse(stream, "results", &search_value(&[], started)).await?;
        return write_sse(
            stream,
            "done",
            &json!({"ok": true, "finished": 0, "total": 0}),
        )
        .await;
    }

    let mut tasks = JoinSet::new();
    for root in roots {
        let state = state.clone();
        let params = params.clone();
        tasks.spawn(async move {
            let request = build_search_request_for_path(&params, Some(root.clone()))
                .map_err(|err| (root.clone(), err.to_string()))?;
            let hits = execute_search(state, request)
                .await
                .map_err(|err| (root.clone(), err))?;
            Ok::<_, (PathBuf, String)>((root, hits))
        });
    }

    let mut finished = 0usize;
    let mut all_hits = Vec::<SearchHit>::new();
    let mut errors = Vec::<String>::new();
    while let Some(result) = tasks.join_next().await {
        finished += 1;
        match result {
            Ok(Ok((root, mut hits))) => {
                for hit in &mut hits {
                    hit.file_path = root.join(&hit.file_path);
                }
                all_hits.append(&mut hits);
            }
            Ok(Err((root, err))) => errors.push(format!("{}: {err}", root.display())),
            Err(err) => errors.push(err.to_string()),
        }
        all_hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.file_path.cmp(&right.file_path))
                .then_with(|| left.start_line.cmp(&right.start_line))
        });
        if all_hits.len() > limit {
            all_hits.truncate(limit);
        }
        let mut value = search_value(&all_hits, started);
        if let Some(object) = value.as_object_mut() {
            object.insert("finished".to_string(), json!(finished));
            object.insert("total".to_string(), json!(total));
            if !errors.is_empty() {
                object.insert("errors".to_string(), json!(&errors));
            }
        }
        write_sse(stream, "results", &value).await?;
        write_sse(
            stream,
            "status",
            &json!({"stage": "searching", "finished": finished, "total": total}),
        )
        .await?;
    }

    write_sse(
        stream,
        "done",
        &json!({"ok": errors.is_empty(), "finished": finished, "total": total}),
    )
    .await
}

async fn write_sse(stream: &mut TcpStream, event: &str, value: &Value) -> Result<()> {
    stream
        .write_all(format!("event: {event}\n").as_bytes())
        .await?;
    stream
        .write_all(format!("data: {}\n\n", serde_json::to_string(value)?).as_bytes())
        .await?;
    stream.flush().await?;
    Ok(())
}

fn tracked_roots() -> Result<Vec<PathBuf>> {
    list_workspaces().map(|workspaces| workspaces.into_iter().map(|ws| ws.root).collect())
}

fn resolve_tracked_path(
    params: &HashMap<String, Vec<String>>,
    require_dir: bool,
) -> Result<(PathBuf, PathBuf)> {
    let roots = tracked_roots()?;
    let workspace_param = param(params, "workspace")
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "__all__");
    let path_param = param(params, "path")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(".");

    let root = if let Some(workspace) = workspace_param {
        let workspace = Workspace::resolve(&PathBuf::from(workspace))?;
        let root = workspace.root.canonicalize()?;
        if !roots
            .iter()
            .any(|tracked| tracked.canonicalize().is_ok_and(|tracked| tracked == root))
        {
            bail!("workspace is not tracked");
        }
        root
    } else {
        let candidate = PathBuf::from(path_param);
        if !candidate.is_absolute() {
            bail!("workspace is required for relative paths");
        }
        let canonical = candidate.canonicalize()?;
        roots
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .find(|root| canonical.starts_with(root))
            .ok_or_else(|| anyhow!("path is outside tracked workspaces"))?
    };

    let raw_path = PathBuf::from(path_param);
    let candidate = if raw_path.is_absolute() {
        raw_path
    } else {
        root.join(raw_path)
    };
    let canonical = candidate.canonicalize()?;
    if !canonical.starts_with(&root) {
        bail!("path is outside workspace");
    }
    if require_dir && !canonical.is_dir() {
        bail!("path is not a directory");
    }
    if !require_dir && !canonical.is_file() {
        bail!("path is not a file");
    }
    Ok((root, canonical))
}

fn read_tracked_file(params: &HashMap<String, Vec<String>>) -> Result<Value> {
    let (root, path) = resolve_tracked_path(params, false)?;
    let mut file = std::fs::File::open(&path)?;
    let mut limited = (&mut file).take(MAX_FILE_BYTES + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_FILE_BYTES as usize;
    if truncated {
        bytes.truncate(MAX_FILE_BYTES as usize);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let line_count = text.lines().count();
    let rel = path.strip_prefix(&root).unwrap_or(&path);
    Ok(json!({
        "workspace": root,
        "path": rel,
        "absolute_path": path,
        "text": text,
        "truncated": truncated,
        "line_count": line_count
    }))
}

#[derive(Debug, PartialEq, Eq)]
struct EditorLaunch {
    program: String,
    args: Vec<String>,
}

fn open_tracked_file(params: &HashMap<String, Vec<String>>) -> Result<Value> {
    let (_, path) = resolve_tracked_path(params, false)?;
    let line = parse_usize_param(params, "line", 1).max(1);
    let launch = editor_launch_for_path(&path, line);
    std::process::Command::new(&launch.program)
        .args(&launch.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {}", launch.program))?;
    Ok(json!({
        "ok": true,
        "path": path,
        "line": line,
        "program": launch.program
    }))
}

fn editor_launch_for_path(path: &Path, line: usize) -> EditorLaunch {
    if let Some(launch) = configured_editor_launch(path, line) {
        return launch;
    }
    for candidate in [
        "code",
        "code-insiders",
        "cursor",
        "codium",
        "windsurf",
        "zed",
        "subl",
    ] {
        if let Some(program) = find_program(candidate) {
            return editor_launch_from_parts([program], path, line);
        }
    }
    platform_open_launch(path)
}

fn configured_editor_launch(path: &Path, line: usize) -> Option<EditorLaunch> {
    for name in ["IVYGREP_WEB_EDITOR", "IVYGREP_EDITOR", "EDITOR", "VISUAL"] {
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        let parts = split_editor_command(&value);
        let Some(program) = parts.first() else {
            continue;
        };
        if matches!(name, "EDITOR" | "VISUAL") && is_terminal_editor(program) {
            continue;
        }
        return Some(editor_launch_from_parts(parts, path, line));
    }
    None
}

fn split_editor_command(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn editor_launch_from_parts<I>(parts: I, path: &Path, line: usize) -> EditorLaunch
where
    I: IntoIterator<Item = String>,
{
    let mut parts = parts.into_iter();
    let program = parts
        .next()
        .unwrap_or_else(|| platform_open_program().to_string());
    let mut args = parts.collect::<Vec<_>>();
    let target = path.display().to_string();
    let basename = program_basename(&program);
    match basename.as_str() {
        "code" | "code-insiders" | "codium" | "cursor" | "windsurf" => {
            args.push("-g".to_string());
            args.push(format!("{target}:{}", line.max(1)));
        }
        "zed" | "subl" | "mate" => args.push(format!("{target}:{}", line.max(1))),
        "vim" | "nvim" | "vi" | "nano" | "emacs" => {
            args.push(format!("+{}", line.max(1)));
            args.push(target);
        }
        _ => args.push(target),
    }
    EditorLaunch { program, args }
}

fn program_basename(program: &str) -> String {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();
    name.trim_end_matches(".exe").to_string()
}

fn is_terminal_editor(program: &str) -> bool {
    matches!(
        program_basename(program).as_str(),
        "vim" | "nvim" | "vi" | "nano" | "emacs" | "emacsclient" | "hx" | "helix"
    )
}

fn find_program(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(candidate.display().to_string());
            }
        }
    }
    None
}

fn platform_open_launch(path: &Path) -> EditorLaunch {
    EditorLaunch {
        program: platform_open_program().to_string(),
        args: platform_open_args(path),
    }
}

#[cfg(target_os = "macos")]
fn platform_open_program() -> &'static str {
    "open"
}

#[cfg(target_os = "windows")]
fn platform_open_program() -> &'static str {
    "cmd"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_open_program() -> &'static str {
    "xdg-open"
}

#[cfg(target_os = "macos")]
fn platform_open_args(path: &Path) -> Vec<String> {
    vec![path.display().to_string()]
}

#[cfg(target_os = "windows")]
fn platform_open_args(path: &Path) -> Vec<String> {
    vec![
        "/C".to_string(),
        "start".to_string(),
        "".to_string(),
        path.display().to_string(),
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_open_args(path: &Path) -> Vec<String> {
    vec![path.display().to_string()]
}

fn read_tracked_tree(params: &HashMap<String, Vec<String>>) -> Result<Value> {
    let (root, path) = resolve_tracked_path(params, true)?;
    let mut entries = std::fs::read_dir(&path)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let file_type = entry.file_type().ok()?;
            let rel = path.strip_prefix(&root).ok()?.to_path_buf();
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(json!({
                "name": name,
                "path": rel,
                "is_dir": file_type.is_dir()
            }))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        let a_dir = a.get("is_dir").and_then(Value::as_bool).unwrap_or(false);
        let b_dir = b.get("is_dir").and_then(Value::as_bool).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| {
            a.get("name")
                .and_then(Value::as_str)
                .cmp(&b.get("name").and_then(Value::as_str))
        })
    });
    entries.truncate(500);
    let rel = path.strip_prefix(&root).unwrap_or(&path);
    Ok(json!({
        "workspace": root,
        "path": rel,
        "entries": entries
    }))
}

async fn write_html(stream: &mut TcpStream, html: &str) -> Result<()> {
    write_response(
        stream,
        "200 OK",
        "text/html; charset=utf-8",
        html.as_bytes(),
    )
    .await
}

async fn write_json(stream: &mut TcpStream, status: &str, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write_response(stream, status, "application/json; charset=utf-8", &body).await
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    Ok(())
}

fn render_app_html(config: &WebConfig) -> String {
    let boot = json!({
        "version": BUILD_VERSION,
        "query": config.initial_query,
        "workspace": config.initial_path.as_ref().map(|path| path.display().to_string()),
    });
    WEB_INDEX_HTML.replace("__IVYGREP_BOOT__", &serde_json::to_string(&boot).unwrap())
}

fn embedded_asset(path: &str) -> Option<&'static WebAsset> {
    WEB_ASSETS.iter().find(|asset| asset.path == path)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut iter = value.as_bytes().iter().copied();
    while let Some(byte) = iter.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let hi = iter
                    .next()
                    .ok_or_else(|| anyhow!("incomplete percent escape"))?;
                let lo = iter
                    .next()
                    .ok_or_else(|| anyhow!("incomplete percent escape"))?;
                let hex = [hi, lo];
                let value = std::str::from_utf8(&hex)
                    .ok()
                    .and_then(|text| u8::from_str_radix(text, 16).ok())
                    .ok_or_else(|| anyhow!("invalid percent escape"))?;
                bytes.push(value);
            }
            other => bytes.push(other),
        }
    }
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_roundtrip_handles_spaces_and_paths() {
        let value = "/tmp/project dir/src/main.rs";
        assert_eq!(percent_decode(&percent_encode(value)).unwrap(), value);
        assert_eq!(percent_decode("hello+world").unwrap(), "hello world");
    }

    #[test]
    fn bind_addr_accepts_localhost() {
        assert_eq!(bind_addr("localhost", 4747).unwrap().port(), 4747);
    }

    #[test]
    fn search_request_defaults_to_all_indices_hybrid() {
        let (_, params) = parse_target("/api/search?q=hello").unwrap();
        let request = build_search_request(&params).unwrap();
        let DaemonRequest::Search {
            path,
            query,
            limit,
            context,
            ..
        } = request
        else {
            panic!("expected hybrid search request");
        };
        assert!(path.is_none());
        assert_eq!(query, "hello");
        assert_eq!(limit, Some(DEFAULT_SEARCH_LIMIT));
        assert_eq!(context, 2);
    }

    #[test]
    fn search_request_respects_explicit_limit() {
        let (_, params) = parse_target("/api/search?q=hello&limit=10").unwrap();
        let request = build_search_request(&params).unwrap();
        let DaemonRequest::Search { limit, .. } = request else {
            panic!("expected hybrid search request");
        };
        assert_eq!(limit, Some(10));
    }

    #[test]
    fn search_request_supports_literal_workspace_mode() {
        let (_, params) =
            parse_target("/api/search?q=needle&mode=literal&workspace=/tmp/repo").unwrap();
        let request = build_search_request(&params).unwrap();
        let DaemonRequest::LiteralSearch { path, query, .. } = request else {
            panic!("expected literal search request");
        };
        assert_eq!(path, Some(PathBuf::from("/tmp/repo")));
        assert_eq!(query, "needle");
    }

    #[test]
    fn editor_launch_uses_line_aware_args_for_gui_editors() {
        let path = PathBuf::from("/tmp/repo/src/lib.rs");
        let launch = editor_launch_from_parts(["code".to_string()], &path, 42);
        assert_eq!(
            launch,
            EditorLaunch {
                program: "code".to_string(),
                args: vec!["-g".to_string(), "/tmp/repo/src/lib.rs:42".to_string()]
            }
        );
    }

    #[test]
    fn editor_launch_keeps_prefix_args() {
        let path = PathBuf::from("/tmp/repo/src/lib.rs");
        let launch = editor_launch_from_parts(
            ["cursor".to_string(), "--reuse-window".to_string()],
            &path,
            7,
        );
        assert_eq!(
            launch.args,
            vec![
                "--reuse-window".to_string(),
                "-g".to_string(),
                "/tmp/repo/src/lib.rs:7".to_string()
            ]
        );
    }

    #[test]
    fn terminal_editors_are_not_used_from_plain_editor_env() {
        assert!(is_terminal_editor("vim"));
        assert!(!is_terminal_editor("code"));
    }
}
