use std::io::{IsTerminal, stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyModifiers},
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
use crate::protocol::{
    DaemonRequest, DaemonResponse, FileSearchResult, SearchHit, group_hits_by_file,
};
use crate::search::{SearchOptions, hybrid_search};
use crate::workspace::{Workspace, WorkspaceScope, list_workspaces, resolve_workspace_and_scope};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TUI_DEFAULT_LIMIT: usize = 100;
const FLASH_DURATION: Duration = Duration::from_secs(3);

// ---------------------------------------------------------------------------
// Interaction modes
// ---------------------------------------------------------------------------

/// The TUI uses a hierarchical navigation model:
///   Search → FileList → SnippetList → FileView
///   Esc goes back one level; Enter goes forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Cursor is inside the search input box.
    Search,
    /// Browsing the deduplicated file list (left panel).
    FileList,
    /// Navigating individual snippets within a file (right panel).
    SnippetList,
    /// Viewing the full expanded file content (right panel), scrollable.
    FileView,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn syn_to_ratatui(style: SynStyle) -> Style {
    Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ))
}

fn resolve_editor() -> String {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string())
}

fn open_in_editor(
    file: &std::path::Path,
    line: usize,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    let editor = resolve_editor();
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    let result = std::process::Command::new(&editor)
        .arg(format!("+{}", line))
        .arg(file)
        .status();

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    match result {
        Ok(s) if !s.success() => bail!("editor exited with {s}"),
        Err(e) => bail!("failed to launch {editor}: {e}"),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    // -- input --
    input: Input,

    // -- search results --
    hits: Vec<SearchHit>,
    grouped_files: Vec<FileSearchResult>,

    // -- selection --
    file_list_state: ListState,
    snippet_index: usize,

    // -- view --
    mode: Mode,
    file_view_scroll: u16,
    file_view_cache: Option<(PathBuf, String)>,

    // -- search lifecycle --
    is_searching: bool,
    last_query: String,
    debounce_timer: Option<Instant>,

    // -- config --
    cli: Cli,
    workspace: Workspace,
    scope_filter: Option<WorkspaceScope>,

    // -- ui chrome --
    status_message: Option<String>,
    flash: Option<(String, Instant)>,

    // -- syntax --
    ps: SyntaxSet,
    ts: ThemeSet,

    // -- runtime --
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
            grouped_files: Vec::new(),
            file_list_state: ListState::default(),
            snippet_index: 0,
            mode: Mode::Search,
            file_view_scroll: 0,
            file_view_cache: None,
            is_searching: false,
            last_query: String::new(),
            debounce_timer: Some(Instant::now()),
            cli,
            workspace,
            scope_filter,
            status_message: Some("Type to search".to_string()),
            flash: None,
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            runtime,
        })
    }

    // -- file list navigation --

    fn next_file(&mut self) {
        if self.grouped_files.is_empty() {
            return;
        }
        let i = match self.file_list_state.selected() {
            Some(i) if i >= self.grouped_files.len() - 1 => 0,
            Some(i) => i + 1,
            None => 0,
        };
        self.file_list_state.select(Some(i));
        self.snippet_index = 0;
    }

    fn prev_file(&mut self) {
        if self.grouped_files.is_empty() {
            return;
        }
        let i = match self.file_list_state.selected() {
            Some(0) | None => self.grouped_files.len().saturating_sub(1),
            Some(i) => i - 1,
        };
        self.file_list_state.select(Some(i));
        self.snippet_index = 0;
    }

    // -- snippet navigation --

    fn next_snippet(&mut self) {
        let count = self.current_snippets().len();
        if count == 0 {
            return;
        }
        self.snippet_index = if self.snippet_index >= count - 1 {
            0
        } else {
            self.snippet_index + 1
        };
    }

    fn prev_snippet(&mut self) {
        let count = self.current_snippets().len();
        if count == 0 {
            return;
        }
        self.snippet_index = if self.snippet_index == 0 {
            count.saturating_sub(1)
        } else {
            self.snippet_index - 1
        };
    }

    /// Snippets for the currently selected file.
    fn current_snippets(&self) -> &[SearchHit] {
        self.file_list_state
            .selected()
            .and_then(|i| self.grouped_files.get(i))
            .map(|f| f.hits.as_slice())
            .unwrap_or(&[])
    }

    /// Currently selected snippet (if any).
    fn selected_snippet(&self) -> Option<&SearchHit> {
        self.current_snippets().get(self.snippet_index)
    }

    fn absolute_path_for(&self, path: &std::path::Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace.root.join(path)
        }
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }

    fn active_flash(&self) -> Option<&str> {
        self.flash
            .as_ref()
            .filter(|(_, t)| t.elapsed() < FLASH_DURATION)
            .map(|(msg, _)| msg.as_str())
    }

    /// Load file content for the FileView mode.
    fn ensure_file_view_cache(&mut self) {
        if let Some(file_idx) = self.file_list_state.selected()
            && let Some(file) = self.grouped_files.get(file_idx)
        {
            let abs = self.absolute_path_for(&file.file_path);
            if self
                .file_view_cache
                .as_ref()
                .is_some_and(|(p, _)| p == &abs)
            {
                return;
            }
            match std::fs::read_to_string(&abs) {
                Ok(content) => self.file_view_cache = Some((abs, content)),
                Err(e) => {
                    self.file_view_cache = Some((abs, format!("(error reading file: {e})")));
                }
            }
        }
    }

    /// Reset to clean search state (when input is cleared).
    fn reset_results(&mut self) {
        self.hits.clear();
        self.grouped_files.clear();
        self.file_list_state.select(None);
        self.snippet_index = 0;
        self.file_view_cache = None;
        self.file_view_scroll = 0;
        self.status_message = Some("Type to search".to_string());
    }

    // -- search --

    fn trigger_search(&mut self) {
        let q = self.input.value().trim().to_string();
        if q.is_empty() {
            self.reset_results();
            self.is_searching = false;
            return;
        }

        self.is_searching = true;
        self.status_message = Some("Searching…".to_string());

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
        let result = match daemon_result {
            Ok(Some(DaemonResponse::SearchResults { hits })) => Ok(hits),
            Ok(Some(DaemonResponse::Error { message })) => {
                tracing::warn!("daemon TUI search failed ({message}), falling back to local");
                self.local_search(&q)
            }
            Ok(None) => self.local_search(&q),
            Ok(Some(other)) => {
                tracing::warn!("unexpected daemon response: {other:?}");
                self.local_search(&q)
            }
            Err(err) => Err(err),
        };

        self.is_searching = false;

        match result {
            Ok(mut hits) => {
                hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.hits = hits;
                self.grouped_files = group_hits_by_file(&self.hits, None);
                if self.grouped_files.is_empty() {
                    self.file_list_state.select(None);
                    self.status_message = Some("No results".to_string());
                } else {
                    self.file_list_state.select(Some(0));
                    self.status_message = None;
                }
                self.snippet_index = 0;
                self.file_view_cache = None;
                self.file_view_scroll = 0;
            }
            Err(err) => {
                self.reset_results();
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
                .filter(|ws| ws.last_indexed_at_unix.is_some())
                .filter_map(|ws| Workspace::resolve(&ws.root).ok())
                .collect::<Vec<_>>()
        } else {
            vec![self.workspace.clone()]
        };

        for ws in workspaces {
            let mut hits = hybrid_search(&ws, query, Some(model.as_ref()), &options)?;
            if self.cli.all_indices {
                for hit in &mut hits {
                    hit.file_path = ws.root.join(&hit.file_path);
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

// ---------------------------------------------------------------------------
// Workspace / search helpers (unchanged)
// ---------------------------------------------------------------------------

fn prepare_workspace_for_tui(
    cli: &Cli,
    workspace: &Workspace,
    runtime: &tokio::runtime::Handle,
) -> Result<()> {
    if cli.all_indices && !cli.skip_gitignore {
        return Ok(());
    }

    let needs_reindex_for_gitignore =
        cli.skip_gitignore && !workspace.read_metadata()?.is_some_and(|m| m.skip_gitignore);

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
        scope_path: scoped.map(|s| s.rel_path.clone()),
        scope_is_file: scoped.is_some_and(|s| s.is_file),
        skip_gitignore: cli.skip_gitignore,
    }
}

// ---------------------------------------------------------------------------
// Terminal session RAII
// ---------------------------------------------------------------------------

struct TerminalSession {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut s = Self {
            raw_mode_enabled: false,
            alternate_screen_enabled: false,
        };
        stdout().execute(EnterAlternateScreen)?;
        s.alternate_screen_enabled = true;
        enable_raw_mode()?;
        s.raw_mode_enabled = true;
        Ok(s)
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

// ---------------------------------------------------------------------------
// Status-bar hint lines
// ---------------------------------------------------------------------------

fn hints_search() -> Line<'static> {
    hint_line(&[
        ("↓/Tab", "results"),
        ("Enter", "search"),
        ("Esc", "clear/quit"),
    ])
}

fn hints_file_list() -> Line<'static> {
    hint_line(&[
        ("↑↓", "navigate"),
        ("Enter", "snippets"),
        ("e", "edit"),
        ("y", "copy"),
        ("Esc", "search"),
    ])
}

fn hints_snippet_list() -> Line<'static> {
    hint_line(&[
        ("↑↓", "navigate"),
        ("Enter", "expand"),
        ("e", "edit"),
        ("y", "copy"),
        ("Esc", "back"),
    ])
}

fn hints_file_view() -> Line<'static> {
    hint_line(&[
        ("↑↓", "scroll"),
        ("PgUp/Dn", "fast"),
        ("e", "edit"),
        ("y", "copy"),
        ("Esc", "back"),
    ])
}

fn hint_line(pairs: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled(
            *key,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {desc}"),
            Style::default().fg(Color::Rgb(145, 145, 160)),
        ));
    }
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Snippet rendering for the right panel
// ---------------------------------------------------------------------------

