use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use colored::Colorize;
use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

use crate::config;
use crate::daemon;
use crate::embedding::create_model;
use crate::indexer::{
    index_workspace, maybe_complete_neural_for_small_workspace, remove_workspace_index,
    workspace_is_indexed,
};
use crate::jobs::{self, JobKind, JobUpdate};
use crate::mcp;
use crate::protocol::{
    BUILD_VERSION, DaemonRequest, DaemonResponse, SearchHit, group_hits_by_file,
};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search, literal_search};
use crate::workspace::{
    Workspace, WorkspaceIndexState, list_workspaces, resolve_workspace_and_scope,
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "ivygrep", version, about = "Semantic grep that stays local", long_about = None)]
pub struct Cli {
    #[arg(value_name = "QUERY", required = false)]
    pub query: Option<String>,

    #[arg(value_name = "PATH", required = false)]
    pub query_path: Option<PathBuf>,

    #[arg(long = "add", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub add_path: Option<PathBuf>,

    #[arg(long = "rm", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub rm_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub status: bool,

    #[arg(long, default_value_t = false)]
    pub daemon: bool,

    #[arg(long, default_value_t = false)]
    pub mcp: bool,

    #[arg(long, default_value_t = false)]
    pub wait_for_enhancement: bool,

    #[arg(short, long, global = true)]
    pub force: bool,

    /// Launch the interactive terminal UI.
    #[arg(long = "interactive", visible_alias = "ui", global = true)]
    pub ui: bool,

    /// Fast exact-match search backed by the index. Deterministic results,
    /// orders of magnitude faster than grep/rg for indexed repos.
    #[arg(long, short = 'l', global = true)]
    pub literal: bool,

    /// Legacy regex mode (walks all files, no index). Use `rg` directly instead.
    #[arg(long, global = true, hide = true)]
    pub regex: bool,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(short = 'C', long, default_value_t = 2, global = true)]
    pub context: usize,

    #[arg(long = "type", global = true)]
    pub type_filter: Option<String>,

    #[arg(long, alias = "all", global = true)]
    pub all_indices: bool,

    #[arg(long, value_name = "GLOBS", value_delimiter = ',', global = true)]
    pub include: Vec<String>,

    #[arg(long, value_name = "GLOBS", value_delimiter = ',', global = true)]
    pub exclude: Vec<String>,

    #[arg(short = 'n', long, global = true)]
    pub limit: Option<usize>,

    #[arg(long, global = true, conflicts_with = "limit")]
    pub no_limit: bool,

    #[arg(long, global = true)]
    pub no_watch: bool,

    #[arg(long, global = true)]
    pub first_line_only: bool,

    #[arg(long, global = true)]
    pub file_name_only: bool,

    #[arg(long, global = true)]
    pub verbose: bool,

    #[arg(long, global = true)]
    pub skip_gitignore: bool,

    /// Use lightweight hash-based embeddings instead of the default ONNX
    /// neural model. Faster startup, no model download, lower quality.
    #[arg(long, global = true)]
    pub hash: bool,

    #[arg(long, hide = true, value_name = "PATH")]
    pub enhance_internal: Option<PathBuf>,
}

pub async fn run() -> Result<()> {
    init_tracing();
    config::ensure_app_dirs()?;

    if maybe_run_legacy_mcp_stdio()? {
        return Ok(());
    }

    if maybe_run_doctor_command()? {
        return Ok(());
    }

    let cli = Cli::parse();
    let action_count = [
        cli.add_path.is_some(),
        cli.rm_path.is_some(),
        cli.status,
        cli.daemon,
        cli.mcp,
    ]
    .iter()
    .filter(|flag| **flag)
    .count();

    if action_count > 1 {
        bail!("use only one action at a time: --add, --rm, --status, --daemon, or --mcp");
    }

    if cli.daemon {
        daemon::run_daemon().await?;
        return Ok(());
    }

    if cli.mcp {
        mcp::serve_stdio()?;
        return Ok(());
    }

    if cli.status {
        return run_status(cli.json).await;
    }

    if let Some(path) = &cli.add_path {
        return run_add(
            path,
            !cli.no_watch,
            cli.force,
            cli.skip_gitignore,
            cli.json,
            cli.hash,
        )
        .await;
    }

    if let Some(path) = &cli.rm_path {
        return run_remove(path, cli.json).await;
    }

    if let Some(path) = &cli.enhance_internal {
        let workspace = Workspace::resolve(path)?;
        workspace.ensure_dirs()?;

        // Write PID file so --status can show "enhancing..."
        let pid_path = workspace.enhancing_pid_path();
        let _ = std::fs::write(&pid_path, std::process::id().to_string());
        let _ = jobs::start_job(&workspace, JobKind::Enhancement, "starting", 1);
        let stop_heartbeat = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let heartbeat_stop = stop_heartbeat.clone();
        let heartbeat_workspace = workspace.clone();
        std::thread::spawn(move || {
            while !heartbeat_stop.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if heartbeat_stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let progress =
                    std::fs::read_to_string(heartbeat_workspace.enhancing_progress_path())
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty());
                let paused_reason =
                    std::fs::read_to_string(heartbeat_workspace.enhancing_paused_path())
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty());
                let mut update = JobUpdate {
                    phase: Some(if paused_reason.is_some() {
                        "paused".to_string()
                    } else if progress.is_some() {
                        "running".to_string()
                    } else {
                        "starting".to_string()
                    }),
                    ..Default::default()
                };
                if let Some(progress) = progress {
                    update.details.insert("progress".to_string(), progress);
                }
                if let Some(reason) = paused_reason {
                    update.details.insert("paused_reason".to_string(), reason);
                }
                let _ = jobs::heartbeat_job(&heartbeat_workspace, JobKind::Enhancement, update);
            }
        });

        let result = {
            match crate::embedding::create_neural_model_background() {
                Ok(model) => {
                    let enhance_res =
                        crate::indexer::enhance_workspace_neural(&workspace, model.as_ref());
                    if let Err(e) = &enhance_res {
                        let _ = std::fs::write(
                            workspace.index_dir.join(".enhancing.error"),
                            format!("Enhancement error: {:?}", e),
                        );
                    } else {
                        let _ = std::fs::remove_file(workspace.index_dir.join(".enhancing.error"));
                    }
                    enhance_res
                }
                Err(e) => {
                    let _ = std::fs::write(
                        workspace.index_dir.join(".enhancing.error"),
                        format!("Model init error: {:?}", e),
                    );
                    Ok(0)
                }
            }
        };

        // ONNX clean teardown can sometimes segfault in multithreaded handlers.
        // We'll intentionally skip proper Rust panic runtime teardown and forcefully exit.
        if let Err(e) = result {
            stop_heartbeat.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = jobs::finish_job(
                &workspace,
                JobKind::Enhancement,
                "failed",
                Some(format!("{e:#}")),
            );
            let _ = std::fs::remove_file(&pid_path);
            let _ = std::fs::remove_file(workspace.enhancing_progress_path());
            eprintln!("Background enhancement failed: {:?}", e);
            std::process::exit(1);
        }
        stop_heartbeat.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = jobs::finish_job(&workspace, JobKind::Enhancement, "completed", None);
        let _ = std::fs::remove_file(&pid_path);
        let _ = std::fs::remove_file(workspace.enhancing_progress_path());
        std::process::exit(0);
    }

    if cli.ui {
        return crate::tui::run_tui(cli).await;
    }

    run_query(cli).await
}

