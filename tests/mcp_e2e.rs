use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

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
        reader.read_line(&mut line).expect("Failed to read from stdout");
        serde_json::from_str(&line).expect("Invalid JSON returned from stdout")
    };

    // 1. Initialize
    send_request(&mut stdin, json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        }
    }));
    let init_res = read_response(&mut reader);
    assert_eq!(init_res["id"], 1);

    // 2. tools/list
    send_request(&mut stdin, json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }));
    let list_res = read_response(&mut reader);
    assert_eq!(list_res["id"], 2);
    let tools = list_res["result"]["tools"].as_array().expect("tools should be an array");
    assert!(tools.iter().any(|t| t["name"] == "ig_search"));
    assert!(tools.iter().any(|t| t["name"] == "ig_status"));

    // 3. tools/call ig_status
    send_request(&mut stdin, json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "ig_status",
            "arguments": {}
        }
    }));
    let status_res = read_response(&mut reader);
    assert_eq!(status_res["id"], 3);
    assert!(status_res["result"]["content"].as_array().is_some());

    // 4. tools/call ig_search
    send_request(&mut stdin, json!({
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
    }));
    let search_res = read_response(&mut reader);
    assert_eq!(search_res["id"], 4);
    let content = &search_res["result"]["content"][0]["text"];
    let content_str = content.as_str().unwrap();
    assert!(content_str.contains("test.rs"));

    // Close stdin and wait for exit
    drop(stdin);
    let status = child.wait().expect("Failed to wait on child");
    assert!(status.success());
}