/// Build syntax-highlighted lines for all snippets of the current file.
/// Returns the lines and the start-line offset of each snippet inside the vec.
fn render_snippet_lines(
    snippets: &[SearchHit],
    selected: Option<usize>,
    ps: &SyntaxSet,
    ts: &ThemeSet,
) -> (Vec<Line<'static>>, Vec<usize>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
        .unwrap();

    for (i, hit) in snippets.iter().enumerate() {
        offsets.push(lines.len());
        let is_sel = selected == Some(i);

        // ── header ──
        let marker = if is_sel { "❯ " } else { "  " };
        let hdr_style = if is_sel {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, hdr_style),
            Span::styled(format!(":{}-{}", hit.start_line, hit.end_line), hdr_style),
            Span::styled(
                format!("  [score {:.2}]", hit.score),
                Style::default().fg(Color::Rgb(140, 140, 150)),
            ),
        ]));

        // ── syntax-highlighted code ──
        let ext = hit
            .file_path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");
        let syntax = ps
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| ps.find_syntax_plain_text());
        let mut hl = HighlightLines::new(syntax, theme);
        let sel_bg = if is_sel {
            Some(Color::Rgb(35, 42, 62))
        } else {
            None
        };

        for src_line in syntect::util::LinesWithEndings::from(&hit.preview) {
            let ranges: Vec<(SynStyle, &str)> = hl.highlight_line(src_line, ps).unwrap_or_default();
            let spans: Vec<Span> = ranges
                .into_iter()
                .map(|(sty, text)| {
                    let mut rs = syn_to_ratatui(sty);
                    if let Some(bg) = sel_bg {
                        rs = rs.bg(bg);
                    }
                    Span::styled(text.to_string(), rs)
                })
                .collect();
            lines.push(Line::from(spans));
        }

        // ── visual divider between snippets ──
        if i + 1 < snippets.len() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  ────────────────────────────────────────────",
                Style::default().fg(Color::Rgb(60, 65, 80)),
            )));
            lines.push(Line::from(""));
        }
    }

    (lines, offsets)
}

