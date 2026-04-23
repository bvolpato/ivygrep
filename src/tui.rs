use std::io::{IsTerminal, stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::cli::Cli;
use crate::daemon;
use crate::protocol::{DaemonRequest, DaemonResponse, SearchHit};
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces, resolve_workspace_and_scope};

const TUI_DEFAULT_LIMIT: usize = 100;

fn syn_to_ratatui(style: SynStyle) -> Style {
    Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ))
}

struct App {
    input: Input,
    hits: Vec<SearchHit>,
    list_state: ListState,
    is_searching: bool,
    last_query: String,
    debounce_timer: Option<Instant>,
    cli: Cli,
    workspace: Workspace,
    scope_filter: Option<WorkspaceScope>,
    status_message: Option<String>,
    ps: SyntaxSet,
    ts: ThemeSet,
    runtime: tokio::runtime::Handle,
}

impl App {
    fn new(cli: Cli, runtime: tokio::runtime::Handle) -> Result<Self> {
        let query_path = match &cli.query_path {
            Some(path) => path.clone(),
            None => std::env::current_dir()?,
        };
        let (workspace, scope_filter) = resolve_workspace_and_scope(&query_path)?;
        prepare_workspace_for_tui(&cli, &workspace, &runtime)?;

        Ok(Self {
            input: Input::default().with_value(cli.query.clone().unwrap_or_default()),
            hits: Vec::new(),
            list_state: ListState::default(),
            is_searching: false,
            last_query: String::new(),
            debounce_timer: Some(Instant::now()),
            cli,
            workspace,
            scope_filter,
            status_message: Some("Type to search".to_string()),
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            runtime,
        })
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.hits.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.hits.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn trigger_search(&mut self) {
        let q = self.input.value().trim().to_string();
        if q.is_empty() {
            self.hits.clear();
            self.list_state.select(None);
            self.status_message = Some("Type to search".to_string());
            self.is_searching = false;
            return;
        }

        self.is_searching = true;
        self.status_message = Some("Searching...".to_string());

        let request = build_search_request(
            &self.cli,
            self.workspace.root.clone(),
            self.scope_filter.as_ref(),
            q.clone(),
        );

        let daemon_result = tokio::task::block_in_place(|| {
            self.runtime
                .block_on(async { daemon::request(&request, !self.cli.no_watch).await })
        });
        let hits = match daemon_result {
            Ok(Some(DaemonResponse::SearchResults { hits })) => Ok(hits),
            Ok(Some(DaemonResponse::Error { message })) => {
                tracing::warn!("daemon TUI search failed ({message}), falling back to local");
                self.local_search(&q)
            }
            Ok(None) => self.local_search(&q),
            Ok(Some(other)) => {
                tracing::warn!("daemon TUI search returned unexpected response: {other:?}");
                self.local_search(&q)
            }
            Err(err) => Err(err),
        };

        self.is_searching = false;

        match hits {
            Ok(mut hits) => {
                hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.hits = hits;
                if self.hits.is_empty() {
                    self.list_state.select(None);
                    self.status_message = Some("No results".to_string());
                } else {
                    self.list_state.select(Some(0));
                    self.status_message = None;
                }
            }
            Err(err) => {
                self.hits.clear();
                self.list_state.select(None);
                self.status_message = Some(format!("Search failed: {err:#}"));
            }
        }
    }

    fn local_search(&self, query: &str) -> Result<Vec<SearchHit>> {
        let model = crate::embedding::create_model(self.cli.hash);
        let options = build_search_options(&self.cli, self.scope_filter.as_ref());
        let mut all_hits = Vec::new();

        let workspaces = if self.cli.all_indices {
            list_workspaces()?
                .into_iter()
                .filter(|workspace| workspace.last_indexed_at_unix.is_some())
                .filter_map(|workspace| Workspace::resolve(&workspace.root).ok())
                .collect::<Vec<_>>()
        } else {
            vec![self.workspace.clone()]
        };

        for workspace in workspaces {
            let mut hits = hybrid_search(&workspace, query, Some(model.as_ref()), &options)?;
            if self.cli.all_indices {
                for hit in &mut hits {
                    hit.file_path = workspace.root.join(&hit.file_path);
                }
            }
            all_hits.append(&mut hits);
        }

        if let Some(limit) = tui_limit(&self.cli) {
            all_hits.truncate(limit);
        }

        Ok(all_hits)
    }
}

fn prepare_workspace_for_tui(
    cli: &Cli,
    workspace: &Workspace,
    runtime: &tokio::runtime::Handle,
) -> Result<()> {
    if cli.all_indices && !cli.skip_gitignore {
        return Ok(());
    }

    let needs_reindex_for_gitignore = cli.skip_gitignore
        && !workspace
            .read_metadata()?
            .is_some_and(|metadata| metadata.skip_gitignore);

    if crate::indexer::workspace_is_indexed(workspace) && !needs_reindex_for_gitignore {
        return Ok(());
    }

    if std::io::stderr().is_terminal() {
        eprintln!(
            "⟐ Preparing interactive search index for {}",
            workspace.root.display()
        );
    }

    let request = DaemonRequest::Index {
        path: workspace.root.clone(),
        watch: !cli.no_watch,
        skip_gitignore: cli.skip_gitignore,
    };
    let daemon_result = tokio::task::block_in_place(|| {
        runtime.block_on(async { daemon::request(&request, !cli.no_watch).await })
    });

    match daemon_result {
        Ok(Some(DaemonResponse::Ack { .. })) => Ok(()),
        Ok(Some(DaemonResponse::Error { message })) => bail!(message),
        Ok(None) => {
            ensure_local_skip_gitignore_metadata(cli, workspace)?;
            let model = crate::embedding::create_hash_model();
            crate::indexer::index_workspace(workspace, model.as_ref())?;
            Ok(())
        }
        Ok(Some(other)) => bail!("unexpected daemon response while preparing TUI: {other:?}"),
        Err(err) => Err(err),
    }
}

fn ensure_local_skip_gitignore_metadata(cli: &Cli, workspace: &Workspace) -> Result<()> {
    if !cli.skip_gitignore {
        return Ok(());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut metadata =
        workspace
            .read_metadata()?
            .unwrap_or_else(|| crate::workspace::WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: now,
                last_indexed_at_unix: None,
                watch_enabled: false,
                skip_gitignore: false,
                index_generation: 0,
            });
    if !metadata.skip_gitignore {
        metadata.skip_gitignore = true;
        workspace.ensure_dirs()?;
        workspace.write_metadata(&metadata)?;
    }

    Ok(())
}

fn tui_limit(cli: &Cli) -> Option<usize> {
    if cli.no_limit {
        Some(usize::MAX)
    } else {
        cli.limit.or(Some(TUI_DEFAULT_LIMIT))
    }
}

fn build_search_options(cli: &Cli, scope_filter: Option<&WorkspaceScope>) -> SearchOptions {
    SearchOptions {
        limit: tui_limit(cli),
        context: cli.context,
        type_filter: cli.type_filter.clone(),
        include_globs: cli.include.clone(),
        exclude_globs: cli.exclude.clone(),
        scope_filter: if cli.all_indices {
            None
        } else {
            scope_filter.cloned()
        },
        skip_gitignore: cli.skip_gitignore,
    }
}

fn build_search_request(
    cli: &Cli,
    workspace_root: PathBuf,
    scope_filter: Option<&WorkspaceScope>,
    query: String,
) -> DaemonRequest {
    let scoped = if cli.all_indices { None } else { scope_filter };
    DaemonRequest::Search {
        path: if cli.all_indices {
            None
        } else {
            Some(workspace_root)
        },
        query,
        limit: tui_limit(cli),
        context: cli.context,
        type_filter: cli.type_filter.clone(),
        include_globs: cli.include.clone(),
        exclude_globs: cli.exclude.clone(),
        scope_path: scoped.map(|scope| scope.rel_path.clone()),
        scope_is_file: scoped.is_some_and(|scope| scope.is_file),
        skip_gitignore: cli.skip_gitignore,
    }
}

struct TerminalSession {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut session = Self {
            raw_mode_enabled: false,
            alternate_screen_enabled: false,
        };

