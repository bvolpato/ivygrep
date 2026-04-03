use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config;
use crate::embedding::{EmbeddingModel, create_model};
use crate::indexer::{index_workspace, remove_workspace_index};
use crate::protocol::{BUILD_VERSION, DaemonRequest, DaemonResponse};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces};

#[derive(Clone)]
struct DaemonState {
    lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>>,
    watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    trigger_tx: mpsc::UnboundedSender<PathBuf>,
}

impl DaemonState {
    /// Try to get the ONNX model without blocking. If it's not loaded yet,
    /// return a fast hash-based model so searches don't stall during startup.
    fn get_model_or_fallback(&self) -> Arc<dyn EmbeddingModel> {
        match self.lazy_model.get() {
            Some(model) => model.clone(),
            None => Arc::from(create_model(true)),
        }
    }
}

pub async fn run_daemon() -> Result<()> {
    config::ensure_app_dirs()?;

    let socket_path = config::socket_path()?;
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind socket {}", socket_path.display()))?;
    eprintln!("ivygrep daemon listening on {}", socket_path.display());

    let (trigger_tx, mut trigger_rx) = mpsc::unbounded_channel::<PathBuf>();

    // Defer model creation — the ONNX download happens on first use.
    let lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>> =
        Arc::new(std::sync::OnceLock::new());

    let state = DaemonState {
        lazy_model: lazy_model.clone(),
        watchers: Arc::new(Mutex::new(HashMap::new())),
        trigger_tx,
    };

    // Eagerly start loading the ONNX model in the background so it's ready
    // when the first search arrives. Searches that arrive before loading
    // completes will use a fast hash-based fallback.
    {
        let lazy = lazy_model.clone();
        std::thread::spawn(move || {
            lazy.get_or_init(|| {
                eprintln!("loading embedding model...");
                Arc::from(create_model(false))
            });
        });
    }

    tokio::spawn(async move {
        while let Some(path) = trigger_rx.recv().await {
            let index_path = path.clone();
            if let Err(err) = tokio::task::spawn_blocking(move || {
                let workspace = Workspace::resolve(&index_path)?;
                let hash_model = create_model(true);
                let _ = index_workspace(&workspace, hash_model.as_ref())?;
                Result::<(), anyhow::Error>::Ok(())
            })
            .await
            .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())))
            {
                eprintln!("watch update failed for {}: {err:#}", path.display());
                warn!(
                    "watch-triggered indexing failed for {}: {err:#}",
                    path.display()
                );
            } else {
                eprintln!("watch update indexed {}", path.display());
            }
        }
    });

    info!("ivygrep daemon listening on {}", socket_path.display());

    loop {
        let (stream, _addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                error!("daemon connection error: {err:#}");
            }
        });
    }
}

async fn handle_connection(stream: UnixStream, state: DaemonState) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        return Ok(());
    }

    let request: DaemonRequest = serde_json::from_str(&line)?;
    let response = handle_request(state, request).await;

    let payload = serde_json::to_vec(&response)?;
    let mut stream = reader.into_inner();
    stream.write_all(&payload).await?;
    stream.write_all(b"\n").await?;

    Ok(())
}

