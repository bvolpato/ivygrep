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
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty()),
        "search response should contain text: {response}"
    );
    let payload = &response["result"]["structuredContent"];
    assert!(
        payload.is_object(),
        "search response should contain structuredContent: {response}"
    );
    payload.clone()
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
            "protocolVersion": "2025-11-25",
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
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
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
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(
        repo.join("src/lib.rs"),
        "mod helper;\npub fn execute_helper() -> u64 { helper::value() }\n",
    )
    .unwrap();
    std::fs::write(repo.join("src/helper.rs"), "pub fn value() -> u64 { 42 }\n").unwrap();
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
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    );

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

    // Unknown tools return a recoverable error without dropping the session.
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "ig_missing",
                "arguments": {}
            }
        }),
    );
    let unknown_tool = read_response(&mut reader);
    assert_eq!(unknown_tool["id"], 20);
    assert_eq!(unknown_tool["error"]["code"], -32602);
    assert!(
        unknown_tool["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );

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
    assert!(status_res["result"]["structuredContent"].is_object());

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
    assert!(search_res["result"]["structuredContent"].is_object());
    let payload = search_payload(&search_res);
    assert!(payload["result_count"].as_u64().unwrap() > 0);
    assert_eq!(payload["results"][0]["file_path"], "test.rs");
    assert!(payload["total_matches"].as_u64().unwrap() >= 1);
    assert!(payload["truncated"].is_boolean(), "{payload:#}");
    let search_text = search_res["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        serde_json::from_str::<Value>(search_text).is_err(),
        "hits text block should be a compact rendering, not JSON: {search_text}"
    );
    assert!(search_text.contains("test.rs"), "{search_text}");

    // 5. Build one bounded context pack through the same MCP tool.
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "ig_search",
                "arguments": {
                    "query": "change execute_helper behavior",
                    "path": repo.to_string_lossy().to_string(),
                    "output": "context_pack",
                    "budget_tokens": 1000
                }
            }
        }),
    );
    let context_res = read_response(&mut reader);
    assert_eq!(context_res["id"], 5);
    let context_payload = search_payload(&context_res);
    assert_eq!(context_payload["mode"], "context");
    assert_eq!(context_payload["context_pack"]["budget_tokens"], 1000);
    assert!(
        context_payload["context_pack"]["used_tokens"]
            .as_u64()
            .unwrap()
            <= 1000
    );
    let context_items = context_payload["context_pack"]["items"]
        .as_array()
        .expect("context pack items should be an array");
    assert!(
        context_items.iter().any(|item| {
            item["file_path"] == "src/helper.rs"
                && item["roles"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("dependency"))
        }),
        "MCP context pack should expand Rust dependencies: {context_payload:#}"
    );
    assert_eq!(context_res["result"]["structuredContent"], context_payload);

    // 6. Edit the workspace and verify the same MCP session reconciles it.
    std::fs::write(repo.join("test.rs"), "fn bar_after_agent_edit() {}").unwrap();
    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
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
    assert_eq!(refreshed_res["id"], 6);
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

fn create_bulk_repo(root: &Path, files: usize) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    for index in 0..files {
        let body = (0..30)
            .map(|line| {
                format!(
                    "pub fn bulk_first_call_symbol_{index}_{line}(value: u64) -> u64 {{ value.wrapping_add({}) }}\n",
                    line + 1
                )
            })
            .collect::<String>();
        fs::write(root.join("src").join(format!("module_{index}.rs")), body).unwrap();
    }
}

/// Indexing job records (`job.json`) under `home`, as (workspace dir, pid).
fn indexing_job_pids(home: &Path) -> Vec<(String, u64)> {
    let mut pids = Vec::new();
    let Ok(entries) = fs::read_dir(home.join("indexes")) else {
        return pids;
    };
    for entry in entries.flatten() {
        let Ok(raw) = fs::read_to_string(entry.path().join("job.json")) else {
            continue;
        };
        let Ok(ledger) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        for job in ledger["jobs"].as_array().into_iter().flatten() {
            if job["kind"] == "indexing"
                && let Some(pid) = job["pid"].as_u64()
            {
                pids.push((entry.file_name().to_string_lossy().to_string(), pid));
            }
        }
    }
    pids
}