        stdout().execute(EnterAlternateScreen)?;
        session.alternate_screen_enabled = true;
        enable_raw_mode()?;
        session.raw_mode_enabled = true;

        Ok(session)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.raw_mode_enabled {
            let _ = disable_raw_mode();
        }
        if self.alternate_screen_enabled {
            let _ = stdout().execute(LeaveAlternateScreen);
        }
    }
}

pub async fn run_tui(cli: Cli) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    let mut app = App::new(cli, rt)?;

    let _session = TerminalSession::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    if !app.input.value().is_empty() {
        app.trigger_search();
        app.last_query = app.input.value().to_string();
    }

    loop {
        if let Some(timer) = app.debounce_timer
            && timer.elapsed() >= Duration::from_millis(300)
        {
            let current_query = app.input.value().to_string();
            if current_query != app.last_query {
                app.last_query = current_query;
                app.trigger_search();
            }
            app.debounce_timer = None;
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(f.area());

            let input_style = Style::default().fg(Color::Cyan);
            let input_widget = Paragraph::new(app.input.value()).style(input_style).block(
                Block::default().borders(Borders::ALL).title(Span::styled(
                    " ⟐ ivygrep ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )),
            );
            f.render_widget(input_widget, chunks[0]);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(chunks[1]);

            let items: Vec<ListItem> = if app.hits.is_empty() {
                vec![ListItem::new(Line::from(Span::styled(
                    app.status_message.as_deref().unwrap_or("No results"),
                    Style::default().fg(Color::DarkGray),
                )))]
            } else {
                app.hits
                    .iter()
                    .map(|hit| {
                        let filename = hit
                            .file_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        let path_dir = hit
                            .file_path
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let content = Line::from(vec![
                            Span::styled(
                                format!("{} ", filename),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(":{}", hit.start_line),
                                Style::default().fg(Color::Yellow),
                            ),
                            Span::styled(
                                format!("  {} - {:.2}", path_dir, hit.score),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]);
                        ListItem::new(content)
                    })
                    .collect()
            };

            let hits_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Results "))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Rgb(40, 40, 40)),
                )
                .highlight_symbol("❯ ");

            f.render_stateful_widget(hits_list, main_chunks[0], &mut app.list_state);

            let preview_block = Block::default().borders(Borders::ALL).title(" Preview ");
            if let Some(selected) = app.list_state.selected() {
                if let Some(hit) = app.hits.get(selected) {
                    let syntax = app
                        .ps
                        .find_syntax_by_extension(
                            hit.file_path
                                .extension()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap_or(""),
                        )
                        .unwrap_or_else(|| app.ps.find_syntax_plain_text());

                    let theme = app
                        .ts
                        .themes
                        .get("base16-ocean.dark")
                        .or_else(|| app.ts.themes.values().next())
                        .unwrap();
                    let mut h = HighlightLines::new(syntax, theme);

                    let mut lines = Vec::new();
                    for line in syntect::util::LinesWithEndings::from(&hit.preview) {
                        let ranges: Vec<(SynStyle, &str)> =
                            h.highlight_line(line, &app.ps).unwrap_or_default();
                        let spans: Vec<Span> = ranges
                            .into_iter()
                            .map(|(style, text)| {
                                Span::styled(text.to_string(), syn_to_ratatui(style))
                            })
                            .collect();
                        lines.push(Line::from(spans));
                    }

                    let preview = Paragraph::new(lines)
                        .block(preview_block)
                        .wrap(Wrap { trim: false });
                    f.render_widget(preview, main_chunks[1]);
                } else {
                    f.render_widget(Paragraph::new("").block(preview_block), main_chunks[1]);
                }
            } else {
                f.render_widget(Paragraph::new("").block(preview_block), main_chunks[1]);
            }

            f.set_cursor_position(ratatui::layout::Position {
                x: chunks[0].x + app.input.visual_cursor() as u16 + 1,
                y: chunks[0].y + 1,
            });
        })?;

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Up => app.previous(),
                KeyCode::Down => app.next(),
                KeyCode::Enter => {
                    app.trigger_search();
                    app.last_query = app.input.value().to_string();
                }
                _ => {
                    let prev_val = app.input.value().to_string();
                    app.input.handle_event(&Event::Key(key));
                    if prev_val != app.input.value() {
                        app.debounce_timer = Some(Instant::now());
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::*;

    #[test]
    fn tui_search_request_preserves_workspace_scope() {
        let cli = Cli::parse_from([
            "ig",
            "--interactive",
            "--hash",
            "-n",
            "7",
            "-C",
            "4",
            "--include",
            "*.rs",
            "--exclude",
            "vendor/**",
            "needle",
        ]);
        let scope = WorkspaceScope {
            rel_path: PathBuf::from("src/search.rs"),
            is_file: true,
        };

        let request = build_search_request(
            &cli,
            PathBuf::from("/repo"),
            Some(&scope),
            "needle".to_string(),
        );

        match request {
            DaemonRequest::Search {
                path,
                query,
                limit,
                context,
                include_globs,
                exclude_globs,
                scope_path,
                scope_is_file,
                skip_gitignore,
                ..
            } => {
                assert_eq!(path, Some(PathBuf::from("/repo")));
                assert_eq!(query, "needle");
                assert_eq!(limit, Some(7));
                assert_eq!(context, 4);
                assert_eq!(include_globs, vec!["*.rs"]);
                assert_eq!(exclude_globs, vec!["vendor/**"]);
                assert_eq!(scope_path, Some(PathBuf::from("src/search.rs")));
                assert!(scope_is_file);
                assert!(!skip_gitignore);
            }
            other => panic!("expected Search request, got {other:?}"),
        }
    }

    #[test]
    fn tui_all_indices_drops_workspace_scope() {
        let cli = Cli::parse_from(["ig", "--interactive", "--all-indices", "needle"]);
        let scope = WorkspaceScope {
            rel_path: PathBuf::from("src"),
            is_file: false,
        };

        let request = build_search_request(
            &cli,
            PathBuf::from("/repo"),
            Some(&scope),
            "needle".to_string(),
        );

        match request {
            DaemonRequest::Search {
                path, scope_path, ..
            } => {
                assert_eq!(path, None);
                assert_eq!(scope_path, None);
            }
            other => panic!("expected Search request, got {other:?}"),
        }
    }
}
