use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::daemon::DaemonState;
use crate::protocol::{
    BUILD_VERSION, DaemonRequest, DaemonResponse, SearchHit, group_hits_by_file,
};
use crate::workspace::{Workspace, list_workspaces};

const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_CONCURRENT_HTTP_CONNECTIONS: usize = 128;
const HTTP_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 50;
const WEB_AUTH_COOKIE: &str = "ivygrep_web_token";
const SECURITY_HEADERS: &str = concat!(
    "Content-Security-Policy: default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'\r\n",
    "Cross-Origin-Opener-Policy: same-origin\r\n",
    "Cross-Origin-Resource-Policy: same-origin\r\n",
    "Permissions-Policy: camera=(), geolocation=(), microphone=(), payment=(), usb=()\r\n",
    "Referrer-Policy: no-referrer\r\n",
    "X-Content-Type-Options: nosniff\r\n",
    "X-Frame-Options: DENY\r\n",
);

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
    headers: HashMap<String, String>,
}

enum TimedHttpRequest {
    Received(Option<HttpRequest>),
    TimedOut,
}

fn web_auth_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN
        .get_or_init(|| {
            format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            )
        })
        .as_str()
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
    if !local_addr.ip().is_loopback() {
        params.push(format!("token={}", percent_encode(web_auth_token())));
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
    let auth_token = (!listener.local_addr()?.ip().is_loopback()).then_some(web_auth_token());
    let connections = Arc::new(Semaphore::new(MAX_CONCURRENT_HTTP_CONNECTIONS));
    loop {
        let permit = connections.clone().acquire_owned().await?;
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(err) = handle_connection(stream, state, config, auth_token).await {
                tracing::warn!("web request failed: {err:#}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: DaemonState,
    config: WebConfig,
    auth_token: Option<&'static str>,
) -> Result<()> {
    let request =
        match read_http_request_with_timeout(&mut stream, HTTP_HEADER_READ_TIMEOUT).await? {
            TimedHttpRequest::Received(Some(request)) => request,
            TimedHttpRequest::Received(None) => return Ok(()),
            TimedHttpRequest::TimedOut => {
                return write_json(
                    &mut stream,
                    "408 Request Timeout",
                    &json!({"error": "HTTP request headers timed out"}),
                )
                .await;
            }
        };
    let (path, params) = parse_target(&request.target)?;
    if !valid_host(&request, auth_token.is_some()) {
        return write_json(
            &mut stream,
            "403 Forbidden",
            &json!({"error": "invalid Host header"}),
        )
        .await;
    }

    if path == "/" || path == "/index.html" {
        if request.method != "GET" {
            return method_not_allowed(&mut stream, "GET").await;
        }
        if let Some(expected) = auth_token {
            if let Some(presented) = param(&params, "token") {
                if !tokens_match(presented, expected) {
                    return unauthorized(&mut stream).await;
                }
                return establish_web_session(&mut stream, &path, &params, expected).await;
            }
            if !request_has_auth(&request, expected) {
                return unauthorized(&mut stream).await;
            }
        }
        return write_html(&mut stream, &render_app_html(&config)).await;
    }
    if let Some(asset) = embedded_asset(&path) {
        if request.method != "GET" {
            return method_not_allowed(&mut stream, "GET").await;
        }
        return write_response(&mut stream, "200 OK", asset.content_type, asset.bytes).await;
    }

    if path.starts_with("/api/") {
        if !valid_api_origin(&request) {
            return write_json(
                &mut stream,
                "403 Forbidden",
                &json!({"error": "cross-origin API request denied"}),
            )
            .await;
        }
        if let Some(expected) = auth_token
            && !request_has_auth(&request, expected)
        {
            return unauthorized(&mut stream).await;
        }
    }

    match path.as_str() {
        "/api/status" => {
            if request.method != "GET" {
                return method_not_allowed(&mut stream, "GET").await;
            }
            let response = crate::daemon::handle_web_request(state, DaemonRequest::Status).await;
            write_json(&mut stream, "200 OK", &serde_json::to_value(response)?).await
        }
        "/api/search" => {
            if request.method != "GET" {
                return method_not_allowed(&mut stream, "GET").await;
            }
            let value = run_search(state, &params).await;
            write_json(&mut stream, "200 OK", &value).await
        }
        "/api/search/stream" => {
            if request.method != "GET" {
                return method_not_allowed(&mut stream, "GET").await;
            }
            write_search_stream(&mut stream, state, &params).await
        }
        "/api/file" => {
            if request.method != "GET" {
                return method_not_allowed(&mut stream, "GET").await;
            }
            match read_tracked_file(&params) {
                Ok(value) => write_json(&mut stream, "200 OK", &value).await,
                Err(err) => {
                    write_json(
                        &mut stream,
                        "400 Bad Request",
                        &json!({"error": err.to_string()}),
                    )
                    .await
                }
            }
        }
        "/api/open" => {
            if request.method != "POST" {
                return method_not_allowed(&mut stream, "POST").await;
            }
            match open_tracked_file(&params) {
                Ok(value) => write_json(&mut stream, "200 OK", &value).await,
                Err(err) => {
                    write_json(
                        &mut stream,
                        "400 Bad Request",
                        &json!({"error": err.to_string()}),
                    )
                    .await
                }
            }
        }
        "/api/tree" => {
            if request.method != "GET" {
                return method_not_allowed(&mut stream, "GET").await;
            }
            match read_tracked_tree(&params) {
                Ok(value) => write_json(&mut stream, "200 OK", &value).await,
                Err(err) => {
                    write_json(
                        &mut stream,
                        "400 Bad Request",
                        &json!({"error": err.to_string()}),
                    )
                    .await
                }
            }
        }
        _ => write_json(&mut stream, "404 Not Found", &json!({"error": "not found"})).await,
    }
}

async fn read_http_request_with_timeout(
    stream: &mut TcpStream,
    timeout: Duration,
) -> Result<TimedHttpRequest> {
    match tokio::time::timeout(timeout, read_http_request(stream)).await {
        Ok(request) => Ok(TimedHttpRequest::Received(request?)),
        Err(_) => Ok(TimedHttpRequest::TimedOut),
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
        if buf.len() > MAX_HTTP_HEADER_BYTES {
            bail!("HTTP headers exceed {MAX_HTTP_HEADER_BYTES} bytes");
        }
        if buf.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buf
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or(buf.len());
    let text =
        std::str::from_utf8(&buf[..header_end]).context("HTTP request headers are not UTF-8")?;
    let mut lines = text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing HTTP request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || target.is_empty() {
        bail!("malformed HTTP request line");
    }
    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| anyhow!("malformed HTTP header"))?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            bail!("malformed HTTP header name");
        }
        if matches!(name.as_str(), "host" | "origin" | "authorization")
            && headers.contains_key(&name)
        {
            bail!("duplicate {name} header");
        }
        let value = value.trim();
        headers
            .entry(name)
            .and_modify(|current: &mut String| {
                current.push_str("; ");
                current.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }
    Ok(Some(HttpRequest {
        method,
        target,
        headers,
    }))
}

fn valid_host(request: &HttpRequest, auth_required: bool) -> bool {
    let Some(authority) = request.headers.get("host") else {
        return false;
    };
    let Some(host) = authority_host(authority) else {
        return false;
    };
    if !auth_required {
        return host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    }
    host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().is_ok()
}

fn authority_host(authority: &str) -> Option<&str> {
    if authority.is_empty()
        || authority.contains(['/', '\\', '@'])
        || authority.chars().any(char::is_whitespace)
    {
        return None;
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        let suffix = &rest[end + 1..];
        if !suffix.is_empty() && (!suffix.starts_with(':') || suffix[1..].parse::<u16>().is_err()) {
            return None;
        }
        return (!host.is_empty()).then_some(host);
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.parse::<u16>().is_ok() => Some(host),
        Some(_) => None,
        None => Some(authority),
    }
}

fn valid_api_origin(request: &HttpRequest) -> bool {
    if request
        .headers
        .get("sec-fetch-site")
        .is_some_and(|value| value.eq_ignore_ascii_case("cross-site"))
    {
        return false;
    }
    let Some(origin) = request.headers.get("origin") else {
        return true;
    };
    let Some(authority) = request.headers.get("host") else {
        return false;
    };
    let Some(origin_authority) = origin
        .strip_prefix("http://")
        .and_then(|value| value.split('/').next())
    else {
        return false;
    };
    origin_authority.eq_ignore_ascii_case(authority)
}

fn request_has_auth(request: &HttpRequest, expected: &str) -> bool {
    let bearer_matches = request
        .headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| tokens_match(value.trim(), expected));
    bearer_matches
        || request
            .headers
            .get("cookie")
            .and_then(|cookies| cookie_value(cookies, WEB_AUTH_COOKIE))
            .is_some_and(|value| tokens_match(value, expected))
}

fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies.split(';').find_map(|cookie| {
        let (cookie_name, value) = cookie.trim().split_once('=')?;
        (cookie_name == name).then_some(value)
    })
}

fn tokens_match(presented: &str, expected: &str) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    presented
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
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

async fn establish_web_session(
    stream: &mut TcpStream,
    path: &str,
    params: &HashMap<String, Vec<String>>,
    token: &str,
) -> Result<()> {
    let location = target_without_param(path, params, "token");
    let cookie = format!("{WEB_AUTH_COOKIE}={token}; HttpOnly; Path=/; SameSite=Strict");
    write_response_with_headers(
        stream,
        "303 See Other",
        "text/plain; charset=utf-8",
        b"",
        &[
            ("Cache-Control", "no-store"),
            ("Location", location.as_str()),
            ("Set-Cookie", cookie.as_str()),
        ],
    )
    .await
}

fn target_without_param(
    path: &str,
    params: &HashMap<String, Vec<String>>,
    excluded: &str,
) -> String {
    let mut pairs = params
        .iter()
        .filter(|(key, _)| key.as_str() != excluded)
        .flat_map(|(key, values)| values.iter().map(move |value| (key, value)))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(right.0).then_with(|| left.1.cmp(right.1)));
    if pairs.is_empty() {
        return path.to_string();
    }
    format!(
        "{path}?{}",
        pairs
            .into_iter()
            .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
            .collect::<Vec<_>>()
            .join("&")
    )
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
            context,
            type_filter,
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
            disable_memory_expansion: false,
        },
    })
}