async fn run_status(json: bool) -> Result<()> {
    // Read status directly from the filesystem — no need to route through
    // the daemon socket. Status data (SQLite stats, PID files, metadata)
    // is all local. This avoids blocking when the daemon is busy.
    let workspaces = list_workspaces()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
    } else if workspaces.is_empty() {
        println!("No indexed workspaces.");
        println!(
            "\n  Run \x1b[1mig \"query\"\x1b[0m in a project to auto-index, or \x1b[1mig --add .\x1b[0m to register one."
        );
    } else {
        let mut grouped: std::collections::BTreeMap<
            std::path::PathBuf,
            Vec<&crate::workspace::WorkspaceStatus>,
        > = std::collections::BTreeMap::new();

        for ws in &workspaces {
            let key = ws.base_repo_root.clone().unwrap_or_else(|| ws.root.clone());
            grouped.entry(key).or_default().push(ws);
        }

        for (base_root, mut wss) in grouped {
            wss.sort_by(|a, b| {
                let a_is_base = a.base_repo_root.is_none();
                let b_is_base = b.base_repo_root.is_none();
                b_is_base.cmp(&a_is_base).then_with(|| a.root.cmp(&b.root))
            });

            // Make sure the group itself has a visually distinct header
            // if the base repo isn't explicitly listed as an active workspace.
            if wss
                .first()
                .map(|w| w.base_repo_root.is_some())
                .unwrap_or(false)
            {
                println!("\x1b[1;36m⟐ {}\x1b[0m", base_root.display());
                println!("  \x1b[90m(Base repository not directly tracked by ivygrep)\x1b[0m\n");
            }

            for ws in wss {
                let is_overlay = ws.base_repo_root.is_some();
                let prefix = if is_overlay { "  " } else { "" };

                if is_overlay {
                    println!("  \x1b[1;35m↳ Overlay: {}\x1b[0m", ws.root.display());
                } else {
                    println!("\x1b[1;36m⟐ {}\x1b[0m", ws.root.display());
                }

                println!("{prefix}  ID:     {}", ws.id);

                // Index timestamp
                match ws.last_indexed_at_unix {
                    Some(ts) => {
                        let ago = format_timestamp_ago(ts);
                        println!("{prefix}  Index:  \x1b[32m✓ indexed\x1b[0m ({ago})");
                    }
                    None if ws.indexing_in_progress => {
                        println!("{prefix}  Index:  \x1b[1;33m⟳ initial indexing\x1b[0m");
                    }
                    None => {
                        println!("{prefix}  Index:  \x1b[33m⚠ never indexed\x1b[0m");
                    }
                }

                // Daemon/watcher
                if ws.watch_enabled && ws.watcher_alive {
                    println!("{prefix}  Watch:  \x1b[32m● configured + live\x1b[0m");
                } else if ws.watch_enabled {
                    println!("{prefix}  Watch:  \x1b[1;33m◐ configured, watcher offline\x1b[0m");
                } else {
                    println!("{prefix}  Watch:  \x1b[90m○ static\x1b[0m");
                }

                // Chunk stats
                if is_overlay {
                    println!(
                        "{prefix}  Files:  {} files, {} chunks (overlaid delta)",
                        ws.file_count, ws.chunk_count
                    );
                } else {
                    println!(
                        "{prefix}  Files:  {} files, {} chunks",
                        ws.file_count, ws.chunk_count
                    );
                }

                // Index size
                let size = format_bytes(ws.index_size_bytes);
                println!("{prefix}  Size:   {size}");

                // Embedding status
                if ws.enhancing_in_progress {
                    let accel = crate::embedding::hardware_acceleration_info();

                    let progress_str = if let Some(count) = ws.enhancing_progress_count {
                        let pct = if ws.chunk_count > 0 {
                            (count as f64 / ws.chunk_count as f64 * 100.0).min(100.0) as u64
                        } else {
                            100
                        };
                        format!("({count} / {} chunks, ~{pct}%), ", ws.chunk_count)
                    } else {
                        String::new()
                    };

                    if let Some(reason) = &ws.enhancing_paused_reason {
                        println!(
                            "{prefix}  Search: \x1b[1;33m⟳ enhancing [PAUSED]\x1b[0m {progress_str}(Paused: {reason})"
                        );
                    } else {
                        println!(
                            "{prefix}  Search: \x1b[1;33m⟳ enhancing\x1b[0m {progress_str}(computing {accel} in background...)"
                        );
                    }
                } else if ws.enhancing_stalled {
                    println!(
                        "{prefix}  Search: \x1b[1;31m⚠ stalled neural upgrade\x1b[0m (run `ig doctor` or retry a query)"
                    );
                } else if ws.has_neural_vectors {
                    let pct = if ws.chunk_count > 0 {
                        let ratio = (ws.neural_vector_count as f64 / ws.chunk_count as f64) * 100.0;
                        format!("{:.0}%", ratio.min(100.0))
                    } else {
                        "100%".to_string()
                    };
                    let accel = crate::embedding::hardware_acceleration_info();
                    println!(
                        "{prefix}  Search: \x1b[1;32m★ neural\x1b[0m ({} enhanced, {pct}, {accel})",
                        ws.neural_vector_count
                    );
                } else if ws.indexing_in_progress {
                    let progress_str = ws.indexing_progress.as_deref().unwrap_or("starting");
                    let detail = if progress_str == "scanning" {
                        "scanning filesystem...".to_string()
                    } else if progress_str.contains('/') {
                        format!("{progress_str} files")
                    } else {
                        progress_str.to_string()
                    };
                    println!("{prefix}  Search: \x1b[1;33m⟳ indexing\x1b[0m ({detail})");
                } else if ws.indexing_stalled {
                    println!(
                        "{prefix}  Search: \x1b[1;31m⚠ stalled indexing\x1b[0m (run `ig doctor --fix`)"
                    );
                } else if is_overlay {
                    if ws.chunk_count > 0 {
                        println!(
                            "{prefix}  Search: \x1b[33m◆ hash\x1b[0m (+ base neural/hash delegation)"
                        );
                    } else {
                        println!(
                            "{prefix}  Search: \x1b[35m⟐ overlay\x1b[0m (fully delegated to base)"
                        );
                    }
                } else if let Some(err) = &ws.enhancing_error {
                    let err_line = err.lines().next().unwrap_or("unknown error");
                    if err_line.contains("neural feature not compiled") {
                        // Expected for static/musl builds — not an error
                        println!(
                            "{prefix}  Search: \x1b[33m◆ hash\x1b[0m (neural not available in this build)"
                        );
                    } else {
                        // Real ONNX failure
                        println!(
                            "{prefix}  Search: \x1b[1;31m⚠️ neural upgrade failed\x1b[0m (run `ig query` to retry, or check .enhancing.error)"
                        );
                        println!("{prefix}          Error: \x1b[31m{err_line}\x1b[0m");
                    }
                } else if ws.chunk_count > 0 {
                    println!(
                        "{prefix}  Search: \x1b[33m◆ hash\x1b[0m (fast, run a query to trigger neural upgrade)"
                    );
                } else {
                    println!("{prefix}  Search: \x1b[90m○ empty\x1b[0m");
                }

                println!();
            }
        }

        // Summary
        let total_files: u64 = workspaces.iter().map(|w| w.file_count).sum();
        let total_chunks: u64 = workspaces.iter().map(|w| w.chunk_count).sum();
        let total_size: u64 = workspaces.iter().map(|w| w.index_size_bytes).sum();
        let neural_count = workspaces.iter().filter(|w| w.has_neural_vectors).count();
        println!(
            "\x1b[90m{} workspace(s), {} files, {} chunks, {} on disk, {}/{} neural\x1b[0m",
            workspaces.len(),
            total_files,
            total_chunks,
            format_bytes(total_size),
            neural_count,
            workspaces.len(),
        );
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_timestamp_ago(unix_ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ago = now.saturating_sub(unix_ts);
    if ago < 60 {
        format!("{ago}s ago")
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h ago", ago / 3600)
    } else {
        format!("{}d ago", ago / 86400)
    }
}

fn should_autospawn_daemon_for_query(workspace: &Workspace, no_watch: bool) -> bool {
    if no_watch {
        return false;
    }

    workspace
        .read_metadata()
        .ok()
        .flatten()
        .is_some_and(|meta| meta.watch_enabled)
}

async fn run_add(
    path: &Path,
    watch: bool,
    force: bool,
    skip_gitignore: bool,
    json: bool,
    _hash: bool,
) -> Result<()> {
    let workspace = Workspace::resolve(path)?;

    ensure_no_nested_workspaces(&workspace.root)?;

    let mut meta =
        workspace
            .read_metadata()?
            .unwrap_or_else(|| crate::workspace::WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                last_indexed_at_unix: None,
                watch_enabled: watch,
                skip_gitignore,
                index_generation: 0,
            });
    meta.watch_enabled = watch;
    if skip_gitignore {
        meta.skip_gitignore = true;
    }
    workspace.ensure_dirs()?;
    workspace.write_metadata(&meta)?;

    if force {
        let remove_request = DaemonRequest::Remove {
            path: workspace.root.clone(),
        };

        if let Some(response) =
            daemon::request::<fn(String, usize, usize)>(&remove_request, false, None).await?
        {
            if let DaemonResponse::Error { message } = response {
                bail!(message);
            }
        } else {
            remove_workspace_index(&workspace)?;
        }
    }

    if !force && workspace_is_indexed(&workspace) && !json {
        println!("Workspace already indexed: {}", workspace.root.display());
        println!("Use --force to rebuild from scratch.");
    }

    let request = DaemonRequest::Index {
        path: workspace.root.clone(),
        watch,
        skip_gitignore,
    };

    if let Some(response) =
        daemon::request::<fn(String, usize, usize)>(&request, watch, None).await?
    {
        return print_daemon_response(response, json);
    }

    let model = crate::embedding::create_hash_model();
    let summary = index_workspace(&workspace, model.as_ref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!(
            "Indexed {} files ({} chunks, {} deleted)",
            summary.indexed_files, summary.total_chunks, summary.deleted_files
        );
    }

    Ok(())
}