async fn handle_request(state: DaemonState, request: DaemonRequest) -> DaemonResponse {
    match request {
        DaemonRequest::Status => match list_workspaces() {
            Ok(workspaces) => DaemonResponse::Status {
                workspaces,
                version: Some(BUILD_VERSION.to_string()),
            },
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::Index { path, watch } => {
            let workspace = match Workspace::resolve(&path) {
                Ok(workspace) => workspace,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };

            let index_result = tokio::task::spawn_blocking(move || {
                let hash_model = create_model(true);
                index_workspace(&workspace, hash_model.as_ref())
            })
            .await
            .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

            match index_result {
                Ok(summary) => {
                    if watch && let Err(err) = register_watcher(&state, &path) {
                        return DaemonResponse::Error {
                            message: format!("indexed but failed to watch: {err:#}"),
                        };
                    }

                    DaemonResponse::Ack {
                        message: format!(
                            "indexed {} files ({} chunks)",
                            summary.indexed_files, summary.total_chunks
                        ),
                    }
                }
                Err(err) => DaemonResponse::Error {
                    message: err.to_string(),
                },
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
            scope_path,
            scope_is_file,
        } => {
            let state_clone = state.clone();

            let workspaces = if let Some(p) = path {
                match Workspace::resolve(&p) {
                    Ok(workspace) => vec![workspace],
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match list_workspaces() {
                    Ok(ws) => ws
                        .into_iter()
                        .filter(|w| w.last_indexed_at_unix.is_some())
                        .filter_map(|w| Workspace::resolve(&w.root).ok())
                        .collect(),
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };

            let options = SearchOptions {
                limit,
                context,
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter: scope_from_request(scope_path, scope_is_file),
            };

            let result = tokio::task::spawn_blocking(move || {
                let model = state_clone.get_model_or_fallback();
                let mut all_hits = Vec::new();
                let mut all_errors: Vec<String> = Vec::new();
                let ws_neural_missing: Vec<PathBuf> = workspaces
                    .iter()
                    .filter(|w| !w.vector_neural_path().exists() && !w.is_enhancing_active())
                    .map(|w| w.root.clone())
                    .collect();

                for workspace in &workspaces {
                    match hybrid_search(workspace, &query, Some(model.as_ref()), &options) {
                        Ok(mut hits) => all_hits.append(&mut hits),
                        Err(err) => {
                            warn!(
                                "hybrid_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!(
                                "{}: {err:#}",
                                workspace.root.display()
                            ));
                        }
                    }
                }
                all_hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(l) = options.limit {
                    all_hits.truncate(l);
                }
                // Spawn background neural enhancement for workspaces that need it
                if std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none() {
                    for root in ws_neural_missing {
                        if let Ok(ws) = Workspace::resolve(&root) {
                            let _ = ws.trigger_background_enhancement();
                        }
                    }
                }
                (all_hits, all_errors)
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("search task panicked: {join_err:#}");
                (Vec::new(), vec![format!("search task panicked: {join_err:#}")])
            });

            // If ALL workspaces failed (no hits and at least one error),
            // propagate as Error so the CLI can fall back to local search.
            if result.0.is_empty() && !result.1.is_empty() {
                DaemonResponse::Error {
                    message: format!("search failed: {}", result.1.join("; ")),
                }
            } else {
                DaemonResponse::SearchResults { hits: result.0 }
            }
        }
        DaemonRequest::RegexSearch {
            path,
            pattern,
            limit,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
        } => {
            let workspaces = if let Some(p) = path {
                match Workspace::resolve(&p) {
                    Ok(workspace) => vec![workspace],
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            } else {
                match list_workspaces() {
                    Ok(ws) => ws
                        .into_iter()
                        .filter(|w| w.last_indexed_at_unix.is_some())
                        .filter_map(|w| Workspace::resolve(&w.root).ok())
                        .collect(),
                    Err(err) => {
                        return DaemonResponse::Error {
                            message: err.to_string(),
                        };
                    }
                }
            };

            let scope_filter = scope_from_request(scope_path, scope_is_file);
            let result = tokio::task::spawn_blocking(move || {
                let mut all_hits = Vec::new();
                for workspace in &workspaces {
                    match regex_search(
                        workspace,
                        &pattern,
                        limit,
                        scope_filter.as_ref(),
                        &include_globs,
                        &exclude_globs,
                    ) {
                        Ok(mut hits) => all_hits.append(&mut hits),
                        Err(err) => {
                            warn!(
                                "regex_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                        }
                    }
                }

                if let Some(l) = limit {
                    all_hits.truncate(l);
                }

                all_hits
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("regex search task panicked: {join_err:#}");
                Vec::new()
            });

            DaemonResponse::SearchResults { hits: result }
        }
        DaemonRequest::Remove { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                state.watchers.lock().remove(&workspace.id);
                let _ = std::fs::remove_file(workspace.watcher_pid_path());
                match remove_workspace_index(&workspace) {
                    Ok(_) => DaemonResponse::Ack {
                        message: format!("removed workspace index {}", workspace.id),
                    },
                    Err(err) => DaemonResponse::Error {
                        message: err.to_string(),
                    },
                }
            }
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::Restart => {
            info!("restart requested, shutting down");
            // Clean up socket so the new daemon can bind immediately
            if let Ok(sp) = config::socket_path() {
                let _ = std::fs::remove_file(sp);
            }
            // Schedule exit after the response is sent
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            });
            DaemonResponse::Ack {
                message: "restarting".to_string(),
            }
        }
    }
}

fn register_watcher(state: &DaemonState, path: &std::path::Path) -> Result<()> {
    let workspace = Workspace::resolve(path)?;

    if state.watchers.lock().contains_key(&workspace.id) {
        return Ok(());
    }

    let trigger_tx = state.trigger_tx.clone();
    let root = workspace.root.clone();

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            let _ = trigger_tx.send(root.clone());
        }
    })?;

    watcher.watch(&workspace.root, RecursiveMode::Recursive)?;
    state.watchers.lock().insert(workspace.id.clone(), watcher);

    // Write the daemon PID so the CLI can verify the watcher is alive
    // and skip expensive Merkle scans ("trust but verify").
    let _ = std::fs::write(workspace.watcher_pid_path(), std::process::id().to_string());

    eprintln!("watching {}", workspace.root.display());

    Ok(())
}

fn scope_from_request(scope_path: Option<PathBuf>, scope_is_file: bool) -> Option<WorkspaceScope> {
    scope_path.map(|rel_path| WorkspaceScope {
        rel_path,
        is_file: scope_is_file,
    })
}

pub async fn request(request: &DaemonRequest, autospawn: bool) -> Result<Option<DaemonResponse>> {
    let socket_path = config::socket_path()?;

    if socket_path.exists() && UnixStream::connect(&socket_path).await.is_err() {
        let _ = std::fs::remove_file(&socket_path);
    }

    // Auto-spawn the daemon if it isn't running.
    // Skip when IVYGREP_NO_AUTOSPAWN is set (for tests and CI).
    if autospawn
        && !socket_path.exists()
        && std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        let is_ig = exe
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "ig");
        if is_ig {
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("--daemon");

            // Redirect daemon I/O to a log file to keep the CLI terminal clean.
            if let Ok(log_file) =
                config::app_home()
                    .map(|h| h.join("daemon.log"))
                    .and_then(|log_path| {
                        std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_path)
                            .map_err(|e| anyhow::anyhow!(e))
                    })
            {
                let log_stderr = log_file
                    .try_clone()
                    .unwrap_or_else(|_| std::fs::File::open("/dev/null").unwrap());
                cmd.stdout(std::process::Stdio::from(log_file));
                cmd.stderr(std::process::Stdio::from(log_stderr));
            }

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }

            let _ = cmd.spawn();
            // Poll for socket readiness (up to 2s)
            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if socket_path.exists() {
                    break;
                }
            }
        }
    }

    if !socket_path.exists() {
        return Ok(None);
    }

    // Timeout on connect — if the daemon is a zombie stuck in kernel sleep,
    // the connect() will hang. Don't let the CLI join the zombie pile.
    let mut stream = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        UnixStream::connect(&socket_path),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        _ => {
            // Connect timed out or failed — daemon is dead or zombie.
            // Remove the stale socket so we don't try again.
            let _ = std::fs::remove_file(&socket_path);
            return Ok(None);
        }
    };

    let payload = serde_json::to_vec(request)?;
    // Timeout writes too — a zombie daemon may accept the connection
    // but never read from it, causing writes to eventually block.
    if tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async {
            stream.write_all(&payload).await?;
            stream.write_all(b"\n").await?;
            Ok::<_, anyhow::Error>(())
        },
    )
    .await
    .is_err()
    {
        let _ = std::fs::remove_file(&socket_path);
        return Ok(None);
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // Timeout varies by request type: Index can take 30+ min on massive repos
    // (dd-source: 270K files), while Status should complete in seconds.
    let timeout_secs = match request {
        DaemonRequest::Index { .. } => 1800, // 30 min for large repos
        DaemonRequest::Status | DaemonRequest::Restart => 5, // quick
        DaemonRequest::Search { .. } | DaemonRequest::RegexSearch { .. } => 120, // 2 min for search
        DaemonRequest::Remove { .. } => 30,  // cleanup
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        reader.read_line(&mut line),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(_)) | Err(_) => return Ok(None),
    }

    if line.trim().is_empty() {
        return Ok(None);
    }

    let response: DaemonResponse = serde_json::from_str(&line)?;
    Ok(Some(response))
}
