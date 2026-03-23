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
use crate::protocol::{DaemonRequest, DaemonResponse};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces};

#[derive(Clone)]
struct DaemonState {
    model: Arc<dyn EmbeddingModel>,
    watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    trigger_tx: mpsc::UnboundedSender<PathBuf>,
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

    let state = DaemonState {
        model: Arc::from(create_model(false)),
        watchers: Arc::new(Mutex::new(HashMap::new())),
        trigger_tx,
    };

    let indexing_state = state.clone();
    tokio::spawn(async move {
        while let Some(path) = trigger_rx.recv().await {
            let index_path = path.clone();
            let model = indexing_state.model.clone();
            if let Err(err) = tokio::task::spawn_blocking(move || {
                let workspace = Workspace::resolve(&index_path)?;
                let _ = index_workspace(&workspace, model.as_ref())?;
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
            Ok(workspaces) => DaemonResponse::Status { workspaces },
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        DaemonRequest::Index { path, watch } => {
            let model = state.model.clone();
            let workspace = match Workspace::resolve(&path) {
                Ok(workspace) => workspace,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };

            let index_result =
                tokio::task::spawn_blocking(move || index_workspace(&workspace, model.as_ref()))
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
            let model = state.model.clone();
            
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
                    Ok(ws) => ws.into_iter().filter(|w| w.last_indexed_at_unix.is_some()).filter_map(|w| Workspace::resolve(&w.root).ok()).collect(),
                    Err(err) => return DaemonResponse::Error { message: err.to_string() },
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
                let mut all_hits = Vec::new();
                for workspace in workspaces {
                    if let Ok(mut hits) = hybrid_search(&workspace, &query, model.as_ref(), &options) {
                        all_hits.append(&mut hits);
                    }
                }
                // Sort combined hits by score (descending)
                all_hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(l) = options.limit {
                    all_hits.truncate(l);
                }
                all_hits
            })
            .await
            .unwrap_or_else(|_join_err| {
                // If thread panicked, return empty hits or string
                Vec::new()
            });

            DaemonResponse::SearchResults { hits: result }
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
                    Ok(ws) => ws.into_iter().filter(|w| w.last_indexed_at_unix.is_some()).filter_map(|w| Workspace::resolve(&w.root).ok()).collect(),
                    Err(err) => return DaemonResponse::Error { message: err.to_string() },
                }
            };

            let scope_filter = scope_from_request(scope_path, scope_is_file);
            let result = tokio::task::spawn_blocking(move || {
                let mut all_hits = Vec::new();
                for workspace in workspaces {
                    if let Ok(mut hits) = regex_search(
                        &workspace,
                        &pattern,
                        limit,
                        scope_filter.as_ref(),
                        &include_globs,
                        &exclude_globs,
                    ) {
                        all_hits.append(&mut hits);
                    }
                }
                
                // Regex search score logic in Rust: wait, `regex_search` doesn't strictly score, but it has `score: 1.0` or file index order.
                // It's already sorted by file inside. Doing nothing keeps file order, which is fine.
                // Just cut off the limit:
                if let Some(l) = limit {
                    all_hits.truncate(l);
                }
                
                all_hits
            })
            .await
            .unwrap_or_else(|_join_err| {
                Vec::new() // return empty on panic
            });

            DaemonResponse::SearchResults { hits: result }
        }
        DaemonRequest::Remove { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                state.watchers.lock().remove(&workspace.id);
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
    state.watchers.lock().insert(workspace.id, watcher);
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
    
    // Auto-spawn the daemon if it isn't running, to provide a transparent frictionless background indexer.
    // Skip when IVYGREP_NO_AUTOSPAWN is set (useful in tests and CI).
    if autospawn && !socket_path.exists() && std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none() {
        if let Ok(exe) = std::env::current_exe() {
            let is_ig = exe
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "ig");
            if is_ig {
                let mut cmd = std::process::Command::new(exe);
                cmd.arg("--daemon");

                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    // Put daemon in its own process group so it survives Ctrl+C on the parent CLI
                    cmd.process_group(0);
                }

                // Spawn detached daemon process
                let _ = cmd.spawn();
                // Give it a brief moment to bind the socket
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
        }
    }

    if !socket_path.exists() {
        return Ok(None);
    }

    let mut stream = match UnixStream::connect(&socket_path).await {
        Ok(stream) => stream,
        Err(_) => return Ok(None),
    };

    let payload = serde_json::to_vec(request)?;
    stream.write_all(&payload).await?;
    stream.write_all(b"\n").await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.trim().is_empty() {
        return Ok(None);
    }

    let response: DaemonResponse = serde_json::from_str(&line)?;
    Ok(Some(response))
}
