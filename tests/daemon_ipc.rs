//! IPC round-trip tests for the daemon transport layer.
//!
//! Exercises bind → connect → request → response over the real platform IPC
//! (Unix sockets on macOS/Linux, TCP loopback on Windows).

use std::fs;
use std::path::Path;
use std::process::Command;

use ivygrep::protocol::{BUILD_VERSION, DaemonRequest, DaemonRequestEnvelope, DaemonResponse};
use serial_test::serial;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn isolate_home(home: &Path) {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    ivygrep::config::ensure_app_dirs().unwrap();
}

async fn roundtrip(request: &DaemonRequest) -> DaemonResponse {
    let mut stream = ivygrep::ipc::connect().await.expect("connect failed");

    let payload = serde_json::to_vec(&DaemonRequestEnvelope::new(request.clone())).unwrap();
    stream.write_all(&payload).await.unwrap();
    stream.write_all(b"\n").await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    serde_json::from_str(&line).expect("failed to parse response")
}

async fn bind_for_test() -> Option<(ivygrep::ipc::IpcListener, std::path::PathBuf)> {
    match ivygrep::ipc::bind().await {
        Ok(bound) => Some(bound),
        Err(err)
            if err
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied) =>
        {
            None
        }
        Err(err) => panic!("bind failed unexpectedly: {err:#}"),
    }
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

fn create_test_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("hello.rs"),
        "pub fn daemon_roundtrip_marker() -> &'static str { \"pass\" }\n",
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

