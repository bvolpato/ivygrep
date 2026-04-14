use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::config;
use crate::embedding::{EmbeddingModel, create_model};
use crate::indexer::{
    index_workspace, index_workspace_for_watcher, maybe_complete_neural_for_small_workspace,
    remove_workspace_index,
};
use crate::jobs::{self, JobKind, JobUpdate};
use crate::protocol::{BUILD_VERSION, DaemonRequest, DaemonResponse};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search, literal_search};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces};

struct WatchRegistration {
    _watcher: RecommendedWatcher,
    control: Arc<WatchControl>,
}

struct WatchControl {
    workspace: Workspace,
    notify: Notify,
    dirty: AtomicBool,
    indexing: AtomicBool,
    active: AtomicBool,
    pending_events: AtomicU64,
    coalesced_events: AtomicU64,
}

impl WatchControl {
    fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            notify: Notify::new(),
            dirty: AtomicBool::new(false),
            indexing: AtomicBool::new(false),
            active: AtomicBool::new(true),
            pending_events: AtomicU64::new(0),
            coalesced_events: AtomicU64::new(0),
        }
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.pending_events.fetch_add(1, Ordering::Relaxed);
        self.notify.notify_one();
    }

    fn snapshot_phase(&self) -> (&'static str, bool, bool, u64, u64) {
        let indexing = self.indexing.load(Ordering::Relaxed);
        let dirty = self.dirty.load(Ordering::Relaxed);
        let pending_events = self.pending_events.load(Ordering::Relaxed);
        let coalesced_events = self.coalesced_events.load(Ordering::Relaxed);
        let phase = if indexing {
            "indexing"
        } else if dirty {
            "dirty"
        } else {
            "idle"
        };
        (phase, indexing, dirty, pending_events, coalesced_events)
    }
}

#[derive(Clone)]
struct DaemonState {
    lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>>,
    watchers: Arc<Mutex<HashMap<String, WatchRegistration>>>,
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

    let (listener, socket_path) = crate::ipc::bind().await?;
    eprintln!("ivygrep daemon listening on {}", socket_path.display());

    // Defer model creation — the ONNX download happens on first use.
    let lazy_model: Arc<std::sync::OnceLock<Arc<dyn EmbeddingModel>>> =
        Arc::new(std::sync::OnceLock::new());

    let state = DaemonState {
        lazy_model: lazy_model.clone(),
        watchers: Arc::new(Mutex::new(HashMap::new())),
    };

