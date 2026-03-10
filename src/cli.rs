use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use colored::Colorize;
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

    #[arg(short, long, global = true)]
    pub force: bool,

    #[arg(long, global = true)]
    pub regex: bool,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(short = 'C', long, default_value_t = 0, global = true)]
    pub context: usize,

    #[arg(long = "type", global = true)]
    pub type_filter: Option<String>,

    #[arg(short = 'n', long, default_value_t = crate::DEFAULT_TOP_K, global = true)]
    pub limit: usize,

    #[arg(long, global = true)]
    pub no_watch: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Index(IndexArgs),
    Add(IndexArgs),
    Rm(PathArg),
    Status,
    Daemon,
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Args)]
pub struct PathArg {
    pub path: PathBuf,
}

pub async fn run() -> Result<()> {
    init_tracing();
    config::ensure_app_dirs()?;

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Daemon) => {
            daemon::run_daemon().await?;
            Ok(())
        }
        Some(Command::Status) => run_status(cli.json).await,
        Some(Command::Index(args)) | Some(Command::Add(args)) => {
            run_index(&args.path, !cli.no_watch, cli.force, cli.json).await
        }
        Some(Command::Rm(arg)) => run_remove(&arg.path, cli.json).await,
        None => run_query(cli).await,
    }
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
        path: path.to_path_buf(),
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
    let request = DaemonRequest::Remove {
        path: path.to_path_buf(),
    };
    if let Some(response) = daemon::request(&request).await? {
        return print_daemon_response(response, json);
    }

    let workspace = Workspace::resolve(path)?;
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

    let cwd = env::current_dir()?;
    let workspace = Workspace::resolve(&cwd)?;

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

        render_hits(&hits, cli.json)?;
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

    render_hits(&hits, cli.json)?;
    Ok(())
}

fn render_hits(hits: &[SearchHit], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(hits)?);
        return Ok(());
    }

    if hits.is_empty() {
        println!("No results.");
        return Ok(());
    }

    for hit in hits {
        let source = if hit.sources.is_empty() {
            String::new()
        } else {
            format!(" [{}]", hit.sources.join("+"))
        };

        println!(
            "{}:{}:{}{}",
            hit.file_path.to_string_lossy().blue(),
            hit.start_line.to_string().yellow(),
            hit.preview,
            source.dimmed(),
        );
    }

    Ok(())
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
        DaemonResponse::SearchResults { hits } => render_hits(&hits, json),
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
