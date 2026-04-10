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