async fn run_remove(path: &Path, json: bool) -> Result<()> {
    let workspace = Workspace::resolve(path)?;
    let request = DaemonRequest::Remove {
        path: workspace.root.clone(),
    };
    if let Some(response) =
        daemon::request::<fn(String, usize, usize)>(&request, false, None).await?
    {
        return print_daemon_response(response, json);
    }

    remove_workspace_index(&workspace)?;

    if json {
        println!("{}", serde_json::json!({"removed": workspace.id}));
    } else {
        println!("Removed index for {}", workspace.root.display());
    }

    Ok(())
}

async fn run_query(cli: Cli) -> Result<()> {
    let query = cli
        .query
        .as_deref()
        .context("missing query. Example: ig \"where is tax calculated\"")?;

    let query_path = match &cli.query_path {
        Some(path) => path.clone(),
        None => env::current_dir()?,
    };
    let (workspace, scope_filter) = resolve_workspace_and_scope(&query_path)?;
    let _ = workspace.cleanup_stale_legacy_runtime_files();
    let watch_configured = should_autospawn_daemon_for_query(&workspace, cli.no_watch);
    let scope_path = scope_filter.as_ref().map(|scope| scope.rel_path.clone());
    let scope_is_file = scope_filter.as_ref().is_some_and(|scope| scope.is_file);
    let initial_index_state = (!cli.all_indices).then(|| workspace.index_health().state);

    let query_path_opt = if cli.all_indices {
        None
    } else {
        Some(workspace.root.clone())
    };
    let mut search_via_daemon = false;

    let backend_limit = if cli.no_limit || cli.file_name_only {
        Some(usize::MAX)
    } else {
        cli.limit
    };

    let display_limit = if cli.no_limit || (cli.file_name_only && cli.limit.is_none()) {
        Some(usize::MAX)
    } else {
        cli.limit
    };

    if !cli.all_indices {
        let first_run = matches!(initial_index_state, Some(WorkspaceIndexState::NotIndexed));
        let needs_repair = matches!(initial_index_state, Some(WorkspaceIndexState::Unhealthy));
        if first_run || needs_repair {
            // Always show progress for first-run, even when the daemon handles it.
            let msg = if needs_repair {
                "Index unhealthy — rebuilding"
            } else if workspace.is_worktree() {
                "First run — computing worktree overlay"
            } else {
                "First run — indexing"
            };
            eprintln!(
                "{} {} {}",
                "⟐".bold(),
                msg.bold(),
                workspace.root.display().to_string().dimmed()
            );

            let daemon_index_request = DaemonRequest::Index {
                path: workspace.root.clone(),
                watch: !cli.no_watch,
                skip_gitignore: cli.skip_gitignore,
            };

            // Send the index request to the daemon, but show a progress spinner
            // while we wait so the user knows work is happening.
            let ws_id = workspace.id.clone();
            let show_progress = std::io::stderr().is_terminal();

            let response_future = daemon::request::<fn(String, usize, usize)>(
                &daemon_index_request,
                !cli.no_watch,
                None,
            );

            if show_progress {
                // Poll for progress while waiting for the daemon to finish indexing
                let progress_handle = tokio::spawn({
                    let ws_id = ws_id.clone();
                    async move {
                        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                        let mut tick = 0usize;
                        let mut cached_msg = String::new();
                        loop {
                            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                            let frame = spinner[tick % spinner.len()];
                            tick += 1;

                            // Poll workspace status every ~640ms (every 8th frame)
                            if tick % 8 == 1
                                && let Ok(ws_list) = crate::workspace::list_workspaces()
                                && let Some(status) = ws_list.iter().find(|w| w.id == ws_id)
                            {
                                if status.indexing_in_progress {
                                    if let Some(ref progress) = status.indexing_progress {
                                        if progress == "scanning" {
                                            cached_msg = "scanning filesystem...".to_string();
                                        } else {
                                            cached_msg = format!("indexing {progress} files...");
                                        }
                                    } else {
                                        cached_msg = "indexing...".to_string();
                                    }
                                } else {
                                    cached_msg = format!(
                                        "{} files, {} chunks indexed",
                                        status.file_count, status.chunk_count
                                    );
                                }
                            }

                            if cached_msg.is_empty() {
                                eprint!("\r\x1b[K  {} indexing...", frame);
                            } else {
                                eprint!("\r\x1b[K  {} {}", frame, cached_msg);
                            }
                        }
                    }
                });

                let result = response_future.await;
                progress_handle.abort();
                eprint!("\r\x1b[K"); // clear spinner line

                if let Ok(Some(response)) = result {
                    if let DaemonResponse::Error { message } = response {
                        bail!(message);
                    }
                    search_via_daemon = true;
                }
            } else {
                // Non-interactive: just wait silently
                if let Some(response) = response_future.await? {
                    if let DaemonResponse::Error { message } = response {
                        bail!(message);
                    }
                    search_via_daemon = true;
                }
            }
        } else {
            // Already indexed. Just check if the daemon is online to route the search request.
            // Also verify the daemon version matches — stale daemons silently break search.
            let _t = std::time::Instant::now();
            match daemon::request::<fn(String, usize, usize)>(
                &DaemonRequest::Status,
                watch_configured,
                None,
            )
            .await?
            {
                Some(DaemonResponse::Status {
                    version,
                    workspaces,
                }) => {
                    if version.as_deref() == Some(BUILD_VERSION) {
                        let watcher_offline = watch_configured
                            && workspaces
                                .iter()
                                .find(|status| status.id == workspace.id)
                                .is_some_and(|status| {
                                    status.watch_enabled && !status.watcher_alive
                                });
                        if watcher_offline {
                            tracing::warn!(
                                "daemon online but watcher offline for {}, restarting",
                                workspace.root.display()
                            );
                            restart_daemon().await;
                            search_via_daemon = daemon::request::<fn(String, usize, usize)>(
                                &DaemonRequest::Status,
                                true,
                                None,
                            )
                            .await?
                            .is_some();
                        } else {
                            search_via_daemon = true;
                        }
                    } else {
                        tracing::warn!(
                            "daemon version mismatch: daemon={:?} cli={}, restarting",
                            version,
                            BUILD_VERSION
                        );
                        restart_daemon().await;
                        // Re-check if the new daemon is up
                        search_via_daemon = daemon::request::<fn(String, usize, usize)>(
                            &DaemonRequest::Status,
                            false,
                            None,
                        )
                        .await?
                        .is_some();
                    }
                }
                Some(_) => {
                    // Old daemon without version field — restart it
                    tracing::warn!("daemon has no version field, restarting");
                    restart_daemon().await;
                    search_via_daemon = daemon::request::<fn(String, usize, usize)>(
                        &DaemonRequest::Status,
                        false,
                        None,
                    )
                    .await?
                    .is_some();
                }
                None => {}
            }
        }
    } else if daemon::request::<fn(String, usize, usize)>(
        &DaemonRequest::Status,
        !cli.no_watch,
        None,
    )
    .await?
    .is_some()
    {
        search_via_daemon = true;
    }

    // Indexing always uses hash embeddings (instant, ~0.1s).
    // Search uses ONNX model for query embedding (single text, still fast).
    // Background thread enhances the vector store with neural embeddings
    // after results are returned, silently upgrading quality.

    if !search_via_daemon && !cli.all_indices {
        let first_run = matches!(initial_index_state, Some(WorkspaceIndexState::NotIndexed));
        let needs_repair = matches!(initial_index_state, Some(WorkspaceIndexState::Unhealthy));
        if first_run || needs_repair {
            let msg = if needs_repair {
                "Index unhealthy — rebuilding"
            } else if workspace.is_worktree() {
                "First run — computing worktree overlay"
            } else {
                "First run — indexing"
            };
            eprintln!(
                "{} {} {}",
                "⟐".bold(),
                msg.bold(),
                workspace.root.display().to_string().dimmed()
            );

            ensure_no_nested_workspaces(&workspace.root)?;

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
                    watch_enabled: false,
                    skip_gitignore: false,
                    index_generation: 0,
                });

            if meta.skip_gitignore != cli.skip_gitignore {
                meta.skip_gitignore = cli.skip_gitignore;
                let _ = workspace.write_metadata(&meta);
            }

            let hash_model = crate::embedding::create_hash_model();
            let _summary = index_workspace(&workspace, hash_model.as_ref())?;
        }
        // Skip re-indexing for already-indexed workspaces.
        // The daemon watcher handles incremental updates. Re-scanning
        // 92K files (Merkle diff) takes ~2s on the Linux kernel — too
        // slow for every query. Users can `ig --add .` to force re-index.
    }

    if cli.wait_for_enhancement && !cli.all_indices {
        loop {
            let ws_map = crate::workspace::list_workspaces().unwrap_or_default();
            if let Some(status) = ws_map.iter().find(|ws| ws.id == workspace.id) {
                if status.enhancing_stalled {
                    break;
                }
                if !status.enhancing_in_progress {
                    break;
                }

                if std::io::stderr().is_terminal() {
                    let progress_str = if let Some(count) = status.enhancing_progress_count {
                        let pct = if status.chunk_count > 0 {
                            (count as f64 / status.chunk_count as f64 * 100.0).min(100.0) as u64
                        } else {
                            100
                        };
                        format!(" ({} / {} chunks, ~{}%)", count, status.chunk_count, pct)
                    } else {
                        String::new()
                    };
                    eprint!(
                        "\r\x1b[K  waiting for background neural enhancement{}...",
                        progress_str
                    );
                }
            } else {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        if std::io::stderr().is_terminal()
            && let Ok(ws_map) = crate::workspace::list_workspaces()
            && let Some(status) = ws_map.iter().find(|ws| ws.id == workspace.id)
        {
            if status.enhancing_stalled {
                eprintln!("\r\x1b[K  ⚠ neural enhancement stalled");
            } else {
                eprintln!("\r\x1b[K  ✓ neural enhancement complete");
            }
        }
    }

    if cli.skip_gitignore && !cli.all_indices && (!cli.regex || cli.literal) {
        #[allow(clippy::collapsible_if)]
        if let Ok(Some(mut meta)) = workspace.read_metadata() {
            if !meta.skip_gitignore {
                tracing::info!(
                    "Re-indexing workspace to include gitignore entities as requested..."
                );
                meta.skip_gitignore = true;
                let _ = workspace.write_metadata(&meta);
                if search_via_daemon {
                    let req = crate::protocol::DaemonRequest::Index {
                        path: workspace.root.clone(),
                        skip_gitignore: true,
                        watch: false,
                    };
                    let _ =
                        crate::daemon::request::<fn(String, usize, usize)>(&req, false, None).await;
                } else {
                    let model = crate::embedding::create_model(cli.hash);
                    let _ = crate::indexer::index_workspace(&workspace, model.as_ref());
                }
            }
        }
    }

    let search_model: Option<Box<dyn crate::embedding::EmbeddingModel>> =
        if !search_via_daemon && !cli.regex && !cli.literal {
            Some(create_model(cli.hash))
        } else {
            None
        };

    let hits = if cli.literal {
        let request = DaemonRequest::LiteralSearch {
            path: query_path_opt.clone(),
            query: query.to_string(),
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_path: scope_path.clone(),
            scope_is_file,
            skip_gitignore: cli.skip_gitignore,
        };

        if search_via_daemon {
            match daemon::request::<fn(String, usize, usize)>(&request, false, None).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => {
                    tracing::warn!(
                        "daemon literal search failed ({message}), falling back to local"
                    );
                    let options = SearchOptions {
                        limit: backend_limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                        skip_gitignore: cli.skip_gitignore,
                        progress_tx: None,
                    };
                    let mut all_hits = Vec::new();
                    let workspaces = if cli.all_indices {
                        list_workspaces()
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|w| w.last_indexed_at_unix.is_some())
                            .filter_map(|w| Workspace::resolve(&w.root).ok())
                            .collect()
                    } else {
                        vec![workspace.clone()]
                    };
                    for ws in workspaces {
                        match literal_search(&ws, query, &options) {
                            Ok(mut hits) => {
                                if cli.all_indices {
                                    for hit in &mut hits {
                                        hit.file_path = ws.root.join(&hit.file_path);
                                    }
                                }
                                all_hits.append(&mut hits);
                            }
                            Err(err) => tracing::warn!(
                                "literal_search failed for {}: {err:#}",
                                ws.root.display()
                            ),
                        }
                    }
                    all_hits
                }
                _ => vec![],
            }
        } else {
            let mut all_hits = Vec::new();
            let workspaces = if cli.all_indices {
                list_workspaces()?
                    .into_iter()
                    .filter(|w| w.last_indexed_at_unix.is_some())
                    .filter_map(|w| Workspace::resolve(&w.root).ok())
                    .collect()
            } else {
                vec![workspace.clone()]
            };
            let options = SearchOptions {
                limit: backend_limit,
                context: cli.context,
                type_filter: cli.type_filter.clone(),
                include_globs: cli.include.clone(),
                exclude_globs: cli.exclude.clone(),
                scope_filter: scope_filter.clone(),
                skip_gitignore: cli.skip_gitignore,
                progress_tx: None,
            };
            for ws in workspaces {
                match literal_search(&ws, query, &options) {
                    Ok(mut hits) => {
                        if cli.all_indices {
                            for hit in &mut hits {
                                hit.file_path = ws.root.join(&hit.file_path);
                            }
                        }
                        all_hits.append(&mut hits);
                    }
                    Err(err) => {
                        tracing::warn!("literal_search failed for {}: {err:#}", ws.root.display())
                    }
                }
            }
            if let Some(l) = backend_limit {
                all_hits.truncate(l);
            }
            all_hits
        }
    } else if cli.regex {
        let request = DaemonRequest::RegexSearch {
            path: query_path_opt.clone(),
            pattern: query.to_string(),
            limit: backend_limit,
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_path: scope_path.clone(),
            scope_is_file,
            skip_gitignore: cli.skip_gitignore,
        };

        if search_via_daemon {
            match daemon::request::<fn(String, usize, usize)>(&request, false, None).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                other => {
                    tracing::warn!("daemon regex search returned unexpected response: {other:?}");
                    vec![]
                }
            }
        } else {
            let mut all_hits = Vec::new();
            let workspaces = if cli.all_indices {
                list_workspaces()?
                    .into_iter()
                    .filter(|w| w.last_indexed_at_unix.is_some())
                    .filter_map(|w| Workspace::resolve(&w.root).ok())
                    .collect()
            } else {
                vec![workspace.clone()]
            };
            for ws in workspaces {
                match regex_search(
                    &ws,
                    query,
                    backend_limit,
                    scope_filter.as_ref(),
                    &cli.include,
                    &cli.exclude,
                    cli.skip_gitignore,
                ) {
                    Ok(mut hits) => {
                        if cli.all_indices {
                            for hit in &mut hits {
                                hit.file_path = ws.root.join(&hit.file_path);
                            }
                        }
                        all_hits.append(&mut hits);
                    }
                    Err(err) => {
                        tracing::warn!("regex_search failed for {}: {err:#}", ws.root.display());
                    }
                }
            }
            if let Some(l) = backend_limit {
                all_hits.truncate(l);
            }
            all_hits
        }
    } else {
        let request = DaemonRequest::Search {
            path: query_path_opt.clone(),
            query: query.to_string(),
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_path: scope_path.clone(),
            scope_is_file,
            skip_gitignore: cli.skip_gitignore,
        };

        if search_via_daemon {
            let show_spinner = std::io::stderr().is_terminal();
            let _t_search = std::time::Instant::now();
            let search_future = daemon::request::<fn(String, usize, usize)>(&request, false, None);

            let daemon_result = if show_spinner {
                let spinner_handle = tokio::spawn(async move {
                    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let mut tick = 0usize;
                    // Wait a short beat before showing spinner (fast queries won't flash it)
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    loop {
                        let frame = spinner[tick % spinner.len()];
                        tick += 1;
                        eprint!("\r\x1b[K  {} searching...", frame);
                        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                    }
                });
                let result = search_future.await;
                spinner_handle.abort();
                eprint!("\r\x1b[K");
                result?
            } else {
                daemon::request::<fn(String, usize, usize)>(&request, false, None).await?
            };

            match daemon_result {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => {
                    // Daemon search failed — fall back to local search instead
                    // of showing "No results." to the user.
                    tracing::warn!("daemon search failed ({message}), falling back to local");
                    let options = SearchOptions {
                        limit: backend_limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                        skip_gitignore: cli.skip_gitignore,
                        progress_tx: None,
                    };
                    local_fallback_search(&workspace, cli.all_indices, query, &options, cli.hash)
                }
                other => {
                    tracing::warn!("daemon search unavailable ({other:?}), falling back to local");
                    let options = SearchOptions {
                        limit: backend_limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                        skip_gitignore: cli.skip_gitignore,
                        progress_tx: None,
                    };
                    local_fallback_search(&workspace, cli.all_indices, query, &options, cli.hash)
                }
            }
        } else {
            let mut all_hits = Vec::new();
            let workspaces = if cli.all_indices {
                list_workspaces()?
                    .into_iter()
                    .filter(|w| w.last_indexed_at_unix.is_some())
                    .filter_map(|w| Workspace::resolve(&w.root).ok())
                    .collect()
            } else {
                vec![workspace.clone()]
            };
            for ws in workspaces {
                let _ = ws.cleanup_stale_legacy_runtime_files();
                if !cli.hash {
                    let _ = maybe_complete_neural_for_small_workspace(&ws);
                }
                let _t_search = std::time::Instant::now();
                match hybrid_search(
                    &ws,
                    query,
                    search_model.as_deref(),
                    &SearchOptions {
                        limit: backend_limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                        skip_gitignore: cli.skip_gitignore,
                        progress_tx: None,
                    },
                ) {
                    Ok(mut hits) => {
                        if cli.all_indices {
                            for hit in &mut hits {
                                hit.file_path = ws.root.join(&hit.file_path);
                            }
                        }
                        all_hits.append(&mut hits);
                    }
                    Err(err) => {
                        tracing::warn!("hybrid_search failed for {}: {err:#}", ws.root.display());
                    }
                }
            }
            all_hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if let Some(l) = backend_limit {
                all_hits.truncate(l);
            }
            all_hits
        }
    };

    render_hits(
        &hits,
        cli.json,
        display_limit,
        cli.first_line_only,
        cli.file_name_only,
        cli.verbose,
    )?;

    // Kick off background neural enhancement if not already done.
    // This runs after results are returned so the user is never blocked.
    // We launch it as a separate hidden CLI process to prevent segmentation faults
    // that occur perfectly cleanly tearing down `onnxruntime` when the main process exits.
    // Skipped in CI/test environments (IVYGREP_NO_AUTOSPAWN=1).
    let no_autospawn = env::var("IVYGREP_NO_AUTOSPAWN").is_ok();
    if !cli.all_indices
        && !cli.hash
        && !cli.regex
        && !no_autospawn
        && workspace.needs_neural_enhancement()
    {
        let _ = workspace.trigger_background_enhancement();
    }

    std::process::exit(0);
}

