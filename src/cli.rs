use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use tracing_subscriber::EnvFilter;

use crate::EMBEDDING_DIMENSIONS;
use crate::config;
use crate::daemon;
use crate::embedding::HashEmbeddingModel;
use crate::indexer::{index_workspace, remove_workspace_index, workspace_is_indexed};
use crate::protocol::{DaemonRequest, DaemonResponse, SearchHit};
use crate::regex_search::regex_search;
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, list_workspaces};

#[derive(Debug, Parser)]
#[command(author, version, about = "Semantic grep that stays local", long_about = None)]
pub struct Cli {
    #[arg(value_name = "QUERY", required = false)]
    pub query: Option<String>,

    #[arg(value_name = "PATH", required = false)]
    pub query_path: Option<PathBuf>,

    #[arg(long = "index", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub index_path: Option<PathBuf>,

    #[arg(long = "add", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub add_path: Option<PathBuf>,

    #[arg(long = "rm", value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub rm_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub status: bool,

    #[arg(long, default_value_t = false)]
    pub daemon: bool,

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
}

pub async fn run() -> Result<()> {
    init_tracing();
    config::ensure_app_dirs()?;

    let cli = Cli::parse();
    let action_count = [
        cli.index_path.is_some(),
        cli.add_path.is_some(),
        cli.rm_path.is_some(),
        cli.status,
        cli.daemon,
    ]
    .iter()
    .filter(|flag| **flag)
    .count();

    if action_count > 1 {
        bail!("use only one action at a time: --index, --add, --rm, --status, or --daemon");
    }

    if cli.daemon {
        daemon::run_daemon().await?;
        return Ok(());
    }

    if cli.status {
        return run_status(cli.json).await;
    }

    if let Some(path) = &cli.index_path {
        return run_index(path, !cli.no_watch, cli.force, cli.json).await;
    }

    if let Some(path) = &cli.add_path {
        return run_index(path, !cli.no_watch, cli.force, cli.json).await;
    }

    if let Some(path) = &cli.rm_path {
        return run_remove(path, cli.json).await;
    }

    run_query(cli).await
}

async fn run_status(json: bool) -> Result<()> {
    let response = daemon::request(&DaemonRequest::Status).await?;
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

async fn run_index(path: &Path, watch: bool, force: bool, json: bool) -> Result<()> {
    let workspace = Workspace::resolve(path)?;

    if !force && workspace_is_indexed(&workspace) && !json {
        println!("Workspace already indexed: {}", workspace.root.display());
        println!("Use --force to reindex immediately.");
    }

    let request = DaemonRequest::Index {
        path: workspace.root.clone(),
        watch,
    };

    if let Some(response) = daemon::request(&request).await? {
        return print_daemon_response(response, json);
    }

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let summary = index_workspace(&workspace, &model)?;

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
    if let Some(response) = daemon::request(&request).await? {
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
        .context("missing query. Example: ivygrep \"where is tax calculated\"")?;

    let query_path = match &cli.query_path {
        Some(path) => path.clone(),
        None => env::current_dir()?,
    };
    let workspace = Workspace::resolve(&query_path)?;

    let daemon_index_request = DaemonRequest::Index {
        path: workspace.root.clone(),
        watch: !cli.no_watch,
    };
    if let Some(response) = daemon::request(&daemon_index_request).await? {
        match response {
            DaemonResponse::Ack { .. } => {}
            DaemonResponse::Error { message } => bail!(message),
            _ => {}
        }

        let hits = if cli.regex {
            let request = DaemonRequest::RegexSearch {
                path: workspace.root.clone(),
                pattern: query.to_string(),
                limit: cli.limit,
            };

            match daemon::request(&request).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                _ => vec![],
            }
        } else {
            let request = DaemonRequest::Search {
                path: workspace.root.clone(),
                query: query.to_string(),
                limit: cli.limit,
                context: cli.context,
                type_filter: cli.type_filter.clone(),
            };

            match daemon::request(&request).await? {
                Some(DaemonResponse::SearchResults { hits }) => hits,
                Some(DaemonResponse::Error { message }) => bail!(message),
                _ => vec![],
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
        return Ok(());
    }

    if !workspace_is_indexed(&workspace) {
        let should_index = if cli.force {
            true
        } else {
            prompt_index_first_time()?
        };
        if !should_index {
            bail!("workspace is not indexed; aborting search")
        }
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let _summary = index_workspace(&workspace, &model)?;
    }

    let hits = if cli.regex {
        let request = DaemonRequest::RegexSearch {
            path: workspace.root.clone(),
            pattern: query.to_string(),
            limit: cli.limit,
        };

        match daemon::request(&request).await? {
            Some(DaemonResponse::SearchResults { hits }) => hits,
            Some(DaemonResponse::Error { message }) => bail!(message),
            _ => regex_search(&workspace, query, cli.limit)?,
        }
    } else {
        let request = DaemonRequest::Search {
            path: workspace.root.clone(),
            query: query.to_string(),
            limit: cli.limit,
            context: cli.context,
            type_filter: cli.type_filter.clone(),
        };

        match daemon::request(&request).await? {
            Some(DaemonResponse::SearchResults { hits }) => hits,
            Some(DaemonResponse::Error { message }) => bail!(message),
            _ => {
                let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
                hybrid_search(
                    &workspace,
                    query,
                    &model,
                    &SearchOptions {
                        limit: cli.limit,
                        context: cli.context,
                        type_filter: cli.type_filter.clone(),
                    },
                )?
            }
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

fn prompt_index_first_time() -> Result<bool> {
    let mut stdout = io::stdout();
    writeln!(stdout, "This folder is not indexed. Index it now? [y/N]")?;
    writeln!(stdout, "(-f to force, --no-watch to skip daemon)")?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let normalized = input.trim().to_ascii_lowercase();
    Ok(normalized == "y" || normalized == "yes")
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