    restore_configured_watchers(&state);

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

fn restore_configured_watchers(state: &DaemonState) {
    let workspaces = match list_workspaces() {
        Ok(workspaces) => workspaces,
        Err(err) => {
            warn!("failed to enumerate workspaces for watcher restore: {err:#}");
            return;
        }
    };

    for workspace in workspaces {
        if !workspace.watch_enabled || workspace.last_indexed_at_unix.is_none() {
            continue;
        }

        if let Err(err) = register_watcher(state, &workspace.root) {
            warn!(
                "failed to restore watcher for {}: {err:#}",
                workspace.root.display()
            );
        }
    }
}

fn stop_watcher(workspace: &Workspace, registration: WatchRegistration) {
    registration.control.active.store(false, Ordering::Relaxed);
    registration.control.notify.notify_waiters();
    let _ = jobs::finish_job(workspace, JobKind::Watcher, "stopped", None);
    let _ = std::fs::remove_file(workspace.watcher_pid_path());
}

fn stop_all_watchers(state: &DaemonState) {
    let registrations: Vec<_> = state.watchers.lock().drain().collect();
    for (_, registration) in registrations {
        let workspace = registration.control.workspace.clone();
        stop_watcher(&workspace, registration);
    }
}

async fn handle_connection(stream: crate::ipc::IpcStream, state: DaemonState) -> Result<()> {
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
        DaemonRequest::Index {
            path,
            watch,
            skip_gitignore,
        } => {
            let workspace = match Workspace::resolve(&path) {
                Ok(workspace) => workspace,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };

            // Respect skip_gitignore by updating metadata before indexing
            let _ = workspace.ensure_dirs();
            let mut meta = workspace
                .read_metadata()
                .unwrap_or(None)
                .unwrap_or_else(|| crate::workspace::WorkspaceMetadata {
                    id: workspace.id.clone(),
                    root: workspace.root.clone(),
                    created_at_unix: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    last_indexed_at_unix: None,
                    watch_enabled: watch,
                    skip_gitignore: false,
                    index_generation: 0,
                });

            if meta.skip_gitignore != skip_gitignore {
                meta.skip_gitignore = skip_gitignore;
            }
            meta.watch_enabled = watch;
            let _ = workspace.write_metadata(&meta);

            let index_workspace_target = workspace.clone();
            let index_result = tokio::task::spawn_blocking(move || {
                let hash_model = create_model(true);
                index_workspace(&index_workspace_target, hash_model.as_ref())
            })
            .await
            .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

            match index_result {
                Ok(summary) => {
                    if watch {
                        if let Err(err) = register_watcher(&state, &path) {
                            return DaemonResponse::Error {
                                message: format!("indexed but failed to watch: {err:#}"),
                            };
                        }
                    } else if let Some(registration) = state.watchers.lock().remove(&workspace.id) {
                        stop_watcher(&workspace, registration);
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
            skip_gitignore,
        } => {
            let state_clone = state.clone();

            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
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
                skip_gitignore,
            };

            let result = tokio::task::spawn_blocking(move || {
                let model = state_clone.get_model_or_fallback();
                let mut all_hits = Vec::new();
                let mut all_errors: Vec<String> = Vec::new();
                let ws_neural_missing: Vec<PathBuf> = workspaces
                    .iter()
                    .filter(|w| w.needs_neural_enhancement())
                    .map(|w| w.root.clone())
                    .collect();

                for workspace in &workspaces {
                    let _ = maybe_complete_neural_for_small_workspace(workspace);
                    match hybrid_search(workspace, &query, Some(model.as_ref()), &options) {
                        Ok(mut hits) => {
                            if path.is_none() {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
                        Err(err) => {
                            warn!(
                                "hybrid_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
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
                (
                    Vec::new(),
                    vec![format!("search task panicked: {join_err:#}")],
                )
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
            skip_gitignore,
        } => {
            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
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
                        skip_gitignore,
                    ) {
                        Ok(mut hits) => {
                            if path.is_none() {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
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
        DaemonRequest::LiteralSearch {
            path,
            query,
            limit,
            context,
            type_filter,
            include_globs,
            exclude_globs,
            scope_path,
            scope_is_file,
            skip_gitignore,
        } => {
            let workspaces = if let Some(ref p) = path {
                match Workspace::resolve(p) {
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
            let options = SearchOptions {
                limit,
                context,
                type_filter,
                include_globs,
                exclude_globs,
                scope_filter,
                skip_gitignore,
            };

            let result = tokio::task::spawn_blocking(move || {
                let mut all_hits = Vec::new();
                let mut all_errors: Vec<String> = Vec::new();
                for workspace in &workspaces {
                    match literal_search(workspace, &query, &options) {
                        Ok(mut hits) => {
                            if path.is_none() {
                                for hit in &mut hits {
                                    hit.file_path = workspace.root.join(&hit.file_path);
                                }
                            }
                            all_hits.append(&mut hits);
                        }
                        Err(err) => {
                            warn!(
                                "literal_search failed for {}: {err:#}",
                                workspace.root.display()
                            );
                            all_errors.push(format!("{}: {err:#}", workspace.root.display()));
                        }
                    }
                }

                if all_hits.is_empty() && !all_errors.is_empty() {
                    return Err(all_errors.join("; "));
                }

                if let Some(l) = options.limit {
                    all_hits.truncate(l);
                }
                Ok(all_hits)
            })
            .await
            .unwrap_or_else(|join_err| {
                warn!("literal search task panicked: {join_err:#}");
                Err(join_err.to_string())
            });

            match result {
                Ok(hits) => DaemonResponse::SearchResults { hits },
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::Remove { path } => match Workspace::resolve(&path) {
            Ok(workspace) => {
                // Stop watcher so no new indexing is triggered.
                if let Some(registration) = state.watchers.lock().remove(&workspace.id) {
                    stop_watcher(&workspace, registration);
                }
                if let Ok(Some(mut metadata)) = workspace.read_metadata() {
                    metadata.watch_enabled = false;
                    let _ = workspace.write_metadata(&metadata);
                }

                // Acquire the same fs2 lock that index_workspace holds to
                // wait for any in-progress indexing before deleting.
                match tokio::task::spawn_blocking(move || {
                    workspace.ensure_dirs().ok();
                    let lock_path = workspace.lock_path();
                    if let Ok(lock_file) = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(false)
                        .open(&lock_path)
                    {
                        // Blocking: waits for any running indexer to release.
                        let _ = fs2::FileExt::lock_exclusive(&lock_file);
                        let result = remove_workspace_index(&workspace);
                        let _ = fs2::FileExt::unlock(&lock_file);
                        result
                    } else {
                        remove_workspace_index(&workspace)
                    }
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())))
                {
                    Ok(_) => DaemonResponse::Ack {
                        message: format!("removed workspace index {}", path.display()),
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
            stop_all_watchers(&state);
            // Clean up socket so the new daemon can bind immediately
            crate::ipc::cleanup_socket();
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

    let mut watchers = state.watchers.lock();
    if watchers.contains_key(&workspace.id) {
        return Ok(());
    }

    let control = Arc::new(WatchControl::new(workspace.clone()));
    let callback_control = control.clone();

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            callback_control.mark_dirty();
        }
    })?;

    watcher.watch(&workspace.root, RecursiveMode::Recursive)?;
    watchers.insert(
        workspace.id.clone(),
        WatchRegistration {
            _watcher: watcher,
            control: control.clone(),
        },
    );
    drop(watchers);

    let _ = jobs::start_job(&workspace, JobKind::Watcher, "idle", 1);
    spawn_watch_heartbeat(control.clone());
    spawn_watch_worker(control);

    if let Ok(Some(mut metadata)) = workspace.read_metadata()
        && !metadata.watch_enabled
    {
        metadata.watch_enabled = true;
        let _ = workspace.write_metadata(&metadata);
    }

    // Write the daemon PID so the CLI can verify the watcher is alive
    // and skip expensive Merkle scans ("trust but verify").
    let _ = std::fs::write(workspace.watcher_pid_path(), std::process::id().to_string());

    eprintln!("watching {}", workspace.root.display());

    Ok(())
}

fn spawn_watch_heartbeat(control: Arc<WatchControl>) {
    tokio::spawn(async move {
        loop {
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            let (phase, indexing, dirty, pending_events, coalesced_events) =
                control.snapshot_phase();
            let mut update = JobUpdate {
                phase: Some(phase.to_string()),
                active: Some(true),
                ..Default::default()
            };
            update
                .details
                .insert("indexing".to_string(), indexing.to_string());
            update
                .details
                .insert("dirty".to_string(), dirty.to_string());
            update
                .details
                .insert("pending_events".to_string(), pending_events.to_string());
            update
                .details
                .insert("coalesced_events".to_string(), coalesced_events.to_string());
            let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, update);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

fn spawn_watch_worker(control: Arc<WatchControl>) {
    tokio::spawn(async move {
        loop {
            control.notify.notified().await;
            if !control.active.load(Ordering::Relaxed) {
                break;
            }

            if control.indexing.swap(true, Ordering::Relaxed) {
                continue;
            }

            loop {
                if !control.dirty.swap(false, Ordering::Relaxed) {
                    break;
                }

                let pending = control.pending_events.swap(0, Ordering::Relaxed);
                control
                    .coalesced_events
                    .fetch_add(pending.saturating_sub(1), Ordering::Relaxed);

                let mut update = JobUpdate {
                    phase: Some("indexing".to_string()),
                    active: Some(true),
                    ..Default::default()
                };
                update
                    .details
                    .insert("pending_events".to_string(), pending.to_string());
                let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, update);

                let workspace = control.workspace.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let hash_model = create_model(true);
                    let _ = index_workspace_for_watcher(&workspace, hash_model.as_ref())?;
                    Result::<(), anyhow::Error>::Ok(())
                })
                .await
                .unwrap_or_else(|join_err| Err(anyhow::anyhow!(join_err.to_string())));

                match result {
                    Ok(()) => {
                        eprintln!("watch update indexed {}", control.workspace.root.display());
                        let success = JobUpdate {
                            phase: Some(if control.dirty.load(Ordering::Relaxed) {
                                "dirty".to_string()
                            } else {
                                "idle".to_string()
                            }),
                            last_error: Some(None),
                            ..Default::default()
                        };
                        let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, success);
                    }
                    Err(err) => {
                        eprintln!(
                            "watch update failed for {}: {err:#}",
                            control.workspace.root.display()
                        );
                        warn!(
                            "watch-triggered indexing failed for {}: {err:#}",
                            control.workspace.root.display()
                        );
                        let failed = JobUpdate {
                            phase: Some("error".to_string()),
                            last_error: Some(Some(format!("{err:#}"))),
                            ..Default::default()
                        };
                        let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, failed);
                    }
                }
            }

            control.indexing.store(false, Ordering::Relaxed);
            let idle = JobUpdate {
                phase: Some(if control.dirty.load(Ordering::Relaxed) {
                    "dirty".to_string()
                } else {
                    "idle".to_string()
                }),
                ..Default::default()
            };
            let _ = jobs::heartbeat_job(&control.workspace, JobKind::Watcher, idle);
        }
    });
}

fn scope_from_request(scope_path: Option<PathBuf>, scope_is_file: bool) -> Option<WorkspaceScope> {
    scope_path.map(|rel_path| WorkspaceScope {
        rel_path,
        is_file: scope_is_file,
    })
}

pub async fn request(request: &DaemonRequest, autospawn: bool) -> Result<Option<DaemonResponse>> {
    if crate::ipc::socket_exists() && crate::ipc::connect().await.is_err() {
        crate::ipc::cleanup_socket();
    }

    // Auto-spawn the daemon if it isn't running.
    // Skip when IVYGREP_NO_AUTOSPAWN is set (for tests and CI).
    if autospawn
        && !crate::ipc::socket_exists()
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
                let log_stderr = log_file.try_clone();
                cmd.stdout(std::process::Stdio::from(log_file));
                if let Ok(stderr_file) = log_stderr {
                    cmd.stderr(std::process::Stdio::from(stderr_file));
                } else {
                    cmd.stderr(std::process::Stdio::null());
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }

            #[cfg(not(unix))]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let _ = cmd.spawn();
            // Poll for socket readiness (up to 2s)
            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if crate::ipc::socket_exists() {
                    break;
                }
            }
        }
    }

    if !crate::ipc::socket_exists() {
        return Ok(None);
    }

    // Timeout on connect — if the daemon is a zombie stuck in kernel sleep,
    // the connect() will hang. Don't let the CLI join the zombie pile.
    let mut stream = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        crate::ipc::connect(),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        _ => {
            // Connect timed out or failed — daemon is dead or zombie.
            // Remove the stale socket so we don't try again.
            crate::ipc::cleanup_socket();
            return Ok(None);
        }
    };

    let payload = serde_json::to_vec(request)?;
    // Timeout writes too — a zombie daemon may accept the connection
    // but never read from it, causing writes to eventually block.
    if tokio::time::timeout(std::time::Duration::from_secs(2), async {
        stream.write_all(&payload).await?;
        stream.write_all(b"\n").await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .is_err()
    {
        crate::ipc::cleanup_socket();
        return Ok(None);
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // Timeout varies by request type: Index can take 30+ min on massive repos
    // (dd-source: 270K files), while Status should complete in seconds.
    let timeout_secs = match request {
        DaemonRequest::Index { .. } => 1800, // 30 min for large repos
        DaemonRequest::Status | DaemonRequest::Restart => 5, // quick
        DaemonRequest::Search { .. }
        | DaemonRequest::RegexSearch { .. }
        | DaemonRequest::LiteralSearch { .. } => 120, // 2 min for search
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

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::embedding::create_hash_model;
    use crate::indexer::index_workspace;
    use crate::search::{SearchOptions, hybrid_search};
    use crate::workspace::WorkspaceMetadata;

    #[tokio::test]
    #[serial]
    async fn restore_configured_watchers_makes_workspace_live_and_updates_search() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn before_restart() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let metadata = WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_indexed_at_unix: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            watch_enabled: true,
            skip_gitignore: false,
            index_generation: 0,
        };
        workspace.write_metadata(&metadata).unwrap();

        let state = DaemonState {
            lazy_model: Arc::new(std::sync::OnceLock::new()),
            watchers: Arc::new(Mutex::new(HashMap::new())),
        };

        restore_configured_watchers(&state);

        let mut watcher_live = false;
        for _ in 0..20 {
            if crate::workspace::list_workspaces()
                .unwrap()
                .into_iter()
                .find(|status| status.id == workspace.id)
                .is_some_and(|status| status.watcher_alive)
            {
                watcher_live = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            watcher_live,
            "restored daemon should revive configured watcher"
        );

        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn after_restart() -> bool { true }\n",
        )
        .unwrap();

        let mut updated = false;
        for _ in 0..30 {
            let hits = hybrid_search(
                &workspace,
                "after restart",
                Some(model.as_ref()),
                &SearchOptions {
                    limit: Some(5),
                    ..Default::default()
                },
            )
            .unwrap();
            if hits.iter().any(|hit| hit.preview.contains("after_restart")) {
                updated = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            updated,
            "restored watcher should process file changes after daemon startup"
        );

        stop_all_watchers(&state);
    }

    #[tokio::test]
    #[serial]
    async fn concurrent_register_watcher_creates_exactly_one() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn concurrent_watcher_target() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();
        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        let state = DaemonState {
            lazy_model: Arc::new(std::sync::OnceLock::new()),
            watchers: Arc::new(Mutex::new(HashMap::new())),
        };

        let state_arc = Arc::new(state);
        let barrier = Arc::new(tokio::sync::Barrier::new(8));
        let repo_path = repo.path().to_path_buf();

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let state = Arc::clone(&state_arc);
                let barrier = Arc::clone(&barrier);
                let path = repo_path.clone();
                tokio::spawn(async move {
                    barrier.wait().await;
                    let _ = register_watcher(&state, &path);
                })
            })
            .collect();

        for h in handles {
            h.await.expect("register_watcher task panicked");
        }

        let watcher_count = state_arc.watchers.lock().len();
        assert_eq!(
            watcher_count, 1,
            "exactly one watcher should exist after concurrent registrations, got {watcher_count}"
        );

        stop_all_watchers(&state_arc);
    }
}