fn render_hits(
    hits: &[SearchHit],
    json: bool,
    limit: Option<usize>,
    first_line_only: bool,
    file_name_only: bool,
    verbose: bool,
) -> Result<()> {
    let mut grouped = group_hits_by_file(hits, limit);
    if !verbose {
        for file in &mut grouped {
            for hit in &mut file.hits {
                hit.reason.clear();
            }
        }
    }

    if file_name_only {
        if json {
            let files = grouped
                .iter()
                .map(|result| result.file_path.clone())
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&files)?);
        } else if grouped.is_empty() {
            println!("No results.");
        } else {
            for file in grouped {
                println!("{}", file.file_path.to_string_lossy());
            }
        }
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&grouped)?);
        return Ok(());
    }

    if grouped.is_empty() {
        println!("No results.");
        return Ok(());
    }

    for file in grouped {
        println!(
            "{}  {}  {}",
            file.file_path.to_string_lossy().blue().bold(),
            format!("score={:.4}", file.total_score).green(),
            format!("matches={}", file.hit_count).dimmed(),
        );

        for hit in file.hits {
            let source = if hit.sources.is_empty() {
                String::new()
            } else {
                format!(" [{}]", hit.sources.join("+"))
            };
            println!(
                "  {}-{}{} {}",
                hit.start_line.to_string().yellow(),
                hit.end_line.to_string().yellow(),
                source.dimmed(),
                format!("score={:.4}", hit.score).dimmed(),
            );
            if verbose && !hit.reason.is_empty() {
                println!("    {} {}", "reason:".dimmed(), hit.reason.trim());
            }

            let rendered_preview = if first_line_only {
                hit.preview
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string()
            } else {
                hit.preview.trim().to_string()
            };
            for line in rendered_preview.lines() {
                println!("    {}", line);
            }
        }

        println!();
    }

    Ok(())
}