/// Build syntax-highlighted lines for the full file with line numbers.
/// Lines within `hl_range` get a subtle background highlight.
fn render_file_view_lines(
    content: &str,
    file_path: &std::path::Path,
    hl_range: Option<(usize, usize)>,
    ps: &SyntaxSet,
    ts: &ThemeSet,
) -> Vec<Line<'static>> {
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
        .unwrap();
    let ext = file_path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("");
    let syntax = ps
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| ps.find_syntax_plain_text());
    let mut hl = HighlightLines::new(syntax, theme);

    let total_lines = content.lines().count();
    let gutter_w = format!("{}", total_lines).len().max(3);

    let mut lines = Vec::with_capacity(total_lines);
    for (idx, src) in content.lines().enumerate() {
        let line_num = idx + 1;
        let in_hl = hl_range.is_some_and(|(lo, hi)| line_num >= lo && line_num <= hi);

        let gutter_style = if in_hl {
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(35, 42, 62))
        } else {
            Style::default().fg(Color::Rgb(100, 100, 115))
        };

        let mut spans = vec![Span::styled(
            format!(" {:>width$} │ ", line_num, width = gutter_w),
            gutter_style,
        )];

        // Need to add a newline so syntect can parse.
        let src_nl = format!("{src}\n");
        let ranges: Vec<(SynStyle, &str)> = hl.highlight_line(&src_nl, ps).unwrap_or_default();
        for (sty, text) in ranges {
            let mut rs = syn_to_ratatui(sty);
            if in_hl {
                rs = rs.bg(Color::Rgb(35, 42, 62));
            }
            // Trim trailing newline from the last span.
            let t = text.trim_end_matches('\n').to_string();
            if !t.is_empty() {
                spans.push(Span::styled(t, rs));
            }
        }

        lines.push(Line::from(spans));
    }

    lines
}

