use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use colored::Colorize;
use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

use crate::config;
use crate::daemon;
use crate::embedding::create_model;
use crate::indexer::{index_workspace, remove_workspace_index, workspace_is_indexed};
use crate::jobs::{self, JobKind, JobUpdate};
use crate::mcp;
use crate::protocol::{
    BUILD_VERSION, DaemonRequest, DaemonResponse, SearchHit, group_hits_by_file,
};
use crate::regex_search::regex_search;
use crate::search::{
    SearchOptions, hybrid_search, literal_search, validate_forced_neural_workspaces,
};
use crate::workspace::{
    Workspace, WorkspaceIndexState, list_workspace_roots, list_workspaces,
    resolve_workspace_and_scope,
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "ivygrep",
    version,
    about = "Semantic grep that stays local",
    long_about = None,
    after_help = "Examples:\n  ig \"where is authentication checked?\"\n  ig context \"fix refresh-token races\" --budget 8000\n  ig agent install claude\n  ig --literal validate_token src/\n  ig --symbol UserService\n  ig --add . --wait-for-enhancement\n  ig --status\n  ig --web"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,

    /// Natural-language question, keyword, identifier, or exact search text.
    #[arg(value_name = "QUERY", required = false)]
    pub query: Option<String>,

    /// Workspace, subdirectory, or file to search. Defaults to current directory.
    #[arg(value_name = "PATH", required = false)]
    pub query_path: Option<PathBuf>,

    /// Index PATH and register it for incremental updates. Defaults to current directory.
    #[arg(long = "add", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub add_path: Option<PathBuf>,

    /// Remove PATH's saved index. Defaults to current directory.
    #[arg(long = "rm", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub rm_path: Option<PathBuf>,

    /// Show tracked workspaces, index health, vector coverage, and disk usage.
    #[arg(long, default_value_t = false)]
    pub status: bool,

    /// Diagnose index, daemon, watcher, and model health for PATH.
    #[arg(long, default_value_t = false)]
    pub doctor: bool,

    /// Repair a broken or stale index (used with --doctor).
    #[arg(long, default_value_t = false, requires = "doctor")]
    pub fix: bool,

    /// Perform full cross-store integrity scans (used with --doctor).
    #[arg(long, default_value_t = false, requires = "doctor")]
    pub deep: bool,

    /// Run long-lived indexing, search, MCP, and web services.
    #[arg(long, default_value_t = false)]
    pub daemon: bool,

    /// Open the daemon-backed local web UI.
    #[arg(long, default_value_t = false)]
    pub web: bool,

    /// Host for --web. Non-loopback access requires the token in the printed URL.
    #[arg(long, default_value = "127.0.0.1", requires = "web")]
    pub host: String,

    /// Port for --web. Use 0 to let the OS choose a free port.
    #[arg(long, default_value_t = 4747, requires = "web")]
    pub port: u16,

    /// Serve Model Context Protocol tools over stdio.
    #[arg(long, default_value_t = false)]
    pub mcp: bool,

    /// Wait for requested vector enhancement and report worker failures.
    #[arg(long, default_value_t = false)]
    pub wait_for_enhancement: bool,

    /// Rebuild workspace index from scratch when used with --add.
    #[arg(short, long)]
    pub force: bool,

    /// Launch the interactive terminal UI.
    #[arg(long = "interactive", visible_alias = "ui")]
    pub ui: bool,

    /// Fast exact-match search backed by the index. Deterministic results,
    /// orders of magnitude faster than grep/rg for indexed repos.
    #[arg(long, short = 'l')]
    pub literal: bool,

    /// Legacy regex mode. Uses an index prefilter when possible, otherwise walks files.
    #[arg(long, hide = true)]
    pub regex: bool,

    /// Find exact symbol definitions.
    #[arg(long, conflicts_with_all = ["refs", "callers", "literal", "regex"])]
    pub symbol: bool,

    /// Find exact symbol references.
    #[arg(long, conflicts_with_all = ["symbol", "callers", "literal", "regex"])]
    pub refs: bool,

    /// Find functions or methods that call the named symbol.
    #[arg(long, conflicts_with_all = ["symbol", "refs", "literal", "regex"])]
    pub callers: bool,

    /// Emit machine-readable JSON without ANSI styling.
    #[arg(long)]
    pub json: bool,

    /// Lines before and after the focused match to include in each snippet.
    /// This changes output size, not retrieval ranking.
    #[arg(short = 'C', long, value_name = "LINES", default_value_t = 2)]
    pub context: usize,

    #[arg(
        long = "type",
        help = "Filter by language name, extension, or alias (e.g. rust, rs, py, python, c++, bash, md)"
    )]
    pub type_filter: Option<String>,

    /// Search every tracked workspace instead of resolving one PATH.
    #[arg(long, alias = "all")]
    pub all_indices: bool,

    /// Include only comma-separated path globs. May be repeated.
    #[arg(long, value_name = "GLOBS", value_delimiter = ',')]
    pub include: Vec<String>,

    /// Exclude comma-separated path globs. May be repeated.
    #[arg(long, value_name = "GLOBS", value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Retrieval breadth and maximum ranked result files, not a token, line, or
    /// confidence limit. Larger values search deeper and may improve recall.
    #[arg(short = 'n', long, value_name = "FILES")]
    pub limit: Option<usize>,

    /// Use maximum candidate budgets and return all surviving results. This can
    /// produce large, slower responses.
    #[arg(long, conflicts_with = "limit")]
    pub no_limit: bool,

    /// Index once without starting or registering a filesystem watcher.
    #[arg(long)]
    pub no_watch: bool,

    /// Return only the first non-empty preview line. Ranking is unchanged.
    #[arg(long)]
    pub first_line_only: bool,

    /// Return only file paths. Without --limit, this also uses maximum candidate
    /// budgets; combine with --limit for a bounded path list.
    #[arg(long)]
    pub file_name_only: bool,

    /// Include source-signal explanations and detailed progress diagnostics.
    #[arg(long)]
    pub verbose: bool,

    /// Include files excluded by .gitignore and other standard ignore files.
    #[arg(long)]
    pub skip_gitignore: bool,

    /// Use lightweight hash-based embeddings instead of the default neural
    /// model. Faster startup, no model download, lower quality.
    #[arg(long)]
    pub hash: bool,

    /// Use BM25/path/signature retrieval without vector search.
    #[arg(long, conflicts_with_all = ["hash", "literal", "regex"])]
    pub lexical_only: bool,

    /// Force neural retrieval for benchmarking and diagnostics.
    #[arg(
        long,
        hide = true,
        conflicts_with_all = ["hash", "lexical_only", "literal", "regex"]
    )]
    pub force_neural: bool,

    #[arg(long, hide = true, value_name = "PATH")]
    pub enhance_internal: Option<PathBuf>,

    #[arg(long, hide = true, value_name = "PATH")]
    pub enhance_hash_internal: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Build one task-aware, token-budgeted context bundle.
    Context(ContextArgs),
    /// Install and verify ivygrep for coding agents.
    Agent(AgentArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ContextArgs {
    /// Development task or question to gather context for.
    #[arg(value_name = "TASK")]
    pub task: String,

    /// Workspace or subdirectory. Defaults to current directory.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Maximum model-independent estimated tokens in gathered evidence.
    #[arg(long, value_name = "TOKENS", default_value_t = 8000)]
    pub budget: usize,

    /// Use BM25/path/signature retrieval without vector search.
    #[arg(long, conflicts_with = "hash")]
    pub lexical_only: bool,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Filter by language name, extension, or alias.
    #[arg(long = "type")]
    pub type_filter: Option<String>,

    /// Include only comma-separated path globs. May be repeated.
    #[arg(long, value_name = "GLOBS", value_delimiter = ',')]
    pub include: Vec<String>,

    /// Exclude comma-separated path globs. May be repeated.
    #[arg(long, value_name = "GLOBS", value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Index once without starting a filesystem watcher.
    #[arg(long)]
    pub no_watch: bool,

    /// Include detailed progress diagnostics.
    #[arg(long)]
    pub verbose: bool,

    /// Include files excluded by .gitignore.
    #[arg(long)]
    pub skip_gitignore: bool,

    /// Use lightweight hash-based embeddings.
    #[arg(long, conflicts_with = "lexical_only")]
    pub hash: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AgentCommand {
    /// Configure one client, verify MCP, and run a real search.
    Install {
        /// Coding agent to configure.
        #[arg(value_enum)]
        client: crate::agent::AgentClient,
    },
    /// Detect clients and verify configuration, MCP, and search.
    Doctor,
}

fn apply_context_args(cli: &mut Cli, args: &ContextArgs) {
    cli.query = Some(args.task.clone());
    cli.query_path = args.path.clone();
    cli.lexical_only |= args.lexical_only;
    cli.json |= args.json;
    cli.type_filter = args.type_filter.clone().or(cli.type_filter.take());
    cli.include.extend(args.include.iter().cloned());
    cli.exclude.extend(args.exclude.iter().cloned());
    cli.no_watch |= args.no_watch;
    cli.verbose |= args.verbose;
    cli.skip_gitignore |= args.skip_gitignore;
    cli.hash |= args.hash;
}

pub async fn run() -> Result<()> {
    init_tracing();
    config::ensure_app_dirs()?;

    if maybe_run_legacy_mcp_stdio()? {
        return Ok(());
    }

    let mut cli = Cli::parse();
    let mut agent_command = None;
    let context_args = match cli.command.take() {
        Some(CliCommand::Context(args)) => {
            apply_context_args(&mut cli, &args);
            Some(args)
        }
        Some(CliCommand::Agent(args)) => {
            agent_command = Some(args.command);
            None
        }
        None => None,
    };

    // Resolve --type aliases: "rs" → "rust", "py" → "python", "c++" → "cpp", etc.
    if let Some(ref tf) = cli.type_filter
        && let Some(canonical) = crate::chunking::resolve_type_alias(tf)
    {
        cli.type_filter = Some(canonical.to_string());
    }
    let action_count = [
        cli.add_path.is_some(),
        cli.rm_path.is_some(),
        cli.status,
        cli.doctor,
        cli.daemon,
        cli.web,
        cli.mcp,
        context_args.is_some(),
        agent_command.is_some(),
    ]
    .iter()
    .filter(|flag| **flag)
    .count();

    if action_count > 1 {
        bail!(
            "use only one action at a time: context, agent, --add, --rm, --status, --doctor, --daemon, --web, or --mcp"
        );
    }

    if let Some(command) = agent_command {
        match command {
            AgentCommand::Install { client } => crate::agent::install(client)?,
            AgentCommand::Doctor => crate::agent::doctor()?,
        }
        return Ok(());
    }

    if context_args.is_some()
        && (cli.all_indices
            || cli.literal
            || cli.regex
            || cli.symbol
            || cli.refs
            || cli.callers
            || cli.ui
            || cli.no_limit
            || cli.limit.is_some()
            || cli.first_line_only
            || cli.file_name_only)
    {
        bail!(
            "context uses its task and token budget directly; do not combine it with search modes, --all-indices, --limit, --no-limit, --interactive, or compact-output flags"
        );
    }
    if let Some(args) = &context_args
        && !(256..=131_072).contains(&args.budget)
    {
        bail!("context --budget must be between 256 and 131072 tokens");
    }

    if cli.daemon {
        daemon::run_daemon().await?;
        return Ok(());
    }

    if cli.web {
        let selected_path = match &cli.query_path {
            Some(path) => path.clone(),
            None => env::current_dir()?,
        };
        let selected_path = selected_path.canonicalize().unwrap_or(selected_path);
        let response = daemon::request::<fn(String, usize, usize)>(
            &DaemonRequest::ServeWeb {
                host: cli.host.clone(),
                port: cli.port,
                initial_query: cli.query.clone(),
                initial_path: Some(selected_path),
            },
            true,
            None,
        )
        .await?;
        let url = match response {
            Some(DaemonResponse::WebStarted { url }) => url,
            Some(DaemonResponse::Error { message }) => {
                bail!("could not start ivygrep web server: {message}");
            }
            _ => bail!("could not start ivygrep web server"),
        };
        println!("ivygrep web listening at {url}");
        std::io::stdout().flush().ok();
        open_browser(&url);
        return Ok(());
    }

    if cli.mcp {
        mcp::serve_stdio()?;
        return Ok(());
    }

    if cli.status {
        return run_status(cli.json).await;
    }

    if cli.doctor {
        let path = cli
            .query_path
            .as_deref()
            .or_else(|| cli.query.as_deref().map(Path::new));
        run_doctor(path, cli.fix, cli.deep, cli.json)?;
        return Ok(());
    }

    if let Some(path) = &cli.add_path {
        return run_add(
            path,
            !cli.no_watch,
            cli.force,
            cli.skip_gitignore,
            cli.json,
            cli.hash,
            cli.wait_for_enhancement,
        )
        .await;
    }

    if let Some(path) = &cli.rm_path {
        return run_remove(path, cli.json).await;
    }

    if let Some(path) = &cli.enhance_hash_internal {
        let workspace = Workspace::resolve(path)?;
        workspace.ensure_dirs()?;
        let hash_model = crate::embedding::create_hash_model();
        let result = crate::indexer::enhance_workspace_hash(&workspace, hash_model.as_ref());
        if let Err(error) = &result {
            let _ = std::fs::write(
                workspace.index_dir.join(".enhancing.error"),
                format!("Hash enhancement error: {error:#}"),
            );
            let _ = jobs::finish_job(
                &workspace,
                JobKind::Enhancement,
                "hash-failed",
                Some(error.to_string()),
            );
        } else {
            let _ = std::fs::remove_file(workspace.index_dir.join(".enhancing.error"));
            let _ = jobs::finish_job(&workspace, JobKind::Enhancement, "hash-completed", None);
        }
        let _ = std::fs::remove_file(workspace.enhancing_pid_path());
        let _ = std::fs::remove_file(workspace.enhancing_progress_path());
        let _ = std::fs::remove_file(workspace.enhancing_phase_path());
        let _ = std::fs::remove_file(workspace.enhancing_paused_path());
        return result.map(|_| ());
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
                let stage = std::fs::read_to_string(heartbeat_workspace.enhancing_phase_path())
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
                if let Some(stage) = stage {
                    update.details.insert("stage".to_string(), stage);
                }
                if let Some(reason) = paused_reason {
                    update.details.insert("paused_reason".to_string(), reason);
                }
                let _ = jobs::heartbeat_job(&heartbeat_workspace, JobKind::Enhancement, update);
            }
        });

        let result = (|| {
            let hash_model = crate::embedding::create_hash_model();
            crate::indexer::enhance_workspace_hash(&workspace, hash_model.as_ref())?;

            if workspace.has_overlay() || workspace.base_ref_path().exists() {
                return Ok(0);
            }

            let model = crate::embedding::create_neural_model_background()?;
            crate::indexer::enhance_workspace_neural(&workspace, model.as_ref())
        })();
        if let Err(e) = &result {
            let _ = std::fs::write(
                workspace.index_dir.join(".enhancing.error"),
                format!("Enhancement error: {:?}", e),
            );
        } else {
            let _ = std::fs::remove_file(workspace.index_dir.join(".enhancing.error"));
        }

        // Neural model teardown can fail in multithreaded enhancement handlers.
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
            let _ = std::fs::remove_file(workspace.enhancing_phase_path());
            eprintln!("Background enhancement failed: {:?}", e);
            std::process::exit(1);
        }
        stop_heartbeat.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = jobs::finish_job(&workspace, JobKind::Enhancement, "completed", None);
        let _ = std::fs::remove_file(&pid_path);
        let _ = std::fs::remove_file(workspace.enhancing_progress_path());
        let _ = std::fs::remove_file(workspace.enhancing_phase_path());
        std::process::exit(0);
    }

    if cli.ui {
        return crate::tui::run_tui(cli).await;
    }

    run_query(cli, context_args).await
}