fn print_daemon_response(response: DaemonResponse, json: bool) -> Result<()> {
    match response {
        DaemonResponse::Ack { message } => {
            if json {
                println!("{}", serde_json::json!({"message": message}));
            } else {
                println!("{message}");
            }
            Ok(())
        }
        DaemonResponse::Error { message } => bail!(message),
        DaemonResponse::SearchResults { hits } => {
            render_hits(&hits, json, None, false, false, false)
        }
        DaemonResponse::Status { workspaces, .. } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&workspaces)?);
            } else {
                for ws in &workspaces {
                    println!("{}\t{}", ws.id, ws.root.display());
                }
            }
            Ok(())
        }
        DaemonResponse::SearchProgress { .. } => Ok(()),
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

fn maybe_run_doctor_command() -> Result<bool> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_none_or(|arg| arg != "doctor") {
        return Ok(false);
    }

    let mut fix = false;
    let mut json = false;
    let mut path: Option<PathBuf> = None;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--fix" => fix = true,
            "--json" => json = true,
            "-h" | "--help" => {
                println!("Usage: ig doctor [PATH] [--fix] [--json]");
                println!();
                println!("Inspect the current workspace index and optionally rebuild it.");
                return Ok(true);
            }
            value if value.starts_with('-') => {
                bail!("unknown option for `ig doctor`: {value}");
            }
            value => {
                if path.is_some() {
                    bail!("too many arguments for `ig doctor`");
                }
                path = Some(PathBuf::from(value));
            }
        }
    }

    run_doctor(path.as_deref(), fix, json)?;
    Ok(true)
}