async fn execute_search(
    state: DaemonState,
    request: DaemonRequest,
) -> Result<(Vec<SearchHit>, Vec<String>), String> {
    match crate::daemon::handle_web_request(state, request).await {
        DaemonResponse::SearchResults { hits, warnings } => Ok((hits, warnings)),
        DaemonResponse::Error { message } => Err(message),
        other => Err(format!("unexpected daemon response: {other:?}")),
    }
}

fn search_value(hits: &[SearchHit], warnings: &[String], started: Instant) -> Value {
    let groups = group_hits_by_file(hits, None);
    json!({
        "hits": hits,
        "groups": groups,
        "warnings": warnings,
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0
    })
}

async fn run_search(state: DaemonState, params: &HashMap<String, Vec<String>>) -> Value {
    let started = Instant::now();
    if param(params, "mode") == Some("context") {
        let params = params.clone();
        let permit = state.acquire_cpu_permit().await;
        return match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            build_context_pack(&state, &params)
        })
        .await
        {
            Ok(Ok(bundle)) => json!({
                "context_pack": bundle,
                "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
            }),
            Ok(Err(error)) => json!({"error": error.to_string()}),
            Err(error) => json!({"error": format!("context task failed: {error}")}),
        };
    }
    let request = match build_search_request(params) {
        Ok(request) => request,
        Err(err) => return json!({"error": err.to_string(), "hits": [], "groups": []}),
    };
    match execute_search(state, request).await {
        Ok((hits, warnings)) => search_value(&hits, &warnings, started),
        Err(message) => json!({"error": message, "hits": [], "groups": []}),
    }
}

