//! IPC round-trip tests for the daemon transport layer.
//!
//! Exercises bind → connect → request → response over the real platform IPC
//! (Unix sockets on macOS/Linux, TCP loopback on Windows).

use std::fs;
use std::path::Path;

use ivygrep::protocol::{BUILD_VERSION, DaemonRequest, DaemonResponse};
use serial_test::serial;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn isolate_home(home: &Path) {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    ivygrep::config::ensure_app_dirs().unwrap();
}

async fn roundtrip(request: &DaemonRequest) -> DaemonResponse {
    let mut stream = ivygrep::ipc::connect().await.expect("connect failed");

    let payload = serde_json::to_vec(request).unwrap();
    stream.write_all(&payload).await.unwrap();
    stream.write_all(b"\n").await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    serde_json::from_str(&line).expect("failed to parse response")
}

fn create_test_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("hello.rs"),
        "pub fn daemon_roundtrip_marker() -> &'static str { \"pass\" }\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "init",
        ])
        .current_dir(root)
        .output()
        .unwrap();
}

/// Mini daemon: accept one connection, dispatch the request, reply.
async fn serve_one(
    listener: &ivygrep::ipc::IpcListener,
    handler: impl Fn(DaemonRequest) -> DaemonResponse,
) {
    let (stream, _) = listener.accept().await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    let request: DaemonRequest = serde_json::from_str(&line).unwrap();
    let response = handler(request);

    let payload = serde_json::to_vec(&response).unwrap();
    let mut stream = reader.into_inner();
    stream.write_all(&payload).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
}

// ---------------------------------------------------------------------------
// 1. Status round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn daemon_ipc_status_roundtrip() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    let (listener, _) = ivygrep::ipc::bind().await.unwrap();

    let daemon_handle = tokio::spawn(async move {
        serve_one(&listener, |req| match req {
            DaemonRequest::Status => DaemonResponse::Status {
                workspaces: vec![],
                version: Some(BUILD_VERSION.to_string()),
            },
            _ => DaemonResponse::Error {
                message: "unexpected".into(),
            },
        })
        .await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let response = roundtrip(&DaemonRequest::Status).await;
    match response {
        DaemonResponse::Status {
            version,
            workspaces,
        } => {
            assert_eq!(version.as_deref(), Some(BUILD_VERSION));
            assert!(workspaces.is_empty());
        }
        other => panic!("expected Status, got: {other:?}"),
    }

    daemon_handle.await.unwrap();
    ivygrep::ipc::cleanup_socket();
}

// ---------------------------------------------------------------------------
// 2. Index + Search round-trip (real indexing over IPC)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn daemon_ipc_index_and_search_roundtrip() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    let repo_dir = tempdir().unwrap();
    create_test_repo(repo_dir.path());
    let repo_path = ivygrep::config::canonicalize_lossy(repo_dir.path()).unwrap();

    let (listener, _) = ivygrep::ipc::bind().await.unwrap();

    let daemon_handle = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let request: DaemonRequest = serde_json::from_str(&line).unwrap();
            let response = match request {
                DaemonRequest::Index { ref path, .. } => {
                    let workspace = ivygrep::workspace::Workspace::resolve(path).unwrap();
                    let model = ivygrep::embedding::create_model(true);
                    let stats =
                        ivygrep::indexer::index_workspace(&workspace, model.as_ref()).unwrap();
                    DaemonResponse::Ack {
                        message: format!("indexed {} files", stats.indexed_files),
                    }
                }
                DaemonRequest::Search {
                    ref path,
                    ref query,
                    limit,
                    context,
                    ..
                } => {
                    let workspace =
                        ivygrep::workspace::Workspace::resolve(path.as_ref().unwrap()).unwrap();
                    let model = ivygrep::embedding::create_model(true);
                    let options = ivygrep::search::SearchOptions {
                        limit,
                        context,
                        ..Default::default()
                    };
                    let hits = ivygrep::search::hybrid_search(
                        &workspace,
                        query,
                        Some(model.as_ref()),
                        &options,
                    )
                    .unwrap();
                    DaemonResponse::SearchResults { hits }
                }
                _ => DaemonResponse::Error {
                    message: "unexpected".into(),
                },
            };

            let payload = serde_json::to_vec(&response).unwrap();
            let mut stream = reader.into_inner();
            stream.write_all(&payload).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let index_response = roundtrip(&DaemonRequest::Index {
        path: repo_path.clone(),
        watch: false,
    })
    .await;

    match &index_response {
        DaemonResponse::Ack { message } => {
            assert!(message.contains("indexed"), "got: {message}");
        }
        other => panic!("expected Ack, got: {other:?}"),
    }

    let search_response = roundtrip(&DaemonRequest::Search {
        path: Some(repo_path.clone()),
        query: "daemon_roundtrip_marker".to_string(),
        limit: Some(10),
        context: 0,
        type_filter: None,
        include_globs: vec![],
        exclude_globs: vec![],
        scope_path: None,
        scope_is_file: false,
    })
    .await;

    match &search_response {
        DaemonResponse::SearchResults { hits } => {
            assert!(!hits.is_empty(), "should find daemon_roundtrip_marker");
            assert!(
                hits.iter()
                    .any(|h| h.file_path.to_string_lossy().contains("hello.rs")),
                "should include hello.rs, got: {hits:?}"
            );
        }
        other => panic!("expected SearchResults, got: {other:?}"),
    }

    daemon_handle.await.unwrap();
    ivygrep::ipc::cleanup_socket();
}