fn run_doctor(path: Option<&Path>, fix: bool, json: bool) -> Result<()> {
    let target = match path {
        Some(path) => path.to_path_buf(),
        None => env::current_dir()?,
    };
    let workspace = Workspace::resolve(&target)?;
    let report = crate::doctor::inspect_and_maybe_fix(&workspace, fix)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("Workspace: {}", report.workspace_root.display());
    println!("State: {:?}", report.state);
    println!(
        "Chunks: {}  Files: {}",
        report.chunk_count, report.file_count
    );

    for finding in report.findings {
        println!("- {finding}");
    }

    if fix && report.repaired {
        println!("Repair complete.");
    }

    Ok(())
}

fn maybe_run_legacy_mcp_stdio() -> Result<bool> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(false);
    }

    if args.len() == 2 && args[0] == "mcp" && args[1] == "serve" {
        mcp::serve_stdio()?;
        return Ok(true);
    }

    if args.first().is_some_and(|arg| arg == "mcp") {
        bail!("usage: ig --mcp");
    }

    Ok(false)
}

/// Ask the running daemon to shut down, then spawn a fresh one from the current binary.
async fn restart_daemon() {
    // Send a graceful restart request over the socket.
    // The daemon cleans up its own socket and exits after replying.
    let _ = daemon::request::<fn(String, usize, usize)>(&DaemonRequest::Restart, false, None).await;

    // Give the old daemon a moment to exit
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // If the socket still exists, the old daemon didn't understand
    // Restart (pre-upgrade binary). Remove the socket so the old daemon
    // can't accept new connections, then auto-spawn a new one.
    if crate::ipc::socket_exists() {
        crate::ipc::cleanup_socket();
    }

    // Auto-spawn the new daemon via the standard request path
    let _ = daemon::request::<fn(String, usize, usize)>(&DaemonRequest::Status, true, None).await;
}