// ---------------------------------------------------------------------------
// Main TUI loop
// ---------------------------------------------------------------------------

pub async fn run_tui(cli: Cli) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    let mut app = App::new(cli, rt)?;

    let _session = TerminalSession::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Pre-filled query → immediate search.
    if !app.input.value().is_empty() {
        app.trigger_search();
        app.last_query = app.input.value().to_string();
    }

    loop {
        // ---- debounced search ----
        if let Some(timer) = app.debounce_timer
            && timer.elapsed() >= Duration::from_millis(300)
        {
            let current = app.input.value().to_string();
            if current != app.last_query {
                app.last_query = current;
                app.trigger_search();
            }
            app.debounce_timer = None;
        }

        // ---- render ----
        terminal.draw(|f| {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // search input
                    Constraint::Min(1),    // main area
                    Constraint::Length(1), // status bar
                ])
                .split(f.area());

            // ========== search input ==========
            let focused_on_input = app.mode == Mode::Search;
            let border_color = if focused_on_input {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let title_style = if focused_on_input {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let mode_tag = match app.mode {
                Mode::Search => "",
                Mode::FileList => "  files",
                Mode::SnippetList => "  snippets",
                Mode::FileView => "  view",
            };
            let input_widget = Paragraph::new(app.input.value())
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .title(vec![
                            Span::styled(" ⟐ ivygrep ", title_style),
                            Span::styled(
                                mode_tag,
                                Style::default()
                                    .fg(Color::Rgb(140, 140, 170))
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        ]),
                );
            f.render_widget(input_widget, outer[0]);

            // ========== main area: file list + right panel ==========
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(outer[1]);

            // ----- left: file list -----
            let file_border_color = if app.mode == Mode::FileList {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let file_title = if app.grouped_files.is_empty() {
                " Files ".to_string()
            } else {
                format!(" Files ({}) ", app.grouped_files.len())
            };

            let file_items: Vec<ListItem> = if app.grouped_files.is_empty() {
                vec![ListItem::new(Line::from(Span::styled(
                    app.status_message.as_deref().unwrap_or("No results"),
                    Style::default().fg(Color::DarkGray),
                )))]
            } else {
                app.grouped_files
                    .iter()
                    .enumerate()
                    .map(|(idx, file_result)| {
                        let is_active = matches!(app.mode, Mode::SnippetList | Mode::FileView)
                            && app.file_list_state.selected() == Some(idx);

                        let filename = file_result
                            .file_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let dir = file_result
                            .file_path
                            .parent()
                            .map(|p| {
                                let s = p.to_string_lossy().to_string();
                                if s.is_empty() { ".".to_string() } else { s }
                            })
                            .unwrap_or_else(|| ".".to_string());
                        let active_indicator = if is_active { "▸" } else { " " };

                        let name_style = if is_active {
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        };

                        ListItem::new(Line::from(vec![
                            Span::styled(format!("{active_indicator} {filename}"), name_style),
                            Span::styled(
                                format!("  {dir}"),
                                Style::default().fg(Color::Rgb(120, 120, 135)),
                            ),
                            Span::styled(
                                format!(
                                    "  [{} hit{} · score {:.2}]",
                                    file_result.hit_count,
                                    if file_result.hit_count == 1 { "" } else { "s" },
                                    file_result.total_score
                                ),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]))
                    })
                    .collect()
            };

            let file_list = List::new(file_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(file_border_color))
                        .title(file_title),
                )
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Rgb(45, 45, 65)),
                )
                .highlight_symbol("❯ ");

            f.render_stateful_widget(file_list, main[0], &mut app.file_list_state);

            // ----- right panel -----
            let right_border_color = if matches!(app.mode, Mode::SnippetList | Mode::FileView) {
                Color::Cyan
            } else {
                Color::DarkGray
            };

            match app.mode {
                // -- FileView: full file with line numbers --
                Mode::FileView => {
                    app.ensure_file_view_cache();
                    let (content, file_path) =
                        if let Some((ref path, ref content)) = app.file_view_cache {
                            (content.as_str(), path.clone())
                        } else {
                            ("", PathBuf::new())
                        };

                    let hl_range = app.selected_snippet().map(|h| (h.start_line, h.end_line));

                    let view_lines =
                        render_file_view_lines(content, &file_path, hl_range, &app.ps, &app.ts);
                    let total = view_lines.len() as u16;
                    let visible = main[1].height.saturating_sub(2);
                    let max_scroll = total.saturating_sub(visible);
                    if app.file_view_scroll > max_scroll {
                        app.file_view_scroll = max_scroll;
                    }

                    let display_path = app
                        .file_list_state
                        .selected()
                        .and_then(|i| app.grouped_files.get(i))
                        .map(|f| f.file_path.display().to_string())
                        .unwrap_or_default();
                    let scroll_info = if total > visible {
                        format!(" [{}/{}]", app.file_view_scroll + 1, max_scroll + 1)
                    } else {
                        String::new()
                    };

                    let view_widget = Paragraph::new(view_lines)
                        .scroll((app.file_view_scroll, 0))
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(right_border_color))
                                .title(format!(" {display_path}{scroll_info} ")),
                        );
                    f.render_widget(view_widget, main[1]);
                }

                // -- Search / FileList / SnippetList: snippet previews --
                _ => {
                    let snippets = app.current_snippets();
                    let sel = if app.mode == Mode::SnippetList {
                        Some(app.snippet_index)
                    } else {
                        None
                    };

                    let (snippet_lines, snippet_offsets) =
                        render_snippet_lines(snippets, sel, &app.ps, &app.ts);

                    // Auto-scroll to selected snippet.
                    let scroll: u16 = if let Some(sel_idx) = sel
                        && let Some(&offset) = snippet_offsets.get(sel_idx)
                    {
                        let visible = main[1].height.saturating_sub(2);
                        (offset as u16).saturating_sub(visible / 4)
                    } else {
                        0
                    };

                    let display_path = app
                        .file_list_state
                        .selected()
                        .and_then(|i| app.grouped_files.get(i))
                        .map(|f| f.file_path.display().to_string())
                        .unwrap_or_default();
                    let snippet_count = snippets.len();
                    let right_title = if snippet_count > 0 {
                        if sel.is_some() {
                            format!(
                                " {} — snippet {}/{} ",
                                display_path,
                                app.snippet_index + 1,
                                snippet_count
                            )
                        } else {
                            format!(" {} — {} snippets ", display_path, snippet_count)
                        }
                    } else {
                        " Preview ".to_string()
                    };

                    let snippet_widget = Paragraph::new(snippet_lines)
                        .scroll((scroll, 0))
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(right_border_color))
                                .title(right_title),
                        );
                    f.render_widget(snippet_widget, main[1]);
                }
            }

            // ========== status bar ==========
            let bar_line = if let Some(flash) = app.active_flash() {
                Line::from(Span::styled(
                    format!(" ✓ {flash}"),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                match app.mode {
                    Mode::Search => hints_search(),
                    Mode::FileList => hints_file_list(),
                    Mode::SnippetList => hints_snippet_list(),
                    Mode::FileView => hints_file_view(),
                }
            };
            let status_bar = Paragraph::new(bar_line)
                .style(Style::default().bg(Color::Rgb(30, 30, 45)).fg(Color::White));
            f.render_widget(status_bar, outer[2]);

            // ========== cursor (only when typing) ==========
            if app.mode == Mode::Search {
                f.set_cursor_position(ratatui::layout::Position {
                    x: outer[0].x + app.input.visual_cursor() as u16 + 1,
                    y: outer[0].y + 1,
                });
            }
        })?;

        // ---- input handling ----
        if !crossterm::event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };

        match app.mode {
            // ===== SEARCH MODE =====
            Mode::Search => match key.code {
                KeyCode::Esc => {
                    if app.input.value().is_empty() {
                        break; // quit
                    }
                    app.input = Input::default();
                    app.reset_results();
                    app.last_query.clear();
                    app.debounce_timer = None;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.input.value().is_empty() {
                        break;
                    }
                    app.input = Input::default();
                    app.reset_results();
                    app.last_query.clear();
                    app.debounce_timer = None;
                }
                KeyCode::Enter => {
                    app.trigger_search();
                    app.last_query = app.input.value().to_string();
                    if !app.grouped_files.is_empty() {
                        app.mode = Mode::FileList;
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    if !app.grouped_files.is_empty() {
                        app.mode = Mode::FileList;
                        if app.file_list_state.selected().is_none() {
                            app.file_list_state.select(Some(0));
                        }
                    }
                }
                _ => {
                    let prev = app.input.value().to_string();
                    app.input.handle_event(&Event::Key(key));
                    if prev != app.input.value() {
                        app.debounce_timer = Some(Instant::now());
                    }
                }
            },

            // ===== FILE LIST MODE =====
            Mode::FileList => match key.code {
                KeyCode::Esc | KeyCode::Left => {
                    app.mode = Mode::Search;
                }
                KeyCode::Char('/') => {
                    app.mode = Mode::Search;
                }
                KeyCode::Up | KeyCode::Char('k') => app.prev_file(),
                KeyCode::Down | KeyCode::Char('j') => app.next_file(),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    if !app.current_snippets().is_empty() {
                        app.snippet_index = 0;
                        app.mode = Mode::SnippetList;
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(file_idx) = app.file_list_state.selected()
                        && let Some(file) = app.grouped_files.get(file_idx)
                    {
                        let path = app.absolute_path_for(&file.file_path);
                        let line = file.hits.first().map(|h| h.start_line).unwrap_or(1);
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash(format!("Editor: {e:#}")),
                        }
                    }
                }
                KeyCode::Char('y') => {
                    if let Some(file_idx) = app.file_list_state.selected()
                        && let Some(file) = app.grouped_files.get(file_idx)
                    {
                        let path = app.absolute_path_for(&file.file_path);
                        let text = path.display().to_string();
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone()))
                        {
                            Ok(()) => app.flash(format!("Copied {text}")),
                            Err(e) => app.flash(format!("Clipboard: {e}")),
                        }
                    }
                }
                // Other printable chars → switch to search and type.
                KeyCode::Char(_)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    app.mode = Mode::Search;
                    app.input.handle_event(&Event::Key(key));
                    app.debounce_timer = Some(Instant::now());
                }
                KeyCode::Backspace => {
                    app.mode = Mode::Search;
                    app.input.handle_event(&Event::Key(key));
                    app.debounce_timer = Some(Instant::now());
                }
                _ => {}
            },

            // ===== SNIPPET LIST MODE =====
            Mode::SnippetList => match key.code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                    app.mode = Mode::FileList;
                }
                KeyCode::Char('/') => {
                    app.mode = Mode::Search;
                }
                KeyCode::Up | KeyCode::Char('k') => app.prev_snippet(),
                KeyCode::Down | KeyCode::Char('j') => app.next_snippet(),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    if app.selected_snippet().is_some() {
                        app.ensure_file_view_cache();
                        // Scroll to the snippet's start line.
                        if let Some(hit) = app.selected_snippet() {
                            app.file_view_scroll = (hit.start_line as u16).saturating_sub(5);
                        }
                        app.mode = Mode::FileView;
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let line = hit.start_line;
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash(format!("Editor: {e:#}")),
                        }
                    }
                }
                KeyCode::Char('y') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let text = format!("{}:{}", path.display(), hit.start_line);
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone()))
                        {
                            Ok(()) => app.flash(format!("Copied {text}")),
                            Err(e) => app.flash(format!("Clipboard: {e}")),
                        }
                    }
                }
                _ => {}
            },

            // ===== FILE VIEW MODE =====
            Mode::FileView => match key.code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                    app.mode = Mode::SnippetList;
                    app.file_view_cache = None;
                }
                KeyCode::Char('/') => {
                    app.mode = Mode::Search;
                    app.file_view_cache = None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.file_view_scroll = app.file_view_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.file_view_scroll = app.file_view_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    app.file_view_scroll = app.file_view_scroll.saturating_sub(15);
                }
                KeyCode::PageDown => {
                    app.file_view_scroll = app.file_view_scroll.saturating_add(15);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.file_view_scroll = app.file_view_scroll.saturating_sub(15);
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.file_view_scroll = app.file_view_scroll.saturating_add(15);
                }
                KeyCode::Home => {
                    app.file_view_scroll = 0;
                }
                KeyCode::End => {
                    app.file_view_scroll = u16::MAX; // clamped during render
                }
                KeyCode::Char('e') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let line = hit.start_line;
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash(format!("Editor: {e:#}")),
                        }
                    }
                }
                KeyCode::Char('y') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let text = format!("{}:{}", path.display(), hit.start_line);
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone()))
                        {
                            Ok(()) => app.flash(format!("Copied {text}")),
                            Err(e) => app.flash(format!("Clipboard: {e}")),
                        }
                    }
                }
                _ => {}
            },
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    #[test]
    fn flash_expires_after_duration() {
        let mut flash: Option<(String, Instant)> =
            Some(("hello".to_string(), Instant::now() - FLASH_DURATION));
        let active = flash
            .as_ref()
            .filter(|(_, t)| t.elapsed() < FLASH_DURATION)
            .map(|(msg, _)| msg.as_str());
        assert!(active.is_none());

        flash = Some(("hello".to_string(), Instant::now()));
        let active = flash
            .as_ref()
            .filter(|(_, t)| t.elapsed() < FLASH_DURATION)
            .map(|(msg, _)| msg.as_str());
        assert_eq!(active, Some("hello"));
    }

    #[test]
    fn resolve_editor_reads_env() {
        let original = std::env::var("EDITOR").ok();
        unsafe { std::env::set_var("EDITOR", "nano") };
        assert_eq!(resolve_editor(), "nano");
        match original {
            Some(val) => unsafe { std::env::set_var("EDITOR", val) },
            None => unsafe { std::env::remove_var("EDITOR") },
        }
    }

    #[test]
    fn snippet_rendering_produces_lines_and_offsets() {
        let hits = vec![
            SearchHit {
                file_path: PathBuf::from("test.rs"),
                start_line: 10,
                end_line: 15,
                preview: "fn foo() {}\nfn bar() {}\n".to_string(),
                reason: String::new(),
                score: 0.9,
                sources: vec![],
            },
            SearchHit {
                file_path: PathBuf::from("test.rs"),
                start_line: 30,
                end_line: 35,
                preview: "fn baz() {}\n".to_string(),
                reason: String::new(),
                score: 0.5,
                sources: vec![],
            },
        ];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, offsets) = render_snippet_lines(&hits, Some(0), &ps, &ts);
        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0], 0);
        assert!(offsets[1] > 0);
        assert!(!lines.is_empty());
    }
}