fn build_context_pack(
    state: &DaemonState,
    params: &HashMap<String, Vec<String>>,
) -> Result<crate::context::ContextBundle> {
    let selected = param(params, "workspace")
        .map(str::trim)
        .filter(|workspace| !workspace.is_empty() && *workspace != "__all__")
        .context("select one workspace for context mode")?;
    let selected = Workspace::resolve(Path::new(selected))?;
    let selected_root = selected.root;
    if !tracked_roots()?.contains(&selected_root) {
        bail!("workspace is not tracked");
    }
    let scoped_path = param(params, "scope")
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map_or_else(|| selected_root.clone(), |scope| selected_root.join(scope));
    let (workspace, scope_filter) = crate::workspace::resolve_workspace_and_scope(&scoped_path)?;
    if workspace.root != selected_root {
        bail!("scope is outside selected workspace");
    }
    let query = param(params, "q").unwrap_or_default().trim();
    if query.is_empty() {
        bail!("missing q");
    }
    let budget = match param(params, "budget_tokens") {
        Some(value) => value
            .parse::<usize>()
            .context("budget_tokens must be an integer")?,
        None => 8_000,
    };
    if !(256..=131_072).contains(&budget) {
        bail!("budget_tokens must be between 256 and 131072");
    }
    let skip_gitignore = parse_bool_param(params, "skip_gitignore");
    let model = state.prepare_context_model(&workspace, skip_gitignore)?;
    crate::context::build_context_bundle_with_options(
        &workspace,
        query,
        Some(model.as_ref()),
        &crate::search::SearchOptions {
            limit: None,
            context: 12,
            type_filter: param(params, "type")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            include_globs: csv_param(params, "include"),
            exclude_globs: csv_param(params, "exclude"),
            scope_filter,
            skip_gitignore,
            force_neural: false,
            progress_tx: None,
            cancel_token: None,
        },
        budget,
        &crate::context::ContextBuildOptions {
            since: param(params, "since")
                .map(str::trim)
                .filter(|since| !since.is_empty()),
        },
    )
}

