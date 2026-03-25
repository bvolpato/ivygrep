use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use tracing_subscriber::EnvFilter;

use crate::config;
use crate::daemon;
use crate::embedding::create_model;
use crate::indexer::{index_workspace, remove_workspace_index, workspace_is_indexed};
use crate::mcp;
use crate::protocol::{DaemonRequest, DaemonResponse, SearchHit};
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

    run_query(cli).await
}

async fn run_status(json: bool) -> Result<()> {
    let response = daemon::request(&DaemonRequest::Status, false).await?;
    let workspaces = match response {
        Some(DaemonResponse::Status { workspaces }) => workspaces,
        Some(DaemonResponse::Error { message }) => bail!(message),
        _ => list_workspaces()?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
    } else if workspaces.is_empty() {
        println!("No indexed workspaces.");
    } else {
        for ws in workspaces {
            let marker = if ws.watch_enabled {
                "watching"
            } else {
                "static"
            };
            let indexed = ws
                .last_indexed_at_unix
                .map(|ts| ts.to_string())
                .unwrap_or_else(|| "never".to_string());
            println!("{}\t{}\t{}\t{}", ws.id, marker, indexed, ws.root.display());
        }
    }

    Ok(())
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
        let daemon_index_request = DaemonRequest::Index {
            path: workspace.root.clone(),
            watch: !cli.no_watch,
        };
        if let Some(response) = daemon::request(&daemon_index_request, !cli.no_watch).await? {
            if let DaemonResponse::Error { message } = response {
                bail!(message);
            }
            search_via_daemon = true;
        }
    } else {
        if daemon::request(&DaemonRequest::Status, !cli.no_watch)
            .await?
            .is_some()
        {
            search_via_daemon = true;
        }
    }

    if !search_via_daemon && !cli.all {
        if !workspace_is_indexed(&workspace) {
            eprintln!(
                "{} {} {}",
                "⟐".bold(),
                "First run — indexing".bold(),
                workspace.root.display().to_string().dimmed()
            );
        }
        let model = create_model(cli.hash);
        let _summary = index_workspace(&workspace, model.as_ref())?;
    }

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
                _ => vec![],
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
                if let Ok(mut hits) = regex_search(
                    &ws,
                    query,
                    cli.limit,
                    scope_filter.as_ref(),
                    &cli.include,
                    &cli.exclude,
                ) {
                    all_hits.append(&mut hits);
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

        if search_via_daemon {
            match daemon::request(&request, false).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                _ => vec![],
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
            let model = create_model(cli.hash);
            for ws in workspaces {
                if let Ok(mut hits) = hybrid_search(
                    &ws,
                    query,
                    model.as_ref(),
                    &SearchOptions {
                        limit: cli.limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                        include_globs: cli.include.clone(),
                        exclude_globs: cli.exclude.clone(),
                        scope_filter: scope_filter.clone(),
                    },
                ) {
                    all_hits.append(&mut hits);
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
        }
    };

    render_hits(
        &hits,
        cli.json,
        cli.limit,
        cli.first_line_only,
        cli.file_name_only,
        cli.verbose,
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct FileSearchResult {
    file_path: PathBuf,
    total_score: f32,
    hit_count: usize,
    hits: Vec<SearchHit>,
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

fn group_hits_by_file(hits: &[SearchHit], limit: Option<usize>) -> Vec<FileSearchResult> {
    let mut grouped = HashMap::<PathBuf, FileSearchResult>::new();

    for hit in hits {
        let entry = grouped
            .entry(hit.file_path.clone())
            .or_insert_with(|| FileSearchResult {
                file_path: hit.file_path.clone(),
                total_score: 0.0,
                hit_count: 0,
                hits: vec![],
            });
        entry.total_score += hit.score;
        entry.hit_count += 1;
        entry.hits.push(hit.clone());
    }

    let mut files = grouped.into_values().collect::<Vec<_>>();
    for file in &mut files {
        file.hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.start_line.cmp(&b.start_line))
        });
    }

    files.sort_by(|a, b| {
        b.total_score
            .total_cmp(&a.total_score)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    if let Some(limit) = limit {
        files.truncate(limit);
    }

    files
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
        DaemonResponse::Status { workspaces } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&workspaces)?);
            } else {
                for ws in workspaces {
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