// ---------------------------------------------------------------------------
// 3. Multiple concurrent connections
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn daemon_ipc_multiple_concurrent_connections() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    let (listener, _) = ivygrep::ipc::bind().await.unwrap();

    let daemon_handle = tokio::spawn(async move {
        for _ in 0..3 {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();

                let _request: DaemonRequest = serde_json::from_str(&line).unwrap();
                let response = DaemonResponse::Status {
                    workspaces: vec![],
                    version: Some(BUILD_VERSION.to_string()),
                };

                let payload = serde_json::to_vec(&response).unwrap();
                let mut stream = reader.into_inner();
                stream.write_all(&payload).await.unwrap();
                stream.write_all(b"\n").await.unwrap();
            });
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut handles = vec![];
    for _ in 0..3 {
        handles.push(tokio::spawn(async {
            roundtrip(&DaemonRequest::Status).await
        }));
    }

    for handle in handles {
        let response = handle.await.unwrap();
        match response {
            DaemonResponse::Status { version, .. } => {
                assert_eq!(version.as_deref(), Some(BUILD_VERSION));
            }
            other => panic!("expected Status, got: {other:?}"),
        }
    }

    daemon_handle.await.unwrap();
    ivygrep::ipc::cleanup_socket();
}

// ---------------------------------------------------------------------------
// 4. Error propagation on bad path
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn daemon_ipc_error_on_bad_path() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    let (listener, _) = ivygrep::ipc::bind().await.unwrap();

    let daemon_handle = tokio::spawn(async move {
        serve_one(&listener, |req| match req {
            DaemonRequest::Index { ref path, .. } => {
                match ivygrep::workspace::Workspace::resolve(path) {
                    Ok(_) => DaemonResponse::Ack {
                        message: "should not happen".into(),
                    },
                    Err(err) => DaemonResponse::Error {
                        message: err.to_string(),
                    },
                }
            }
            _ => DaemonResponse::Error {
                message: "unexpected".into(),
            },
        })
        .await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let response = roundtrip(&DaemonRequest::Index {
        path: std::path::PathBuf::from("/nonexistent/path/that/does/not/exist"),
        watch: false,
    })
    .await;

    match response {
        DaemonResponse::Error { message } => {
            assert!(!message.is_empty(), "error message should not be empty");
        }
        other => panic!("expected Error, got: {other:?}"),
    }

    daemon_handle.await.unwrap();
    ivygrep::ipc::cleanup_socket();
}