async fn write_search_stream(
    stream: &mut TcpStream,
    state: DaemonState,
    params: &HashMap<String, Vec<String>>,
) -> Result<()> {
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\n{SECURITY_HEADERS}\r\n"
            )
            .as_bytes(),
        )
        .await?;

    if searches_all_workspaces(params) {
        if param(params, "mode") == Some("context") {
            write_sse(
                stream,
                "results",
                &json!({"error": "select one workspace for context mode"}),
            )
            .await?;
            return write_sse(stream, "done", &json!({"ok": false})).await;
        }
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
        write_sse(stream, "results", &search_value(&[], &[], started)).await?;
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
            let (hits, warnings) = execute_search(state, request)
                .await
                .map_err(|err| (root.clone(), err))?;
            Ok::<_, (PathBuf, String)>((root, hits, warnings))
        });
    }

    let mut finished = 0usize;
    let mut all_hits = Vec::<SearchHit>::new();
    let mut errors = Vec::<String>::new();
    let mut warnings = Vec::<String>::new();
    while let Some(result) = tasks.join_next().await {
        finished += 1;
        match result {
            Ok(Ok((root, mut hits, workspace_warnings))) => {
                for hit in &mut hits {
                    hit.file_path = root.join(&hit.file_path);
                }
                all_hits.append(&mut hits);
                warnings.extend(workspace_warnings);
            }
            Ok(Err((root, err))) => {
                let message = format!("{}: {err}", root.display());
                errors.push(message.clone());
                warnings.push(message);
            }
            Err(err) => {
                let message = err.to_string();
                errors.push(message.clone());
                warnings.push(message);
            }
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
        let mut value = search_value(&all_hits, &warnings, started);
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
        let root = workspace.root;
        if !roots.iter().any(|tracked| tracked == &root) {
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
            .find(|root| canonical.starts_with(root))
            .cloned()
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
    let mut file = crate::workspace_file::open(&root, &path)?;
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

fn open_tracked_file(params: &HashMap<String, Vec<String>>) -> Result<Value> {
    let (_, path) = resolve_tracked_path(params, false)?;
    let line = parse_usize_param(params, "line", 1).max(1);
    let column = param(params, "column")
        .and_then(|value| value.parse::<usize>().ok())
        .map(|column| column.max(1));
    let launch = crate::launcher::web_editor_launch(&path, line, column)?;
    let program = launch.program.to_string_lossy().into_owned();
    launch
        .spawn_detached()
        .with_context(|| format!("failed to launch {program}"))?;
    Ok(json!({
        "ok": true,
        "path": path,
        "line": line,
        "program": program
    }))
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
    write_response_with_headers(
        stream,
        "200 OK",
        "text/html; charset=utf-8",
        html.as_bytes(),
        &[("Cache-Control", "no-store")],
    )
    .await
}

async fn write_json(stream: &mut TcpStream, status: &str, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write_response_with_headers(
        stream,
        status,
        "application/json; charset=utf-8",
        &body,
        &[("Cache-Control", "no-store")],
    )
    .await
}

async fn unauthorized(stream: &mut TcpStream) -> Result<()> {
    let body = serde_json::to_vec(&json!({
        "error": "authentication required; open the URL printed by `ig --web`"
    }))?;
    write_response_with_headers(
        stream,
        "401 Unauthorized",
        "application/json; charset=utf-8",
        &body,
        &[
            ("Cache-Control", "no-store"),
            ("WWW-Authenticate", "Bearer realm=\"ivygrep web\""),
        ],
    )
    .await
}

async fn method_not_allowed(stream: &mut TcpStream, allow: &str) -> Result<()> {
    let body = serde_json::to_vec(&json!({"error": "method not allowed"}))?;
    write_response_with_headers(
        stream,
        "405 Method Not Allowed",
        "application/json; charset=utf-8",
        &body,
        &[("Allow", allow)],
    )
    .await
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    write_response_with_headers(stream, status, content_type, body, &[]).await
}

async fn write_response_with_headers(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, &str)],
) -> Result<()> {
    let mut extra = String::new();
    for (name, value) in extra_headers {
        extra.push_str(name);
        extra.push_str(": ");
        extra.push_str(value);
        extra.push_str("\r\n");
    }
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{SECURITY_HEADERS}{extra}\r\n",
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
    WEB_INDEX_HTML.replace(
        "__IVYGREP_BOOT__",
        &escape_html_attribute(&serde_json::to_string(&boot).unwrap()),
    )
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
    fn search_payload_reports_partial_workspace_warnings() {
        let value = search_value(
            &[],
            &["search failed for /tmp/stale".to_string()],
            Instant::now(),
        );
        assert_eq!(value["warnings"], json!(["search failed for /tmp/stale"]));
    }

    #[tokio::test]
    async fn incomplete_http_headers_hit_read_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n")
            .await
            .unwrap();

        let result = read_http_request_with_timeout(&mut server, Duration::from_millis(20))
            .await
            .unwrap();
        assert!(matches!(result, TimedHttpRequest::TimedOut));
    }

    #[test]
    fn non_loopback_url_contains_process_token_but_loopback_url_does_not() {
        let config = WebConfig {
            host: "127.0.0.1".to_string(),
            port: 4747,
            initial_query: None,
            initial_path: None,
        };
        let loopback = initial_url(&config, "127.0.0.1:4747".parse().unwrap());
        assert!(!loopback.contains("token="));

        let exposed = initial_url(&config, "0.0.0.0:4747".parse().unwrap());
        assert!(exposed.contains(&format!("token={}", web_auth_token())));
    }

    #[test]
    fn auth_accepts_cookie_or_bearer_and_rejects_other_values() {
        let expected = "0123456789abcdef";
        let cookie_request = HttpRequest {
            method: "GET".to_string(),
            target: "/api/status".to_string(),
            headers: HashMap::from([(
                "cookie".to_string(),
                format!("other=1; {WEB_AUTH_COOKIE}={expected}"),
            )]),
        };
        assert!(request_has_auth(&cookie_request, expected));

        let bearer_request = HttpRequest {
            method: "GET".to_string(),
            target: "/api/status".to_string(),
            headers: HashMap::from([("authorization".to_string(), format!("Bearer {expected}"))]),
        };
        assert!(request_has_auth(&bearer_request, expected));
        assert!(!request_has_auth(&bearer_request, "different-token"));
    }

    #[test]
    fn session_redirect_removes_only_token() {
        let (_, params) = parse_target("/?q=auth+flow&token=secret&workspace=/tmp/repo").unwrap();
        let target = target_without_param("/", &params, "token");
        assert!(!target.contains("token="));
        assert!(target.contains("q=auth%20flow"));
        assert!(target.contains("workspace=/tmp/repo"));
    }

    #[test]
    fn boot_config_is_html_attribute_escaped() {
        let config = WebConfig {
            host: "127.0.0.1".to_string(),
            port: 4747,
            initial_query: Some("\"><script>bad()</script>".to_string()),
            initial_path: None,
        };
        let html = render_app_html(&config);
        assert!(!html.contains("<script>bad()</script>"));
        assert!(html.contains("&quot;"));
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
    fn regex_search_request_forwards_type_and_context() {
        let (_, params) =
            parse_target("/api/search?q=marker&mode=regex&type=markdown&context=7").unwrap();
        let request = build_search_request(&params).unwrap();
        let DaemonRequest::RegexSearch {
            context,
            type_filter,
            ..
        } = request
        else {
            panic!("expected regex search request");
        };
        assert_eq!(context, 7);
        assert_eq!(type_filter.as_deref(), Some("markdown"));
    }

    #[test]
    fn raw_web_type_alias_reaches_shared_search_options_normalization() {
        let (_, params) = parse_target("/api/search?q=marker&type=rs").unwrap();
        let request = build_search_request(&params).unwrap();
        let DaemonRequest::Search { type_filter, .. } = request else {
            panic!("expected hybrid search request");
        };
        let options = crate::search::SearchOptions {
            type_filter,
            ..Default::default()
        };
        assert_eq!(options.canonical_type_filter().as_deref(), Some("rust"));
    }
}