/// Run a local hybrid search as a fallback when the daemon is unavailable or broken.
fn local_fallback_search(
    workspace: &Workspace,
    all_indices: bool,
    query: &str,
    options: &SearchOptions,
    use_hash: bool,
) -> Vec<SearchHit> {
    let mut all_hits = Vec::new();
    let workspaces = if all_indices {
        crate::workspace::list_workspaces()
            .unwrap_or_default()
            .into_iter()
            .filter(|w| w.last_indexed_at_unix.is_some())
            .filter_map(|w| Workspace::resolve(&w.root).ok())
            .collect()
    } else {
        vec![workspace.clone()]
    };

    let is_single_word = !query.contains(' ')
        && query
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    let model: Option<Box<dyn crate::embedding::EmbeddingModel>> = if is_single_word {
        None
    } else {
        Some(create_model(use_hash))
    };

    for ws in workspaces {
        if !use_hash {
            let _ = maybe_complete_neural_for_small_workspace(&ws);
        }
        match hybrid_search(&ws, query, model.as_deref(), options) {
            Ok(mut hits) => {
                if all_indices {
                    for hit in &mut hits {
                        hit.file_path = ws.root.join(&hit.file_path);
                    }
                }
                all_hits.append(&mut hits);
            }
            Err(err) => {
                tracing::warn!(
                    "local fallback search also failed for {}: {err:#}",
                    ws.root.display()
                );
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
    all_hits
}

fn ensure_no_nested_workspaces(target_root: &Path) -> Result<()> {
    if let Ok(workspaces) = list_workspaces() {
        let mut conflicts = Vec::new();
        for ws in workspaces {
            if ws.root != target_root && ws.root.starts_with(target_root) {
                conflicts.push(ws.root.clone());
            }
        }
        if !conflicts.is_empty() {
            let conflict_msgs: Vec<String> = conflicts
                .iter()
                .map(|p| format!("ig --rm {}", p.display()))
                .collect();
            let paths_list: Vec<String> = conflicts
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect();
            bail!(
                "Cannot index '{}' because it contains already indexed sub-workspaces:\n{}\n\nYou must remove them first:\n  {}",
                target_root.display(),
                paths_list.join("\n"),
                conflict_msgs.join("\n  ")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;
    use tempfile::tempdir;

    use crate::workspace::WorkspaceMetadata;

    #[test]
    #[serial]
    fn query_autospawn_only_when_watch_is_configured() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();

        assert!(!should_autospawn_daemon_for_query(&workspace, false));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: now,
                last_indexed_at_unix: Some(now),
                watch_enabled: true,
                skip_gitignore: false,
                index_generation: 0,
            })
            .unwrap();

        assert!(should_autospawn_daemon_for_query(&workspace, false));
        assert!(!should_autospawn_daemon_for_query(&workspace, true));
    }
}