/// Mini daemon: accept one connection, dispatch the request, reply.
async fn serve_one(
    listener: &ivygrep::ipc::IpcListener,
    handler: impl Fn(DaemonRequest) -> DaemonResponse,
) {
    let (stream, _) = listener.accept().await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();

    let envelope: DaemonRequestEnvelope = serde_json::from_str(&line).unwrap();
    let response = handler(envelope.request);

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

    let Some((listener, _)) = bind_for_test().await else {
        return;
    };

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

    let Some((listener, _)) = bind_for_test().await else {
        return;
    };

    let daemon_handle = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let envelope: DaemonRequestEnvelope = serde_json::from_str(&line).unwrap();
            let response = match envelope.request {
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
                    DaemonResponse::SearchResults {
                        hits,
                        warnings: Vec::new(),
                    }
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
        skip_gitignore: false,
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
        skip_gitignore: false,
        force_neural: false,
        disable_memory_expansion: true,
    })
    .await;

    match &search_response {
        DaemonResponse::SearchResults { hits, .. } => {
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

    let Some((listener, _)) = bind_for_test().await else {
        return;
    };

    let daemon_handle = tokio::spawn(async move {
        for _ in 0..3 {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();

                let _request: DaemonRequest = serde_json::from_str::<DaemonRequestEnvelope>(&line)
                    .unwrap()
                    .request;
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

    let Some((listener, _)) = bind_for_test().await else {
        return;
    };

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
        skip_gitignore: false,
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

// ---------------------------------------------------------------------------
// 5. Test --skip-gitignore support
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn daemon_ipc_skip_gitignore() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    let repo_dir = tempdir().unwrap();
    let repo_path = repo_dir.path();

    // Create a simple project structure
    fs::write(repo_path.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(repo_path.join("tracked.txt"), "hello ivygrep").unwrap();
    fs::write(repo_path.join("ignored.txt"), "hello ivygrep").unwrap(); // This should be ignored

    let Some((listener, _)) = bind_for_test().await else {
        return;
    };

    let daemon_handle = tokio::spawn(async move {
        for _ in 0..3 {
            serve_one(&listener, |req| match req {
                DaemonRequest::Index {
                    path,
                    skip_gitignore,
                    ..
                } => {
                    let workspace = ivygrep::workspace::Workspace::resolve(&path).unwrap();

                    let _ = workspace.ensure_dirs();
                    let mut meta = workspace
                        .read_metadata()
                        .unwrap_or(None)
                        .unwrap_or_else(|| ivygrep::workspace::WorkspaceMetadata {
                            id: workspace.id.clone(),
                            root: workspace.root.clone(),
                            created_at_unix: 0,
                            last_indexed_at_unix: None,
                            watch_enabled: false,
                            skip_gitignore: false,
                            index_generation: 0,
                        });
                    meta.skip_gitignore = skip_gitignore;
                    let _ = workspace.write_metadata(&meta);

                    let model = ivygrep::embedding::create_model(true);
                    ivygrep::indexer::index_workspace(&workspace, model.as_ref()).unwrap();
                    DaemonResponse::Ack {
                        message: "indexed".into(),
                    }
                }
                DaemonRequest::Search {
                    path,
                    query,
                    limit,
                    context,
                    type_filter,
                    include_globs,
                    exclude_globs,
                    scope_path: _,
                    scope_is_file: _,
                    skip_gitignore,
                    force_neural,
                    disable_memory_expansion: _,
                } => {
                    let workspace =
                        ivygrep::workspace::Workspace::resolve(path.as_ref().unwrap()).unwrap();
                    let model = ivygrep::embedding::create_model(true);
                    let options = ivygrep::search::SearchOptions {
                        limit,
                        context,
                        type_filter,
                        include_globs,
                        exclude_globs,
                        scope_filter: None,
                        skip_gitignore,
                        force_neural,
                        progress_tx: None,
                        cancel_token: None,
                    };
                    let hits = ivygrep::search::hybrid_search(
                        &workspace,
                        &query,
                        Some(model.as_ref()),
                        &options,
                    )
                    .unwrap();
                    DaemonResponse::SearchResults {
                        hits,
                        warnings: Vec::new(),
                    }
                }
                _ => DaemonResponse::Error {
                    message: "unexpected".into(),
                },
            })
            .await;
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 1. Index WITH skip_gitignore = true
    let index_response = roundtrip(&DaemonRequest::Index {
        path: repo_path.to_path_buf(),
        watch: false,
        skip_gitignore: true,
    })
    .await;

    match &index_response {
        DaemonResponse::Ack { .. } => {}
        other => panic!("expected Ack, got: {other:?}"),
    }

    // 2. Search WITH skip_gitignore = true - should return both tracked and ignored files
    let search_all = roundtrip(&DaemonRequest::Search {
        path: Some(repo_path.to_path_buf()),
        query: "hello ivygrep".to_string(),
        limit: Some(10),
        context: 0,
        type_filter: None,
        include_globs: vec![],
        exclude_globs: vec![],
        scope_path: None,
        scope_is_file: false,
        skip_gitignore: true,
        force_neural: false,
        disable_memory_expansion: true,
    })
    .await;

    match &search_all {
        DaemonResponse::SearchResults { hits, .. } => {
            assert_eq!(
                hits.len(),
                2,
                "should find matches in both tracked and ignored files with skip_gitignore"
            );
        }
        other => panic!("expected SearchResults, got: {other:?}"),
    }

    // 3. Search WITH skip_gitignore = false - should return only tracked file
    let search_tracked_only = roundtrip(&DaemonRequest::Search {
        path: Some(repo_path.to_path_buf()),
        query: "hello ivygrep".to_string(),
        limit: Some(10),
        context: 0,
        type_filter: None,
        include_globs: vec![],
        exclude_globs: vec![],
        scope_path: None,
        scope_is_file: false,
        skip_gitignore: false,
        force_neural: false,
        disable_memory_expansion: true,
    })
    .await;

    match &search_tracked_only {
        DaemonResponse::SearchResults { hits, .. } => {
            assert_eq!(
                hits.len(),
                1,
                "should find match only in tracked file when skip_gitignore is false. Hits: {:?}",
                hits
            );
            assert!(hits[0].file_path.to_string_lossy().contains("tracked.txt"));
        }
        other => panic!("expected SearchResults, got: {other:?}"),
    }

    daemon_handle.await.unwrap();
    ivygrep::ipc::cleanup_socket();
}

#[tokio::test]
#[serial]
async fn daemon_ipc_recovers_stale_socket() {
    let home = tempdir().unwrap();
    isolate_home(home.path());

    // Bind once to create the socket file
    let Some((listener, path)) = bind_for_test().await else {
        return;
    };

    // Drop the listener to leave a stale socket file on disk
    drop(listener);
    assert!(path.exists(), "Stale socket should be left on disk");

    // Attempting to bind again should succeed and replace it
    let Some((new_listener, new_path)) = bind_for_test().await else {
        panic!("Should have recovered from stale socket");
    };

    assert!(new_path.exists(), "New socket should exist");
    drop(new_listener);
    ivygrep::ipc::cleanup_socket();
}

// ---------------------------------------------------------------------------
// 7. Real daemon: lease waiters must not starve other workspaces, and
//    concurrent Index requests for one workspace coalesce into one run.
// ---------------------------------------------------------------------------

struct DaemonGuard(std::process::Child);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn spawn_real_daemon(home: &Path) -> DaemonGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_ig"))
        .arg("--daemon")
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_SKIP_WATCHER_RESTORE", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn ig --daemon");
    let guard = DaemonGuard(child);
    for _ in 0..100 {
        if ivygrep::ipc::socket_exists() && ivygrep::ipc::connect().await.is_ok() {
            return guard;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("daemon did not start listening");
}

fn create_bulk_repo(root: &Path, files: usize) {
    fs::create_dir_all(root.join("src")).unwrap();
    for index in 0..files {
        let body = (0..40)
            .map(|line| {
                format!(
                    "pub fn bulk_symbol_{index}_{line}(value: u64) -> u64 {{ value.wrapping_mul({}) }}\n",
                    line + 1
                )
            })
            .collect::<String>();
        fs::write(root.join("src").join(format!("module_{index}.rs")), body).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial]
async fn daemon_ipc_index_storm_does_not_starve_other_workspaces() {
    let home = tempdir().unwrap();
    isolate_home(home.path());
    if bind_for_test().await.is_none() {
        return;
    }
    ivygrep::ipc::cleanup_socket();

    let busy_dir = tempdir().unwrap();
    create_bulk_repo(busy_dir.path(), 600);
    let busy_path = ivygrep::config::canonicalize_lossy(busy_dir.path()).unwrap();
    let idle_dir = tempdir().unwrap();
    create_test_repo(idle_dir.path());
    let idle_path = ivygrep::config::canonicalize_lossy(idle_dir.path()).unwrap();

    let _daemon = spawn_real_daemon(home.path()).await;

    let idle_index = roundtrip(&DaemonRequest::Index {
        path: idle_path.clone(),
        watch: false,
        skip_gitignore: false,
    })
    .await;
    assert!(
        matches!(idle_index, DaemonResponse::Ack { .. }),
        "{idle_index:?}"
    );

    // More concurrent Index requests than CPU permits, all for one workspace.
    let storm = num_cpus::get().max(1) + 2;
    let mut index_tasks = Vec::new();
    for _ in 0..storm {
        let busy_path = busy_path.clone();
        index_tasks.push(tokio::spawn(async move {
            roundtrip(&DaemonRequest::Index {
                path: busy_path,
                watch: false,
                skip_gitignore: false,
            })
            .await
        }));
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let started = std::time::Instant::now();
    let idle_search = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        roundtrip(&DaemonRequest::LiteralSearch {
            path: Some(idle_path.clone()),
            query: "daemon_roundtrip_marker".to_string(),
            limit: Some(5),
            context: 0,
            type_filter: None,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: false,
        }),
    )
    .await
    .expect("search on an idle workspace must not queue behind the index storm");
    let idle_latency = started.elapsed();
    let storm_still_running = index_tasks.iter().any(|task| !task.is_finished());
    match idle_search {
        DaemonResponse::SearchResults { hits, .. } => assert!(!hits.is_empty()),
        other => panic!("expected SearchResults, got {other:?}"),
    }
    eprintln!(
        "idle workspace search took {idle_latency:?} while index storm running={storm_still_running}"
    );

    let mut messages = Vec::new();
    for task in index_tasks {
        match tokio::time::timeout(std::time::Duration::from_secs(120), task)
            .await
            .expect("index storm must finish")
            .unwrap()
        {
            DaemonResponse::Ack { message } => messages.push(message),
            other => panic!("expected Ack, got {other:?}"),
        }
    }
    // Exactly one request performs the full walk. Followers that arrived
    // before that walk started share its Ack; followers that arrived while it
    // was already scanning cannot rely on it and run an incremental rescan,
    // which finds nothing left to do.
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("indexed 600 files")),
        "the leader indexes the whole workspace: {messages:?}"
    );
    assert!(
        messages.iter().all(|message| {
            message.starts_with("indexed 600 files") || message.starts_with("indexed 0 files")
        }),
        "followers either share the walk or rescan to a no-op: {messages:?}"
    );
    let generation = ivygrep::workspace::Workspace::resolve(&busy_path)
        .unwrap()
        .read_metadata()
        .unwrap()
        .unwrap()
        .index_generation;
    assert_eq!(
        generation, 1,
        "exactly one index run committed; follower rescans were no-ops"
    );
}
