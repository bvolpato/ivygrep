use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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
        let path = root.join("editor.cmd");
        std::fs::write(&path, "@echo off\r\nexit /b 0\r\n").unwrap();
        path
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

fn http_get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or(response)
}

fn run_web_until_ready(home: &Path, repo: &Path, query: &str) -> String {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        let output = Command::new(bin())
            .args(["--web", "--host", "127.0.0.1", "--port", "0", query])
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

#[test]
#[serial]
fn web_server_serves_status_search_and_file() {
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

    let url = run_web_until_ready(home.path(), repo.path(), "web_marker");
    let port = port_from_url(&url);
    let second_url = run_web_until_ready(home.path(), repo.path(), "second_marker");
    assert_eq!(
        port,
        port_from_url(&second_url),
        "second --web should reuse the current daemon web listener"
    );

    let status: serde_json::Value = serde_json::from_str(&http_get(port, "/api/status")).unwrap();
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
    let open: serde_json::Value = serde_json::from_str(&http_get(port, &open_path)).unwrap();
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
}
