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
use crate::indexer::{index_workspace, remove_workspace_index, workspace_is_indexed};
use crate::mcp;
use crate::protocol::{
    BUILD_VERSION, DaemonRequest, DaemonResponse, SearchHit, group_hits_by_file,
};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, list_workspaces, resolve_workspace_and_scope};

#[derive(Debug, Parser)]
#[command(author, version, about = "Semantic grep that stays local", long_about = None)]
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

    #[arg(long, global = true)]
    pub regex: bool,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(short = 'C', long, default_value_t = 2, global = true)]
    pub context: usize,

    #[arg(long = "type", global = true)]
    pub type_filter: Option<String>,

    #[arg(long, global = true)]
    pub all: bool,

    #[arg(long, value_name = "GLOBS", value_delimiter = ',', global = true)]
    pub include: Vec<String>,

    #[arg(long, value_name = "GLOBS", value_delimiter = ',', global = true)]
    pub exclude: Vec<String>,

    #[arg(short = 'n', long, global = true)]
    pub limit: Option<usize>,

    #[arg(long, global = true)]
    pub no_watch: bool,

    #[arg(long, global = true)]
    pub first_line_only: bool,

    #[arg(long, global = true)]
    pub file_name_only: bool,

    #[arg(long, global = true)]
    pub verbose: bool,

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
        return run_add(path, !cli.no_watch, cli.force, cli.json).await;
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

        let result = {
            let model_res = crate::embedding::create_neural_model_background();
            if let Ok(model) = model_res {
                crate::indexer::enhance_workspace_neural(&workspace, model.as_ref())
            } else {
                Ok(0)
            }
        };

        // Clean up PID file
        let _ = std::fs::remove_file(&pid_path);

        // ONNX clean teardown can sometimes segfault in multithreaded handlers.
        // We'll intentionally skip proper Rust panic runtime teardown and forcefully exit.
        if let Err(e) = result {
            eprintln!("Background enhancement failed: {:?}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
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
        for ws in &workspaces {
            println!("\x1b[1;36m⟐ {}\x1b[0m", ws.root.display());
            println!("  ID:     {}", ws.id);

            // Index timestamp
            match ws.last_indexed_at_unix {
                Some(ts) => {
                    let ago = format_timestamp_ago(ts);
                    println!("  Index:  \x1b[32m✓ indexed\x1b[0m ({})", ago);
                }
                None => {
                    println!("  Index:  \x1b[33m⚠ never indexed\x1b[0m");
                }
            }

            // Daemon/watcher
            if ws.watch_enabled {
                println!("  Watch:  \x1b[32m● watching\x1b[0m");
            } else {
                println!("  Watch:  \x1b[90m○ static\x1b[0m");
            }

            // Chunk stats
            println!(
                "  Files:  {} files, {} chunks",
                ws.file_count, ws.chunk_count
            );

            // Index size
            let size = format_bytes(ws.index_size_bytes);
            println!("  Size:   {}", size);

            // Embedding status
            if ws.enhancing_in_progress {
                let accel = crate::embedding::hardware_acceleration_info();

                let progress_str = if let Some(count) = ws.enhancing_progress_count {
                    let pct = if ws.chunk_count > 0 {
                        (count as f64 / ws.chunk_count as f64 * 100.0).min(100.0) as u64
                    } else {
                        100
                    };
                    format!("({} / {} chunks, ~{}%), ", count, ws.chunk_count, pct)
                } else {
                    String::new()
                };

                println!(
                    "  Search: \x1b[1;33m⟳ enhancing\x1b[0m {progress_str}(computing {} in background...)",
                    accel
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
                    "  Search: \x1b[1;32m★ neural\x1b[0m ({} enhanced, {}, {})",
                    ws.neural_vector_count, pct, accel
                );
            } else if ws.indexing_in_progress {
                println!(
                    "  Search: \x1b[1;33m⟳ indexing\x1b[0m (scanning, parsing, and chunking documents locally...)"
                );
            } else if ws.chunk_count > 0 {
                println!(
                    "  Search: \x1b[33m◆ hash\x1b[0m (fast, run a query to trigger neural upgrade)"
                );
            } else {
                println!("  Search: \x1b[90m○ empty\x1b[0m");
            }

            println!();
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

async fn run_add(path: &Path, watch: bool, force: bool, json: bool) -> Result<()> {
    let workspace = Workspace::resolve(path)?;

    if force {
        let remove_request = DaemonRequest::Remove {
            path: workspace.root.clone(),
        };

        if let Some(response) = daemon::request(&remove_request, false).await? {
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
    };

    if let Some(response) = daemon::request(&request, false).await? {
        return print_daemon_response(response, json);
    }

    let model = create_model(false);
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
    if let Some(response) = daemon::request(&request, false).await? {
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
    let scope_path = scope_filter.as_ref().map(|scope| scope.rel_path.clone());
    let scope_is_file = scope_filter.as_ref().is_some_and(|scope| scope.is_file);

    let query_path_opt = if cli.all {
        None
    } else {
        Some(workspace.root.clone())
    };
    let mut search_via_daemon = false;

    if !cli.all {
        let first_run = !crate::indexer::workspace_is_indexed(&workspace);
        if first_run {
            // Always show progress for first-run, even when the daemon handles it.
            eprintln!(
                "{} {} {}",
                "⟐".bold(),
                "First run — indexing".bold(),
                workspace.root.display().to_string().dimmed()
            );

            let daemon_index_request = DaemonRequest::Index {
                path: workspace.root.clone(),
                watch: !cli.no_watch,
            };

            // Send the index request to the daemon, but show a progress spinner
            // while we wait so the user knows work is happening.
            let ws_id = workspace.id.clone();
            let show_progress = std::io::stderr().is_terminal();

            let response_future = daemon::request(&daemon_index_request, !cli.no_watch);

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
                                cached_msg = format!(
                                    "{} files, {} chunks indexed...",
                                    status.file_count, status.chunk_count
                                );
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
            match daemon::request(&DaemonRequest::Status, false).await? {
                Some(DaemonResponse::Status { version, .. }) => {
                    if version.as_deref() == Some(BUILD_VERSION) {
                        search_via_daemon = true;
                    } else {
                        tracing::warn!(
                            "daemon version mismatch: daemon={:?} cli={}, restarting",
                            version,
                            BUILD_VERSION
                        );
                        restart_daemon().await;
                        // Re-check if the new daemon is up
                        search_via_daemon = daemon::request(&DaemonRequest::Status, false)
                            .await?
                            .is_some();
                    }
                }
                Some(_) => {
                    // Old daemon without version field — restart it
                    tracing::warn!("daemon has no version field, restarting");
                    restart_daemon().await;
                    search_via_daemon = daemon::request(&DaemonRequest::Status, false)
                        .await?
                        .is_some();
                }
                None => {}
            }
        }
    } else {
        if daemon::request(&DaemonRequest::Status, !cli.no_watch)
            .await?
            .is_some()
        {
            search_via_daemon = true;
        }
    }

    // Indexing always uses hash embeddings (instant, ~0.1s).
    // Search uses ONNX model for query embedding (single text, still fast).
    // Background thread enhances the vector store with neural embeddings
    // after results are returned, silently upgrading quality.

    if !search_via_daemon && !cli.all {
        let first_run = !workspace_is_indexed(&workspace);
        if first_run {
            eprintln!(
                "{} {} {}",
                "⟐".bold(),
                "First run — indexing".bold(),
                workspace.root.display().to_string().dimmed()
            );
            let hash_model = crate::embedding::create_hash_model();
            let _summary = index_workspace(&workspace, hash_model.as_ref())?;
        }
        // Skip re-indexing for already-indexed workspaces.
        // The daemon watcher handles incremental updates. Re-scanning
        // 92K files (Merkle diff) takes ~2s on the Linux kernel — too
        // slow for every query. Users can `ig --add .` to force re-index.
    }

    if cli.wait_for_enhancement && !cli.all {
        loop {
            let ws_map = crate::workspace::list_workspaces().unwrap_or_default();
            if let Some(status) = ws_map.iter().find(|ws| ws.id == workspace.id) {
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
        if std::io::stderr().is_terminal() {
            eprintln!("\r\x1b[K  ✓ neural enhancement complete");
        }
    }

    let is_identifier = !query.contains(' ') 
        && query.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');

    let search_model: Option<Box<dyn crate::embedding::EmbeddingModel>> =
        if !search_via_daemon && !cli.regex && !is_identifier {
            Some(create_model(cli.hash))
        } else {
            None
        };

    let hits = if cli.regex {
        let request = DaemonRequest::RegexSearch {
            path: query_path_opt.clone(),
            pattern: query.to_string(),
            limit: cli.limit,
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_path: scope_path.clone(),
            scope_is_file,
        };

        if search_via_daemon {
            match daemon::request(&request, false).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                other => {
                    tracing::warn!("daemon regex search returned unexpected response: {other:?}");
                    vec![]
                }
            }
        } else {
            let mut all_hits = Vec::new();
            let workspaces = if cli.all {
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
                    cli.limit,
                    scope_filter.as_ref(),
                    &cli.include,
                    &cli.exclude,
                ) {
                    Ok(mut hits) => all_hits.append(&mut hits),
                    Err(err) => {
                        tracing::warn!(
                            "regex_search failed for {}: {err:#}",
                            ws.root.display()
                        );
                    }
                }
            }
            if let Some(l) = cli.limit {
                all_hits.truncate(l);
            }
            all_hits
        }
    } else {
        let request = DaemonRequest::Search {
            path: query_path_opt.clone(),
            query: query.to_string(),
            limit: cli.limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_path: scope_path.clone(),
            scope_is_file,
        };

        let all_hits = if search_via_daemon {
            let show_spinner = std::io::stderr().is_terminal();
            let _t_search = std::time::Instant::now();
            let search_future = daemon::request(&request, false);

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
                daemon::request(&request, false).await?
            };

            match daemon_result {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => {
                    // Daemon search failed — fall back to local search instead
                    // of showing "No results." to the user.
                    tracing::warn!("daemon search failed ({message}), falling back to local");
                    let options = SearchOptions {
                        limit: cli.limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                    };
                    local_fallback_search(&workspace, query, &options, cli.hash)
                }
                other => {
                    tracing::warn!("daemon search unavailable ({other:?}), falling back to local");
                    let options = SearchOptions {
                        limit: cli.limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                    };
                    local_fallback_search(&workspace, query, &options, cli.hash)
                }
            }
        } else {
            let mut all_hits = Vec::new();
            let workspaces = if cli.all {
                list_workspaces()?
                    .into_iter()
                    .filter(|w| w.last_indexed_at_unix.is_some())
                    .filter_map(|w| Workspace::resolve(&w.root).ok())
                    .collect()
            } else {
                vec![workspace.clone()]
            };
            for ws in workspaces {
                let _t_search = std::time::Instant::now();
                match hybrid_search(
                    &ws,
                    query,
                    search_model.as_deref(),
                    &SearchOptions {
                        limit: cli.limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                    },
                ) {
                    Ok(mut hits) => all_hits.append(&mut hits),
                    Err(err) => {
                        tracing::warn!(
                            "hybrid_search failed for {}: {err:#}",
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
            if let Some(l) = cli.limit {
                all_hits.truncate(l);
            }
            all_hits
        };
        all_hits
    };

    render_hits(
        &hits,
        cli.json,
        cli.limit,
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
    if !cli.all
        && !cli.hash
        && !cli.regex
        && !no_autospawn
        && !workspace.vector_neural_path().exists()
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
        if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            eprintln!(
                "{}",
                "hint: try `ig --add . --force` to rebuild index, or check ~/.local/share/ivygrep/daemon.log"
                    .dimmed()
            );
        }
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
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
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
    let _ = daemon::request(&DaemonRequest::Restart, false).await;

    // Give the old daemon a moment to exit
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // If the socket still exists, the old daemon didn't understand
    // Restart (pre-upgrade binary). Remove the socket so the old daemon
    // can't accept new connections, then auto-spawn a new one.
    if let Ok(sp) = config::socket_path() {
        if sp.exists() {
            let _ = std::fs::remove_file(&sp);
        }
    }

    // Auto-spawn the new daemon via the standard request path
    let _ = daemon::request(&DaemonRequest::Status, true).await;
}

/// Run a local hybrid search as a fallback when the daemon is unavailable or broken.
fn local_fallback_search(
    workspace: &Workspace,
    query: &str,
    options: &SearchOptions,
    use_hash: bool,
) -> Vec<SearchHit> {
    let model: Option<Box<dyn crate::embedding::EmbeddingModel>> = {
        let is_identifier = !query.contains(' ')
            && query
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if is_identifier {
            None
        } else {
            Some(create_model(use_hash))
        }
    };

    match hybrid_search(workspace, query, model.as_deref(), options) {
        Ok(hits) => hits,
        Err(err) => {
            tracing::warn!(
                "local fallback search also failed for {}: {err:#}",
                workspace.root.display()
            );
            vec![]
        }
    }
}
