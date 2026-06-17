use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct DaemonGuard {
    pid: Option<u32>,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let Some(pid) = self.pid else {
            return;
        };
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        #[cfg(windows)]
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
}

fn find_watcher_pid(home: &Path) -> Option<u32> {
    let indexes = home.join("indexes");
    for entry in fs::read_dir(indexes).ok()? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path().join(".watcher.pid");
        let Ok(raw_pid) = fs::read_to_string(path) else {
            continue;
        };
        if let Ok(pid) = raw_pid.trim().parse() {
            return Some(pid);
        }
    }
    None
}

fn search_payload(response: &Value) -> Value {
    serde_json::from_str(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("search response should contain text"),
    )
    .expect("search response text should contain JSON")
}

#[test]
fn e2e_mcp_initialize() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("ivygrep_home");

    // Spawn the `ig --mcp` binary process in standard stdio mode
    let bin_path = assert_cmd::cargo::cargo_bin("ig");
    let mut cmd = Command::new(bin_path);
    let mut child = cmd
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ig --mcp");

    // Construct the initialization payload
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        }
    });

    // Write to stdin and close it
    {
        let stdin = child.stdin.as_mut().expect("Failed to get stdin");
        writeln!(stdin, "{init_req}").expect("Failed to write to stdin");
    }

    // Read the response from stdout
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");

    // Parse the JSON response
    let response: Value = serde_json::from_str(&line).expect("Invalid JSON returned from stdout");

    // Assert expectations
    assert_eq!(response["id"], 1);
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response["result"]["protocolVersion"].is_string());
    assert!(response["result"]["capabilities"].is_object());

    // Wait for the server to spin down now that standard input is closed
    let status = child.wait().expect("Failed to wait on child");
    assert!(status.success());
}

#[test]
fn e2e_mcp_full_session() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("ivygrep_home");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("test.rs"), "fn foo() {}").unwrap();
    std::fs::create_dir_all(repo.join(".git")).unwrap();

    let bin_path = assert_cmd::cargo::cargo_bin("ig");
    let mut cmd = Command::new(bin_path);
    let mut child = cmd
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ig --mcp");

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    let send_request = |stdin: &mut std::process::ChildStdin, req: Value| {
        writeln!(stdin, "{}", req).expect("Failed to write to stdin");
    };

    let read_response = |reader: &mut BufReader<std::process::ChildStdout>| -> Value {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("Failed to read from stdout");
        serde_json::from_str(&line).expect("Invalid JSON returned from stdout")
    };

    // 1. Initialize
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "1.0.0" }
            }
        }),
    );
    let init_res = read_response(&mut reader);
    assert_eq!(init_res["id"], 1);

    // 2. tools/list
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    );
    let list_res = read_response(&mut reader);
    assert_eq!(list_res["id"], 2);
    let tools = list_res["result"]["tools"]
        .as_array()
        .expect("tools should be an array");
    assert!(tools.iter().any(|t| t["name"] == "ig_search"));
    assert!(tools.iter().any(|t| t["name"] == "ig_status"));

    // 3. tools/call ig_status
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "ig_status",
                "arguments": {}
            }
        }),
    );
    let status_res = read_response(&mut reader);
    assert_eq!(status_res["id"], 3);
    assert!(status_res["result"]["content"].as_array().is_some());

    // 4. tools/call ig_search
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "ig_search",
                "arguments": {
                    "query": "foo",
                    "path": repo.to_string_lossy().to_string(),
                    "literal": true
                }
            }
        }),
    );
    let search_res = read_response(&mut reader);
    assert_eq!(search_res["id"], 4);
    let payload = search_payload(&search_res);
    assert!(payload["result_count"].as_u64().unwrap() > 0);
    assert_eq!(payload["results"][0]["file_path"], "test.rs");

    // 5. Edit the workspace and verify the same MCP session reconciles it.
    std::fs::write(repo.join("test.rs"), "fn bar_after_agent_edit() {}").unwrap();
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "ig_search",
                "arguments": {
                    "query": "bar_after_agent_edit",
                    "path": repo.to_string_lossy().to_string(),
                    "literal": true
                }
            }
        }),
    );
    let refreshed_res = read_response(&mut reader);
    assert_eq!(refreshed_res["id"], 5);
    let refreshed_payload = search_payload(&refreshed_res);
    assert!(
        refreshed_payload["result_count"].as_u64().unwrap() > 0,
        "MCP search should reconcile edits made during the session: {refreshed_payload}"
    );

    // Close stdin and wait for exit.
    drop(stdin);
    let status = child.wait().expect("Failed to wait on child");
    assert!(status.success());
}

#[test]
fn e2e_mcp_autospawn_watches_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("ivygrep_home");
    let repo = tmp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::write(repo.join("test.rs"), "fn before_edit() {}").unwrap();

    let mut child = Command::new(assert_cmd::cargo::cargo_bin("ig"))
        .env("IVYGREP_HOME", &home)
        .env_remove("IVYGREP_NO_AUTOSPAWN")
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ig --mcp");
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut daemon = DaemonGuard { pid: None };

    let call_search = |stdin: &mut std::process::ChildStdin,
                       reader: &mut BufReader<std::process::ChildStdout>,
                       id: usize,
                       query: &str|
     -> Value {
        writeln!(
            stdin,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": "ig_search",
                    "arguments": {
                        "query": query,
                        "path": repo.to_string_lossy(),
                        "literal": true
                    }
                }
            })
        )
        .unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };

    let initial = call_search(&mut stdin, &mut reader, 1, "before_edit");
    assert!(search_payload(&initial)["result_count"].as_u64().unwrap() > 0);

    let deadline = Instant::now() + Duration::from_secs(5);
    while daemon.pid.is_none() && Instant::now() < deadline {
        if let Some(pid) = find_watcher_pid(&home) {
            daemon.pid = Some(pid);
        } else {
            thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(daemon.pid.is_some(), "MCP did not start the daemon watcher");

    fs::write(repo.join("test.rs"), "fn after_agent_edit() {}").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut id = 2;
    let refreshed = loop {
        let response = call_search(&mut stdin, &mut reader, id, "after_agent_edit");
        let payload = search_payload(&response);
        if payload["result_count"].as_u64().unwrap() > 0 {
            break response;
        }
        assert!(
            Instant::now() < deadline,
            "daemon watcher did not publish the edited file: {payload}"
        );
        id += 1;
        thread::sleep(Duration::from_millis(100));
    };
    assert!(search_payload(&refreshed)["result_count"].as_u64().unwrap() > 0);

    drop(stdin);
    assert!(child.wait().unwrap().success());
}
