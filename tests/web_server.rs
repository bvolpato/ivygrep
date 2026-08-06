use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write as _,
};

use serial_test::serial;
use tempfile::tempdir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ig")
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "commit.gpgSign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git {args:?}: {err}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_repo(root: &Path) {
    std::fs::write(
        root.join("web.rs"),
        "pub fn web_marker_search_target() -> &'static str { \"needle\" }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("account.rs"),
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
    git(root, &["init"]);
    git(root, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "init",
        ],
    );
}

fn create_editor_stub(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let _ = root;
        PathBuf::from(env!("CARGO_BIN_EXE_ig"))
    }

    #[cfg(not(windows))]
    {
        let path = root.join("editor.sh");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

fn http_request(port: u16, method: &str, path: &str, headers: &[(&str, &str)]) -> HttpResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\n");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        write!(request, "Host: 127.0.0.1:{port}\r\n").unwrap();
    }
    for (name, value) in headers {
        write!(request, "{name}: {value}\r\n").unwrap();
    }
    request.push_str("Content-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    HttpResponse {
        status,
        headers,
        body: body.to_string(),
    }
}

fn http_get(port: u16, path: &str) -> String {
    http_request(port, "GET", path, &[]).body
}

fn run_web_until_ready(home: &Path, repo: &Path, query: &str) -> String {
    run_web_until_ready_on_host(home, repo, query, "127.0.0.1")
}