fn open_browser(url: &str) {
    if std::env::var_os("IVYGREP_NO_BROWSER").is_some() {
        return;
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };

    let _ = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
            "\n  Run {} in a project to auto-index, or {} to register one.",
            "ig \"query\"".bold(),
            "ig --add .".bold()
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
                println!(
                    "{}",
                    format!("⟐ {}", base_root.display()).bright_cyan().bold()
                );
                println!(
                    "  {}\n",
                    "(Base repository not directly tracked by ivygrep)".bright_black()
                );
            }

            for ws in wss {
                let is_overlay = ws.base_repo_root.is_some();
                let prefix = if is_overlay { "  " } else { "" };

                if is_overlay {
                    println!(
                        "  {}",
                        format!("↳ Overlay: {}", ws.root.display())
                            .bright_magenta()
                            .bold()
                    );
                } else {
                    println!(
                        "{}",
                        format!("⟐ {}", ws.root.display()).bright_cyan().bold()
                    );
                }

                println!("{prefix}  ID:     {}", ws.id);

                // Index timestamp
                match ws.last_indexed_at_unix {
                    Some(ts) => {
                        let ago = format_timestamp_ago(ts);
                        println!("{prefix}  Index:  {} ({ago})", "✓ indexed".green());
                    }
                    None if ws.indexing_in_progress => {
                        println!("{prefix}  Index:  {}", "⟳ initial indexing".yellow().bold());
                    }
                    None => {
                        println!("{prefix}  Index:  {}", "⚠ never indexed".yellow());
                    }
                }

                // Daemon/watcher
                if ws.watch_enabled && ws.watcher_alive {
                    println!("{prefix}  Watch:  {}", "● configured + live".green());
                } else if ws.watch_enabled {
                    println!(
                        "{prefix}  Watch:  {}",
                        "◐ configured, watcher offline".yellow().bold()
                    );
                } else {
                    println!("{prefix}  Watch:  {}", "○ static".bright_black());
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
                println!(
                    "{prefix}          chunks {}, graph {}, sqlite aux {}, lexical {}, hash {}, neural {}",
                    format_bytes(ws.index_components.stored_chunks_bytes),
                    format_bytes(ws.index_components.graph_bytes),
                    format_bytes(ws.index_components.sqlite_auxiliary_bytes),
                    format_bytes(ws.index_components.lexical_bytes),
                    format_bytes(ws.index_components.hash_vectors_bytes),
                    format_bytes(ws.index_components.neural_vectors_bytes),
                );
                println!(
                    "{prefix}          compaction {} ({:.1}% free, format v{}/{})",
                    if ws.compaction.healthy {
                        "healthy"
                    } else {
                        "attention"
                    },
                    ws.compaction.sqlite_free_percent,
                    ws.compaction.format_version,
                    ws.compaction.current_format_version,
                );

                // Embedding status
                if ws.enhancing_in_progress {
                    let phase = ws.enhancing_phase.as_deref().unwrap_or("background");
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
                            "{prefix}  Search: {} {progress_str}(Paused: {reason})",
                            format!("⟳ enhancing {phase} [PAUSED]").yellow().bold()
                        );
                    } else {
                        println!(
                            "{prefix}  Search: {} {progress_str}(computing local vectors in background...)",
                            format!("⟳ enhancing {phase}").yellow().bold()
                        );
                    }
                } else if ws.enhancing_stalled {
                    println!(
                        "{prefix}  Search: {} (run `ig --doctor` or retry a query)",
                        "⚠ stalled neural upgrade".red().bold()
                    );
                } else if ws.has_neural_vectors {
                    let pct = format!("{:.0}%", ws.neural_coverage_percent);
                    let backend = ws
                        .neural_backend
                        .as_deref()
                        .unwrap_or("local backend unrecorded");
                    let profile = ws.neural_profile.as_deref().unwrap_or("general");
                    println!(
                        "{prefix}  Search: {} ({} / {} vectors, {pct}, {profile} {}d, last enhanced with {backend})",
                        "★ neural".green().bold(),
                        ws.neural_vector_count,
                        ws.vector_key_count,
                        ws.neural_dimensions
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
                    println!(
                        "{prefix}  Search: {} ({detail})",
                        "⟳ indexing".yellow().bold()
                    );
                } else if ws.indexing_stalled {
                    println!(
                        "{prefix}  Search: {} (run `ig --doctor --fix`)",
                        "⚠ stalled indexing".red().bold()
                    );
                } else if is_overlay {
                    if ws.chunk_count > 0 {
                        println!(
                            "{prefix}  Search: {} (+ base neural/hash delegation)",
                            "◆ hash".yellow()
                        );
                    } else {
                        println!(
                            "{prefix}  Search: {} (fully delegated to base)",
                            "⟐ overlay".magenta()
                        );
                    }
                } else if let Some(err) = &ws.enhancing_error {
                    let err_line = err.lines().next().unwrap_or("unknown error");
                    if err_line.contains("neural feature not compiled") {
                        // Expected for static/musl builds — not an error
                        println!(
                            "{prefix}  Search: {} (neural not available in this build)",
                            "◆ hash".yellow()
                        );
                    } else {
                        // Real neural-model failure
                        println!(
                            "{prefix}  Search: {} (run `ig query` to retry, or check .enhancing.error)",
                            "⚠️ neural upgrade failed".red().bold()
                        );
                        println!("{prefix}          Error: {}", err_line.red());
                    }
                } else if ws.chunk_count > 0 {
                    println!(
                        "{prefix}  Search: {} (fast, run a query to trigger neural upgrade)",
                        "◆ hash".yellow()
                    );
                } else {
                    println!("{prefix}  Search: {}", "○ empty".bright_black());
                }
                let reranker_model = ws
                    .reranker_model
                    .as_deref()
                    .map(|model| format!(" {model}"))
                    .unwrap_or_default();
                println!(
                    "{prefix}  Rank:   {}{} (bounded top-{})",
                    ws.reranker_mode, reranker_model, ws.reranker_candidate_limit
                );
                if let Some(error) = &ws.reranker_error {
                    println!("{prefix}          Reranker warning: {error}");
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
            "{}",
            format!(
                "{} workspace(s), {} files, {} chunks, {} on disk, {}/{} neural",
                workspaces.len(),
                total_files,
                total_chunks,
                format_bytes(total_size),
                neural_count,
                workspaces.len(),
            )
            .bright_black()
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
        .is_some_and(|meta| meta.last_indexed_at_unix.is_some())
}

async fn run_add(
    path: &Path,
    watch: bool,
    force: bool,
    skip_gitignore: bool,
    json: bool,
    hash: bool,
    wait_for_enhancement: bool,
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
        print_daemon_response(response, json)?;
        if wait_for_enhancement {
            trigger_workspace_enhancement(&workspace, hash, true).await?;
        }
        return Ok(());
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

    if wait_for_enhancement {
        trigger_workspace_enhancement(&workspace, hash, true).await?;
    }

    Ok(())
}

async fn trigger_workspace_enhancement(
    workspace: &Workspace,
    hash_only: bool,
    wait: bool,
) -> Result<()> {
    let active = workspace.is_enhancing_active();
    let needs_enhancement = if hash_only {
        workspace.needs_hash_enhancement()
    } else {
        workspace.needs_neural_enhancement()
    };
    if !active && !needs_enhancement {
        return Ok(());
    }

    if !active {
        if !config::background_enhancement_enabled() {
            if wait {
                bail!("background enhancement is disabled by environment configuration");
            }
            return Ok(());
        }
        if hash_only {
            workspace.trigger_background_hash_enhancement()?;
        } else {
            workspace.trigger_background_enhancement()?;
        }
    }
    if wait {
        wait_for_workspace_enhancement(workspace, hash_only).await?;
    }
    Ok(())
}

async fn wait_for_workspace_enhancement(workspace: &Workspace, hash_only: bool) -> Result<()> {
    let terminal_error;
    loop {
        let workspaces = crate::workspace::list_workspaces()?;
        let status = workspaces
            .iter()
            .find(|status| status.id == workspace.id)
            .with_context(|| {
                format!(
                    "workspace disappeared while waiting for enhancement: {}",
                    workspace.root.display()
                )
            })?;
        if status.enhancing_stalled {
            bail!(
                "background enhancement stalled{}",
                status
                    .enhancing_error
                    .as_deref()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            );
        }
        if !workspace.is_enhancing_active() {
            terminal_error = status.enhancing_error.clone();
            break;
        }

        if std::io::stderr().is_terminal() {
            let phase = status.enhancing_phase.as_deref().unwrap_or("background");
            let progress = if let Some(count) = status.enhancing_progress_count {
                let percent = if status.chunk_count > 0 {
                    (count as f64 / status.chunk_count as f64 * 100.0).min(100.0) as u64
                } else {
                    100
                };
                format!(
                    " ({} / {} chunks, ~{}%)",
                    count, status.chunk_count, percent
                )
            } else {
                String::new()
            };
            eprint!("\r\x1b[K  waiting for background {phase} enhancement{progress}...");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let hash_complete = !workspace.needs_hash_enhancement();
    let expected_neural_unavailable = terminal_error
        .as_deref()
        .is_some_and(|error| error.contains("neural feature not compiled"))
        && hash_complete;
    if let Some(error) = terminal_error
        && !expected_neural_unavailable
    {
        bail!("background enhancement failed: {error}");
    }
    if !hash_complete {
        bail!("background enhancement exited before hash vectors were complete");
    }
    if !hash_only && workspace.needs_neural_enhancement() && !expected_neural_unavailable {
        bail!("background enhancement exited before neural vectors were complete");
    }

    if std::io::stderr().is_terminal() {
        eprintln!("\r\x1b[K  ✓ background enhancement complete");
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

async fn run_query(cli: Cli, context_args: Option<ContextArgs>) -> Result<()> {
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
    let local_only_mode = cli.lexical_only || cli.symbol || cli.refs || cli.callers;
    let watch_configured =
        should_autospawn_daemon_for_query(&workspace, cli.no_watch) && !local_only_mode;
    let watcher_health_required = !cli.no_watch
        && !local_only_mode
        && workspace
            .read_metadata()
            .ok()
            .flatten()
            .is_some_and(|metadata| metadata.watch_enabled);
    let scope_path = scope_filter.as_ref().map(|scope| scope.rel_path.clone());
    let scope_is_file = scope_filter.as_ref().is_some_and(|scope| scope.is_file);
    let skip_static_daemon_status = should_skip_static_daemon_status(watch_configured);
    let initial_index_state = if cli.all_indices {
        None
    } else if skip_static_daemon_status {
        Some(workspace.quick_index_health().state)
    } else {
        Some(initial_query_index_state(&workspace))
    };

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
        } else if skip_static_daemon_status
            || (!watcher_health_required && crate::ipc::socket_exists())
        {
            search_via_daemon = true;
        } else {
            // Already indexed. Just check if the daemon is online to route the search request.
            // Also verify the daemon version matches — stale daemons silently break search.
            let _t = std::time::Instant::now();
            match daemon::request::<fn(String, usize, usize)>(
                &DaemonRequest::RuntimeStatus {
                    path: query_path_opt.clone(),
                },
                watch_configured,
                None,
            )
            .await?
            {
                Some(DaemonResponse::RuntimeStatus {
                    version,
                    workspace: runtime_status,
                }) => {
                    if version.as_deref() == Some(BUILD_VERSION) {
                        let watcher_offline = watch_configured
                            && runtime_status.as_ref().is_some_and(|status| {
                                status.id == workspace.id
                                    && status.watch_enabled
                                    && !status.watcher_alive
                            });
                        if watcher_offline {
                            tracing::warn!(
                                "daemon online but watcher offline for {}, restarting",
                                workspace.root.display()
                            );
                            restart_daemon().await;
                            search_via_daemon = daemon::request::<fn(String, usize, usize)>(
                                &DaemonRequest::RuntimeStatus {
                                    path: query_path_opt.clone(),
                                },
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
                            &DaemonRequest::RuntimeStatus {
                                path: query_path_opt.clone(),
                            },
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
                        &DaemonRequest::RuntimeStatus {
                            path: query_path_opt.clone(),
                        },
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
        &DaemonRequest::Version,
        !cli.no_watch,
        None,
    )
    .await?
    .is_some()
    {
        search_via_daemon = true;
    }

    // Indexing commits SQLite + Tantivy first so BM25/literal search becomes
    // available quickly. Background enhancement builds hash ANN, then neural
    // vectors, without blocking first results.

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
                    let model = crate::embedding::create_hash_model();
                    let _ = crate::indexer::index_workspace(&workspace, model.as_ref());
                }
            }
        }
    }

    if let Some(context_args) = context_args {
        let options = SearchOptions {
            limit: None,
            context: 12,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_filter: scope_filter.clone(),
            skip_gitignore: cli.skip_gitignore,
            force_neural: cli.force_neural,
            progress_tx: None,
            cancel_token: None,
        };
        let model = local_hybrid_search_model(
            std::slice::from_ref(&workspace),
            query,
            cli.hash,
            cli.lexical_only,
            cli.force_neural,
        )?;
        let bundle = crate::context::build_context_bundle(
            &workspace,
            query,
            model.as_deref(),
            &options,
            context_args.budget,
        )?;
        if cli.json {
            println!("{}", serde_json::to_string_pretty(&bundle)?);
        } else {
            print!("{}", crate::context::render_markdown(&bundle));
        }
        if !cli.lexical_only {
            let result =
                trigger_workspace_enhancement(&workspace, cli.hash, cli.wait_for_enhancement).await;
            if cli.wait_for_enhancement {
                result?;
            }
        }
        std::process::exit(0);
    }

    let hits = if cli.symbol || cli.refs || cli.callers {
        let mode = if cli.symbol {
            crate::symbols::SymbolSearchMode::Definitions
        } else if cli.refs {
            crate::symbols::SymbolSearchMode::References
        } else {
            crate::symbols::SymbolSearchMode::Callers
        };
        let options = SearchOptions {
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_filter: scope_filter.clone(),
            skip_gitignore: cli.skip_gitignore,
            force_neural: false,
            progress_tx: None,
            cancel_token: None,
        };
        local_symbol_search_hits(&workspace, cli.all_indices, query, mode, &options)?
    } else if cli.literal {
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
        let local_options = SearchOptions {
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_filter: scope_filter.clone(),
            skip_gitignore: cli.skip_gitignore,
            force_neural: cli.force_neural,
            progress_tx: None,
            cancel_token: None,
        };

        if search_via_daemon {
            match daemon::request::<fn(String, usize, usize)>(&request, false, None).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => {
                    tracing::warn!(
                        "daemon literal search failed ({message}), falling back to local"
                    );
                    local_literal_search_hits(&workspace, cli.all_indices, query, &local_options)?
                }
                other => {
                    tracing::warn!(
                        "daemon literal search unavailable ({other:?}), falling back to local"
                    );
                    local_literal_search_hits(&workspace, cli.all_indices, query, &local_options)?
                }
            }
        } else {
            local_literal_search_hits(&workspace, cli.all_indices, query, &local_options)?
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
        let local_options = SearchOptions {
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_filter: scope_filter.clone(),
            skip_gitignore: cli.skip_gitignore,
            force_neural: false,
            progress_tx: None,
            cancel_token: None,
        };

        if search_via_daemon {
            match daemon::request::<fn(String, usize, usize)>(&request, false, None).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                other => {
                    tracing::warn!(
                        "daemon regex search unavailable ({other:?}), falling back to local"
                    );
                    local_regex_search_hits(&workspace, cli.all_indices, query, &local_options)?
                }
            }
        } else {
            local_regex_search_hits(&workspace, cli.all_indices, query, &local_options)?
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
            force_neural: cli.force_neural,
        };
        let local_options = SearchOptions {
            limit: backend_limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
            include_globs: cli.include.clone(),
            exclude_globs: cli.exclude.clone(),
            scope_filter: scope_filter.clone(),
            skip_gitignore: cli.skip_gitignore,
            force_neural: cli.force_neural,
            progress_tx: None,
            cancel_token: None,
        };

        if should_route_hybrid_query_via_daemon(search_via_daemon, cli.lexical_only) {
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
                Some(DaemonResponse::Error { message }) if cli.force_neural => {
                    bail!(message)
                }
                Some(DaemonResponse::Error { message }) => {
                    // Daemon search failed — fall back to local search instead
                    // of showing "No results." to the user.
                    tracing::warn!("daemon search failed ({message}), falling back to local");
                    local_fallback_search(
                        &workspace,
                        cli.all_indices,
                        query,
                        &local_options,
                        cli.hash,
                    )?
                }
                other => {
                    tracing::warn!("daemon search unavailable ({other:?}), falling back to local");
                    local_fallback_search(
                        &workspace,
                        cli.all_indices,
                        query,
                        &local_options,
                        cli.hash,
                    )?
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
            let search_model = local_hybrid_search_model(
                &workspaces,
                query,
                cli.hash,
                cli.lexical_only,
                cli.force_neural,
            )?;
            for ws in workspaces {
                let _ = ws.cleanup_stale_legacy_runtime_files();
                let _t_search = std::time::Instant::now();
                match hybrid_search(&ws, query, search_model.as_deref(), &local_options) {
                    Ok(mut hits) => {
                        if hits.is_empty()
                            && !cli.all_indices
                            && let Some(retry_hits) =
                                retry_after_query_repair(&ws, cli.skip_gitignore, || {
                                    hybrid_search(
                                        &ws,
                                        query,
                                        search_model.as_deref(),
                                        &local_options,
                                    )
                                })?
                        {
                            hits = retry_hits;
                        }
                        if cli.all_indices {
                            for hit in &mut hits {
                                hit.file_path = ws.root.join(&hit.file_path);
                            }
                        }
                        all_hits.append(&mut hits);
                    }
                    Err(err) => {
                        if !cli.all_indices
                            && let Some(mut hits) =
                                retry_after_query_repair(&ws, cli.skip_gitignore, || {
                                    hybrid_search(
                                        &ws,
                                        query,
                                        search_model.as_deref(),
                                        &local_options,
                                    )
                                })?
                        {
                            all_hits.append(&mut hits);
                            continue;
                        }
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

    // Kick off background hash and neural enhancement if not already done.
    // Normal queries return immediately; --wait-for-enhancement propagates
    // worker failures and exits only after requested vectors are durable.
    // We launch it as a separate hidden CLI process to prevent segmentation faults
    // observed while tearing down neural-model state when the main process exits.
    // Skipped in CI/test environments (IVYGREP_NO_AUTOSPAWN=1).
    if !cli.all_indices
        && !cli.regex
        && !cli.lexical_only
        && !cli.symbol
        && !cli.refs
        && !cli.callers
    {
        let result =
            trigger_workspace_enhancement(&workspace, cli.hash, cli.wait_for_enhancement).await;
        if cli.wait_for_enhancement {
            result?;
        }
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
        DaemonResponse::Version { .. }
        | DaemonResponse::RuntimeStatus { .. }
        | DaemonResponse::WebStarted { .. } => Ok(()),
        DaemonResponse::SearchProgress { .. } => Ok(()),
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

fn run_doctor(path: Option<&Path>, fix: bool, deep: bool, json: bool) -> Result<()> {
    let target = match path {
        Some(path) => path.to_path_buf(),
        None => env::current_dir()?,
    };
    let workspace = Workspace::resolve(&target)?;
    let report = crate::doctor::inspect_and_maybe_fix(&workspace, fix, deep)?;

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
    println!(
        "Neural: {} / {} vectors ({:.1}%), {} {}d",
        report.neural_vector_count,
        report.vector_key_count,
        report.neural_coverage_percent,
        report.neural_profile,
        report.neural_dimensions,
    );
    println!(
        "Index: chunks {}, graph {}, sqlite aux {}, lexical {}, hash {}, neural {}, other {}",
        format_bytes(report.index_components.stored_chunks_bytes),
        format_bytes(report.index_components.graph_bytes),
        format_bytes(report.index_components.sqlite_auxiliary_bytes),
        format_bytes(report.index_components.lexical_bytes),
        format_bytes(report.index_components.hash_vectors_bytes),
        format_bytes(report.index_components.neural_vectors_bytes),
        format_bytes(report.index_components.other_bytes),
    );
    println!(
        "Compaction: {} ({:.1}% free, format v{}/{})",
        if report.compaction.healthy {
            "healthy"
        } else {
            "attention"
        },
        report.compaction.sqlite_free_percent,
        report.compaction.format_version,
        report.compaction.current_format_version,
    );
    let reranker_model = report
        .reranker_model
        .as_deref()
        .map(|model| format!(" {model}"))
        .unwrap_or_default();
    println!(
        "Reranker: {}{} (top {} candidates)",
        report.reranker_mode, reranker_model, report.reranker_candidate_limit
    );
    if let Some(error) = &report.reranker_error {
        println!("Reranker warning: {error}");
    }

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
    daemon::restart_daemon_process().await;
    let _ = daemon::request::<fn(String, usize, usize)>(&DaemonRequest::Status, true, None).await;
}

/// Run a local hybrid search as a fallback when the daemon is unavailable or broken.
fn local_fallback_search(
    workspace: &Workspace,
    all_indices: bool,
    query: &str,
    options: &SearchOptions,
    use_hash: bool,
) -> Result<Vec<SearchHit>> {
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

    let model =
        local_hybrid_search_model(&workspaces, query, use_hash, false, options.force_neural)?;

    for ws in workspaces {
        match hybrid_search(&ws, query, model.as_deref(), options) {
            Ok(mut hits) => {
                if hits.is_empty()
                    && !all_indices
                    && let Some(retry_hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            hybrid_search(&ws, query, model.as_deref(), options)
                        })?
                {
                    hits = retry_hits;
                }
                if all_indices {
                    for hit in &mut hits {
                        hit.file_path = ws.root.join(&hit.file_path);
                    }
                }
                all_hits.append(&mut hits);
            }
            Err(err) => {
                if !all_indices
                    && let Some(mut hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            hybrid_search(&ws, query, model.as_deref(), options)
                        })?
                {
                    all_hits.append(&mut hits);
                    continue;
                }
                tracing::warn!(
                    "local fallback search failed for {}: {err:#}",
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
    Ok(all_hits)
}

fn local_symbol_search_hits(
    workspace: &Workspace,
    all_indices: bool,
    query: &str,
    mode: crate::symbols::SymbolSearchMode,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let workspaces = if all_indices {
        list_workspaces()?
            .into_iter()
            .filter(|status| status.last_indexed_at_unix.is_some())
            .filter_map(|status| Workspace::resolve(&status.root).ok())
            .collect()
    } else {
        vec![workspace.clone()]
    };

    let mut all_hits = Vec::new();
    for ws in workspaces {
        let mut workspace_options = options.clone();
        if all_indices {
            workspace_options.scope_filter = None;
        }
        match crate::symbols::search_symbols_with_options(&ws, query, mode, &workspace_options) {
            Ok(mut hits) => {
                if all_indices {
                    for hit in &mut hits {
                        hit.file_path = ws.root.join(&hit.file_path);
                    }
                }
                all_hits.append(&mut hits);
            }
            Err(err) => {
                if !all_indices {
                    return Err(err);
                }
                tracing::warn!("symbol search failed for {}: {err:#}", ws.root.display());
            }
        }
    }

    all_hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    if let Some(limit) = options.limit {
        all_hits.truncate(limit);
    }
    Ok(all_hits)
}

fn retry_after_query_repair<F>(
    workspace: &Workspace,
    skip_gitignore: bool,
    retry: F,
) -> Result<Option<Vec<SearchHit>>>
where
    F: FnOnce() -> Result<Vec<SearchHit>>,
{
    if repair_unhealthy_index_for_query(workspace, skip_gitignore)? {
        retry().map(Some)
    } else {
        Ok(None)
    }
}

fn repair_unhealthy_index_for_query(workspace: &Workspace, skip_gitignore: bool) -> Result<bool> {
    if workspace.index_health().is_queryable() {
        return Ok(false);
    }

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
                watch_enabled: false,
                skip_gitignore,
                index_generation: 0,
            });
    meta.skip_gitignore = skip_gitignore;

    remove_workspace_index(workspace)?;
    workspace.ensure_dirs()?;
    workspace.write_metadata(&meta)?;

    let hash_model = crate::embedding::create_hash_model();
    index_workspace(workspace, hash_model.as_ref())?;
    Ok(true)
}

fn local_literal_search_hits(
    workspace: &Workspace,
    all_indices: bool,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let mut all_hits = Vec::new();
    let workspaces = if all_indices {
        list_workspaces()?
            .into_iter()
            .filter(|w| w.last_indexed_at_unix.is_some())
            .filter_map(|w| Workspace::resolve(&w.root).ok())
            .collect()
    } else {
        vec![workspace.clone()]
    };

    for ws in workspaces {
        match literal_search(&ws, query, options) {
            Ok(mut hits) => {
                if hits.is_empty()
                    && !all_indices
                    && let Some(retry_hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            literal_search(&ws, query, options)
                        })?
                {
                    hits = retry_hits;
                }
                if all_indices {
                    for hit in &mut hits {
                        hit.file_path = ws.root.join(&hit.file_path);
                    }
                }
                all_hits.append(&mut hits);
            }
            Err(err) => {
                if !all_indices
                    && let Some(mut hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            literal_search(&ws, query, options)
                        })?
                {
                    all_hits.append(&mut hits);
                    continue;
                }
                tracing::warn!("literal_search failed for {}: {err:#}", ws.root.display());
            }
        }
    }

    if let Some(l) = options.limit {
        all_hits.truncate(l);
    }
    Ok(all_hits)
}

fn local_regex_search_hits(
    workspace: &Workspace,
    all_indices: bool,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchHit>> {
    let mut all_hits = Vec::new();
    let workspaces = if all_indices {
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
            options.limit,
            options.scope_filter.as_ref(),
            &options.include_globs,
            &options.exclude_globs,
            options.skip_gitignore,
        ) {
            Ok(mut hits) => {
                if hits.is_empty()
                    && !all_indices
                    && let Some(retry_hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            regex_search(
                                &ws,
                                query,
                                options.limit,
                                options.scope_filter.as_ref(),
                                &options.include_globs,
                                &options.exclude_globs,
                                options.skip_gitignore,
                            )
                        })?
                {
                    hits = retry_hits;
                }
                if all_indices {
                    for hit in &mut hits {
                        hit.file_path = ws.root.join(&hit.file_path);
                    }
                }
                all_hits.append(&mut hits);
            }
            Err(err) => {
                if !all_indices
                    && let Some(mut hits) =
                        retry_after_query_repair(&ws, options.skip_gitignore, || {
                            regex_search(
                                &ws,
                                query,
                                options.limit,
                                options.scope_filter.as_ref(),
                                &options.include_globs,
                                &options.exclude_globs,
                                options.skip_gitignore,
                            )
                        })?
                {
                    all_hits.append(&mut hits);
                    continue;
                }
                tracing::warn!("regex_search failed for {}: {err:#}", ws.root.display());
            }
        }
    }

    if let Some(l) = options.limit {
        all_hits.truncate(l);
    }
    Ok(all_hits)
}

fn is_single_word_symbol_query(query: &str) -> bool {
    !query.contains(' ')
        && query
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

fn local_hybrid_search_model(
    workspaces: &[Workspace],
    query: &str,
    use_hash: bool,
    lexical_only: bool,
    force_neural: bool,
) -> Result<Option<Box<dyn crate::embedding::EmbeddingModel>>> {
    if lexical_only || (!force_neural && is_single_word_symbol_query(query)) {
        return Ok(None);
    }

    if force_neural {
        validate_forced_neural_workspaces(workspaces, true)?;
        return crate::embedding::create_neural_model().map(Some);
    }

    let has_neural_vectors = workspaces.iter().any(Workspace::has_neural_vectors);
    if use_hash || !has_neural_vectors {
        Ok(Some(crate::embedding::create_hash_model()))
    } else {
        Ok(Some(create_model(false)))
    }
}

fn should_route_hybrid_query_via_daemon(daemon_available: bool, lexical_only: bool) -> bool {
    daemon_available && !lexical_only
}

fn initial_query_index_state(workspace: &Workspace) -> WorkspaceIndexState {
    workspace.quick_index_health().state
}

fn should_skip_static_daemon_status(watch_configured: bool) -> bool {
    !watch_configured
        && std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_some()
        && crate::ipc::socket_exists()
}

fn ensure_no_nested_workspaces(target_root: &Path) -> Result<()> {
    if let Ok(workspace_roots) = list_workspace_roots() {
        let mut conflicts = Vec::new();
        for root in workspace_roots {
            if root != target_root && root.starts_with(target_root) {
                conflicts.push(root);
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

    use crate::embedding::create_hash_model;
    use crate::indexer::index_workspace;
    use crate::workspace::WorkspaceMetadata;

    #[test]
    fn context_inherits_parent_lexical_only_flag() {
        let mut cli = Cli::try_parse_from(["ig", "--lexical-only", "context", "task"]).unwrap();
        let Some(CliCommand::Context(args)) = cli.command.take() else {
            panic!("context subcommand was not parsed");
        };
        apply_context_args(&mut cli, &args);
        assert!(cli.lexical_only);
    }

    #[test]
    #[serial]
    fn query_autospawn_uses_any_completed_index_unless_disabled() {
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
                watch_enabled: false,
                skip_gitignore: false,
                index_generation: 0,
            })
            .unwrap();

        assert!(should_autospawn_daemon_for_query(&workspace, false));
        assert!(!should_autospawn_daemon_for_query(&workspace, true));
    }

    #[test]
    #[serial]
    fn initial_query_index_state_tracks_index_presence() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();

        assert_eq!(
            initial_query_index_state(&workspace),
            WorkspaceIndexState::NotIndexed
        );

        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();

        assert_eq!(
            initial_query_index_state(&workspace),
            WorkspaceIndexState::Healthy
        );
    }

    #[test]
    #[serial]
    fn local_search_uses_hash_model_until_neural_vectors_exist() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();

        let model = local_hybrid_search_model(&[workspace], "semantic query", false, false, false)
            .unwrap()
            .unwrap();
        assert_eq!(model.dimensions(), 256);
    }

    #[test]
    #[serial]
    fn forced_neural_local_search_rejects_missing_vectors() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        let hash_model = create_hash_model();
        index_workspace(&workspace, hash_model.as_ref()).unwrap();

        assert!(
            local_hybrid_search_model(&[workspace], "semantic query", false, false, true).is_err()
        );
    }

    #[test]
    #[serial]
    fn initial_query_index_state_uses_quick_health_for_query_preflight() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn marker() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();

        let model = create_hash_model();
        index_workspace(&workspace, model.as_ref()).unwrap();
        std::fs::write(workspace.tantivy_dir().join("meta.json"), b"not valid json").unwrap();

        assert_eq!(
            workspace.quick_index_health().state,
            WorkspaceIndexState::Healthy
        );
        assert_eq!(
            initial_query_index_state(&workspace),
            WorkspaceIndexState::Healthy
        );
        assert_eq!(
            workspace.index_health().state,
            WorkspaceIndexState::Unhealthy
        );
    }

    #[test]
    #[serial]
    fn static_daemon_status_skip_requires_no_autospawn_and_socket() {
        let home = tempdir().unwrap();
        unsafe {
            std::env::set_var("IVYGREP_HOME", home.path());
            std::env::remove_var("IVYGREP_NO_AUTOSPAWN");
        }
        config::ensure_app_dirs().unwrap();
        crate::ipc::cleanup_socket();

        assert!(!should_skip_static_daemon_status(false));

        unsafe { std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1") };
        assert!(!should_skip_static_daemon_status(false));

        let socket_path = crate::ipc::socket_path().unwrap();
        std::fs::write(&socket_path, b"placeholder").unwrap();
        assert!(should_skip_static_daemon_status(false));
        assert!(!should_skip_static_daemon_status(true));

        crate::ipc::cleanup_socket();
        unsafe { std::env::remove_var("IVYGREP_NO_AUTOSPAWN") };
    }

    #[test]
    fn lexical_only_search_never_routes_through_daemon() {
        assert!(should_route_hybrid_query_via_daemon(true, false));
        assert!(!should_route_hybrid_query_via_daemon(true, true));
        assert!(!should_route_hybrid_query_via_daemon(false, false));
    }
}