/// First call on an unindexed workspace must return a bounded, non-error
/// `status: indexing` payload while the daemon keeps indexing; the MCP process
/// itself must not index locally; a later call returns hits.
#[test]
fn e2e_mcp_first_call_reports_indexing_instead_of_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("ivygrep_home");
    let repo = tmp.path().join("repo");
    create_bulk_repo(&repo, 3_000);

    let mut child = Command::new(assert_cmd::cargo::cargo_bin("ig"))
        .env("IVYGREP_HOME", &home)
        .env_remove("IVYGREP_NO_AUTOSPAWN")
        // Return right after the daemon accepts the run so the assertion does
        // not depend on how fast this machine indexes the fixture.
        .env("IVYGREP_MCP_INDEX_WAIT_SECS", "0")
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ig --mcp");
    let mcp_pid = u64::from(child.id());
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut daemon = DaemonGuard { pid: None };

    let call_search = |stdin: &mut std::process::ChildStdin,
                       reader: &mut BufReader<std::process::ChildStdout>,
                       id: usize|
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
                        "query": "bulk_first_call_symbol_7_3",
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

    let started = Instant::now();
    let first = call_search(&mut stdin, &mut reader, 1);
    let first_latency = started.elapsed();
    // Record the daemon pid before any assertion so a failure still stops it.
    if let Ok(raw) = fs::read_to_string(home.join("daemon.pid")) {
        daemon.pid = raw.trim().parse().ok();
    }
    let result = &first["result"];
    assert_eq!(result["isError"], false, "{first}");
    let status = &result["structuredContent"];
    assert_eq!(status["status"], "indexing", "{first}");
    assert_eq!(
        status["workspace_root"],
        json!(repo.canonicalize().unwrap())
    );
    assert!(status["progress"]["phase"].is_string(), "{status}");
    assert!(status["elapsed_secs"].is_u64(), "{status}");
    assert!(
        status["retry_after_secs"].as_u64().unwrap() >= 1,
        "{status}"
    );
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Indexing "), "{text}");
    assert!(text.contains("call ig_search again"), "{text}");
    // Bounded: autospawn plus one status poll, not the whole index run.
    assert!(
        first_latency < Duration::from_secs(15),
        "first call blocked for {first_latency:?}"
    );

    let daemon_pid = daemon.pid.expect("MCP autospawned a daemon");

    // The daemon owns the run: poll until results arrive, asserting that the
    // MCP process never started an index job of its own.
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut id = 2;
    let mut saw_daemon_job = false;
    let hits = loop {
        for (workspace, pid) in indexing_job_pids(&home) {
            assert_ne!(
                pid, mcp_pid,
                "MCP process indexed {workspace} locally while the daemon was indexing"
            );
            if pid == u64::from(daemon_pid) {
                saw_daemon_job = true;
            }
        }
        let response = call_search(&mut stdin, &mut reader, id);
        let result = &response["result"];
        assert_eq!(result["isError"], false, "{response}");
        if result["structuredContent"]["status"] != "indexing" {
            break search_payload(&response);
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not finish indexing the fixture: {response}"
        );
        id += 1;
        thread::sleep(Duration::from_millis(250));
    };
    for (workspace, pid) in indexing_job_pids(&home) {
        assert_ne!(pid, mcp_pid, "MCP process indexed {workspace} locally");
        if pid == u64::from(daemon_pid) {
            saw_daemon_job = true;
        }
    }
    assert!(
        saw_daemon_job,
        "index job was not recorded under the daemon pid"
    );
    assert!(hits["result_count"].as_u64().unwrap() > 0, "{hits}");
    assert!(
        !home
            .join("indexes")
            .read_dir()
            .unwrap()
            .any(|entry| { entry.unwrap().path().join(".indexing.pid").exists() })
    );

    drop(stdin);
    assert!(child.wait().unwrap().success());
}