fn run_web_until_ready_on_host(home: &Path, repo: &Path, query: &str, host: &str) -> String {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        let output = Command::new(bin())
            .args(["--web", "--host", host, "--port", "0", query])
            .arg(repo)
            .env("IVYGREP_HOME", home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .env("IVYGREP_NO_BROWSER", "1")
            .output()
            .unwrap();
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some((_, url)) = stdout.trim().split_once(" at ") {
                return url.to_string();
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("web server did not become ready");
}

fn port_from_url(url: &str) -> u16 {
    let after_host = url
        .strip_prefix("http://127.0.0.1:")
        .unwrap_or_else(|| panic!("unexpected URL {url}"));
    after_host
        .split(['/', '?'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn paths_and_sources(search: &serde_json::Value) -> (Vec<String>, BTreeSet<String>) {
    let hits = search["hits"].as_array().unwrap();
    let paths = hits
        .iter()
        .filter_map(|hit| hit["file_path"].as_str())
        .map(ToString::to_string)
        .collect();
    let sources = hits
        .iter()
        .flat_map(|hit| hit["sources"].as_array().unwrap())
        .filter_map(serde_json::Value::as_str)
        .map(ToString::to_string)
        .collect();
    (paths, sources)
}

#[test]
#[serial]
fn web_server_serves_status_search_and_file() {
    let home = tempdir().unwrap();
    let repo = tempdir().unwrap();
    create_repo(repo.path());

    let add = Command::new(bin())
        .args(["--add"])
        .arg(repo.path())
        .args(["--force", "--no-watch", "--hash", "--wait-for-enhancement"])
        .env("IVYGREP_HOME", home.path())
        .env_remove("IVYGREP_NO_AUTOSPAWN")
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "ig --add failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );

    let daemon = Command::new(bin())
        .arg("--daemon")
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_WEB_EDITOR", create_editor_stub(home.path()))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _daemon_guard = ChildGuard(daemon);

    let url = run_web_until_ready(home.path(), repo.path(), "web_marker");
    let port = port_from_url(&url);
    let second_url = run_web_until_ready(home.path(), repo.path(), "second_marker");
    assert_eq!(
        port,
        port_from_url(&second_url),
        "second --web should reuse the current daemon web listener"
    );

    let status_response = http_request(port, "GET", "/api/status", &[]);
    assert_eq!(status_response.status, 200);
    assert_eq!(
        status_response
            .headers
            .get("x-frame-options")
            .map(String::as_str),
        Some("DENY")
    );
    assert_eq!(
        status_response
            .headers
            .get("referrer-policy")
            .map(String::as_str),
        Some("no-referrer")
    );
    assert!(
        status_response
            .headers
            .get("content-security-policy")
            .is_some_and(|value| value.contains("frame-ancestors 'none'"))
    );
    let status: serde_json::Value = serde_json::from_str(&status_response.body).unwrap();
    assert_eq!(status["type"], "status");
    assert!(
        status["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|workspace| workspace["root"]
                .as_str()
                .unwrap()
                .ends_with(repo.path().file_name().unwrap().to_str().unwrap()))
    );

    let workspace = percent_encode(&repo.path().canonicalize().unwrap().display().to_string());
    let search_path =
        format!("/api/search?q=web_marker_search_target&workspace={workspace}&limit=5");
    let search: serde_json::Value = serde_json::from_str(&http_get(port, &search_path)).unwrap();
    assert!(
        search["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["file_path"].as_str().unwrap().ends_with("web.rs")),
        "search response: {search:#}"
    );

    let semantic_query = percent_encode("secure account renewal strategy");
    let alias_search: serde_json::Value = serde_json::from_str(&http_get(
        port,
        &format!("/api/search?q={semantic_query}&workspace={workspace}&type=rs&limit=10"),
    ))
    .unwrap();
    let canonical_search: serde_json::Value = serde_json::from_str(&http_get(
        port,
        &format!("/api/search?q={semantic_query}&workspace={workspace}&type=rust&limit=10"),
    ))
    .unwrap();
    let (alias_paths, alias_sources) = paths_and_sources(&alias_search);
    let (canonical_paths, canonical_sources) = paths_and_sources(&canonical_search);
    assert_eq!(alias_paths, canonical_paths);
    assert_eq!(alias_paths, vec!["account.rs"]);
    assert_eq!(alias_sources, canonical_sources);
    assert!(alias_sources.contains("semantic"));
    assert!(alias_sources.contains("hash"));

    std::fs::write(
        repo.path().join("web.rs"),
        "pub fn web_marker_search_target() -> &'static str { \"dirty context marker\" }\n",
    )
    .unwrap();
    let context_path = format!(
        "/api/search?mode=context&q={}&workspace={workspace}&since=main&budget_tokens=4000",
        percent_encode("panic at web.rs:1:7")
    );
    let context: serde_json::Value = serde_json::from_str(&http_get(port, &context_path)).unwrap();
    assert_eq!(context["context_pack"]["change_scope"]["since"], "main");
    assert_eq!(
        context["context_pack"]["change_scope"]["dirty_worktree"],
        true
    );
    assert_eq!(
        context["context_pack"]["referenced_paths"][0]["file_path"],
        "web.rs"
    );
    assert!(
        context["context_pack"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["preview"]
                .as_str()
                .is_some_and(|preview| preview.contains("dirty context marker"))),
        "context response: {context:#}"
    );
    let invalid_budget: serde_json::Value = serde_json::from_str(&http_get(
        port,
        &format!("/api/search?mode=context&q=task&workspace={workspace}&budget_tokens=1"),
    ))
    .unwrap();
    assert_eq!(
        invalid_budget["error"],
        "budget_tokens must be between 256 and 131072"
    );

    let all_search: serde_json::Value = serde_json::from_str(&http_get(
        port,
        "/api/search?q=web_marker_search_target&limit=5",
    ))
    .unwrap();
    let all_hit = all_search["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hit| hit["file_path"].as_str().unwrap().ends_with("web.rs"))
        .unwrap();
    let absolute_hit_path = all_hit["file_path"].as_str().unwrap();
    assert!(
        Path::new(absolute_hit_path).is_absolute(),
        "all-index hit path should be absolute: {all_search:#}"
    );

    let stream_body = http_get(
        port,
        "/api/search/stream?q=web_marker_search_target&limit=5",
    );
    assert!(stream_body.contains("event: results"), "{stream_body}");
    assert!(stream_body.contains("web.rs"), "{stream_body}");

    let file_path = format!("/api/file?workspace={workspace}&path=web.rs");
    let file: serde_json::Value = serde_json::from_str(&http_get(port, &file_path)).unwrap();
    assert!(
        file["text"]
            .as_str()
            .unwrap()
            .contains("web_marker_search_target")
    );

    let absolute_file_path = format!("/api/file?path={}", percent_encode(absolute_hit_path));
    let absolute_file: serde_json::Value =
        serde_json::from_str(&http_get(port, &absolute_file_path)).unwrap();
    assert!(
        absolute_file["text"]
            .as_str()
            .unwrap()
            .contains("web_marker_search_target")
    );

    let open_path = format!("/api/open?workspace={workspace}&path=web.rs&line=1");
    let get_open = http_request(port, "GET", &open_path, &[]);
    assert_eq!(get_open.status, 405);
    assert_eq!(
        get_open.headers.get("allow").map(String::as_str),
        Some("POST")
    );
    let open_response = http_request(port, "POST", &open_path, &[]);
    assert_eq!(open_response.status, 200);
    let open: serde_json::Value = serde_json::from_str(&open_response.body).unwrap();
    assert_eq!(open["ok"], true, "open response: {open:#}");
    assert_eq!(open["line"], 1);

    let tree_path = format!("/api/tree?workspace={workspace}");
    let tree: serde_json::Value = serde_json::from_str(&http_get(port, &tree_path)).unwrap();
    assert!(
        tree["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"].as_str().unwrap() == "web.rs"),
        "tree response: {tree:#}"
    );

    assert_eq!(
        http_request(port, "GET", "/api/status", &[("Host", "attacker.example")]).status,
        403
    );
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/status",
            &[("Origin", "https://attacker.example")]
        )
        .status,
        403
    );
}

#[test]
#[serial]
fn non_loopback_web_uses_token_cookie_and_rejects_unauthorized_api_calls() {
    let home = tempdir().unwrap();
    let repo = tempdir().unwrap();
    create_repo(repo.path());

    let add = Command::new(bin())
        .args(["--add"])
        .arg(repo.path())
        .args(["--force", "--no-watch", "--hash"])
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "ig --add failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );

    let daemon = Command::new(bin())
        .arg("--daemon")
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_WEB_EDITOR", create_editor_stub(home.path()))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _daemon_guard = ChildGuard(daemon);

    let url = run_web_until_ready_on_host(home.path(), repo.path(), "web_marker", "0.0.0.0");
    let port = port_from_url(&url);
    let target = url
        .strip_prefix(&format!("http://127.0.0.1:{port}"))
        .unwrap_or_else(|| panic!("unexpected URL {url}"));
    let token = url
        .split("token=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .unwrap_or_else(|| panic!("authenticated URL did not contain token: {url}"));
    assert_eq!(
        token.len(),
        64,
        "token should contain two UUIDs encoded as 64 hex characters"
    );
    assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));

    assert_eq!(http_request(port, "GET", "/api/status", &[]).status, 401);
    assert_eq!(
        http_request(port, "GET", "/api/not-found", &[]).status,
        401,
        "unknown API routes must not reveal unauthenticated behavior"
    );

    let bootstrap = http_request(port, "GET", target, &[]);
    assert_eq!(
        bootstrap.status, 303,
        "bootstrap response: {}",
        bootstrap.body
    );
    let cookie_header = bootstrap
        .headers
        .get("set-cookie")
        .expect("bootstrap must establish an auth cookie");
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    let cookie = cookie_header.split(';').next().unwrap().to_string();
    let location = bootstrap
        .headers
        .get("location")
        .expect("bootstrap must strip token through a redirect");
    assert!(!location.contains("token="));

    let html = http_request(port, "GET", location, &[("Cookie", &cookie)]);
    assert_eq!(html.status, 200);
    assert!(html.body.contains("name=\"ivygrep-boot\""));
    assert!(!html.body.contains(token), "token leaked into HTML source");

    let status = http_request(port, "GET", "/api/status", &[("Cookie", &cookie)]);
    assert_eq!(status.status, 200);
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/status",
            &[("Cookie", &cookie), ("Origin", "https://attacker.example")]
        )
        .status,
        403
    );
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/status",
            &[("Authorization", &format!("Bearer {token}"))]
        )
        .status,
        200,
        "bearer auth should remain available to non-browser clients"
    );
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/status",
            &[
                ("Host", "attacker.example/path"),
                ("Authorization", &format!("Bearer {token}"))
            ]
        )
        .status,
        403,
        "token must not bypass Host syntax validation"
    );
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/status",
            &[
                ("Host", "attacker.example"),
                ("Authorization", &format!("Bearer {token}"))
            ]
        )
        .status,
        403,
        "non-loopback web access accepts literal IP hosts only"
    );

    let workspace = percent_encode(&repo.path().canonicalize().unwrap().display().to_string());
    let open_path = format!("/api/open?workspace={workspace}&path=web.rs&line=1");
    assert_eq!(
        http_request(port, "GET", &open_path, &[("Cookie", &cookie)]).status,
        405
    );
    let origin = format!("http://127.0.0.1:{port}");
    assert_eq!(
        http_request(
            port,
            "POST",
            &open_path,
            &[("Cookie", &cookie), ("Origin", &origin)]
        )
        .status,
        200
    );
}
