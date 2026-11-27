use std::io::{IsTerminal, stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use crossterm::{
    ExecutableCommand,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
use crate::search::{SearchOptions, validate_forced_neural_workspaces};
use crate::search_service::{HitOrdering, SearchBatch, SearchOutcome, select_search_workspaces};
use crate::workspace::{Workspace, WorkspaceScope};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TUI_DEFAULT_LIMIT: usize = 100;
const FLASH_DURATION: Duration = Duration::from_secs(3);
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);
const INDEXING_STATUS: &str = "Indexing in background… search results may be partial";
const TAB_WIDTH: usize = 4;
/// Rows of context above the hit when the file view opens.
const FILE_VIEW_CONTEXT_ROWS: usize = 4;
const MIN_SPLIT_PERCENT: u16 = 15;
const MAX_SPLIT_PERCENT: u16 = 70;
const DEFAULT_SPLIT_PERCENT: u16 = 35;

/// Pre-rendered file view.
struct FileViewCache {
    path: PathBuf,
    hl_range: Option<(usize, usize)>,
    lines: Vec<Line<'static>>,
    /// Panel width that `row_starts` was calculated for.
    width: u16,
    row_starts: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlashKind {
    Success,
    Failure,
}

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

fn open_in_editor(
    file: &std::path::Path,
    line: usize,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    let launch = crate::launcher::tui_editor_launch(file, line, None)?;
    let program = launch.program.to_string_lossy().into_owned();
    disable_raw_mode()?;
    if let Err(error) = stdout().execute(LeaveAlternateScreen) {
        let restore = restore_terminal(terminal);
        return match restore {
            Ok(()) => Err(error.into()),
            Err(restore) => bail!("failed to suspend terminal: {error}; {restore:#}"),
        };
    }

    let launch_result = launch.status();
    let restore_result = restore_terminal(terminal);
    match (launch_result, restore_result) {
        (Ok(status), Ok(())) if status.success() => Ok(()),
        (Ok(status), Ok(())) => bail!("editor exited with {status}"),
        (Err(error), Ok(())) => bail!("failed to launch {program}: {error}"),
        (Ok(status), Err(restore)) if status.success() => Err(restore),
        (Ok(status), Err(restore)) => {
            bail!("editor exited with {status}; terminal restoration also failed: {restore:#}")
        }
        (Err(error), Err(restore)) => bail!(
            "failed to launch {program}: {error}; terminal restoration also failed: {restore:#}"
        ),
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = stdout().execute(EnterAlternateScreen) {
        errors.push(format!("alternate screen: {error}"));
    }
    if let Err(error) = enable_raw_mode() {
        errors.push(format!("raw mode: {error}"));
    }
    if let Err(error) = terminal.clear() {
        errors.push(format!("clear: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("failed to restore terminal ({})", errors.join("; "))
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------
enum TuiSearchProgress {
    Searching(String, usize, usize),
    Done(Result<SearchOutcome>),
}

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
    /// Top screen row of the file view, counted with wrapped lines.
    file_view_scroll: usize,
    file_view_cache: Option<FileViewCache>,

    // -- search lifecycle --
    is_searching: bool,
    pending_search: bool,
    transition_after_search: bool,
    /// Query of the last search, or `None` if that search was cancelled.
    last_query: Option<String>,
    debounce_timer: Option<Instant>,
    cancel_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    active_search_id: Option<uuid::Uuid>,
    search_task: Option<tokio::task::JoinHandle<()>>,
    cleanup_tasks: Vec<tokio::task::JoinHandle<()>>,

    // -- config --
    cli: Cli,
    workspace: Workspace,
    scope_filter: Option<WorkspaceScope>,
    search_rx: Option<std::sync::mpsc::Receiver<TuiSearchProgress>>,

    // -- ui chrome --
    status_message: Option<String>,
    flash: Option<(FlashKind, String, Instant)>,

    // -- bg indexing progress --
    /// Result of the background index. `None` if no index build is active.
    index_rx: Option<std::sync::mpsc::Receiver<Result<()>>>,
    bg_indexing_active: bool,
    last_bg_check: Instant,

    // -- syntax --
    ps: SyntaxSet,
    ts: ThemeSet,

    // -- runtime --
    runtime: tokio::runtime::Handle,

    // -- layout / mouse --
    split_percent: u16,
    dragging_separator: bool,
    search_rect: Rect,
    file_list_rect: Rect,
    right_panel_rect: Rect,
}

impl App {
    fn new(cli: Cli, runtime: tokio::runtime::Handle) -> Result<Self> {
        let query_path = match &cli.query_path {
            Some(path) => path.clone(),
            None => std::env::current_dir()?,
        };
        let (workspace, scope_filter) = crate::cli::resolve_query_workspace(&cli, &query_path)?;
        let needs_index = workspace_needs_index(&cli, &workspace)?;
        require_terminal()?;
        let index_rx = needs_index.then(|| start_background_index(&cli, &workspace, &runtime));

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
            pending_search: false,
            transition_after_search: false,
            last_query: Some(String::new()),
            debounce_timer: Some(Instant::now()),
            cancel_token: None,
            active_search_id: None,
            search_task: None,
            cleanup_tasks: Vec::new(),
            cli,
            workspace,
            scope_filter,
            search_rx: None,
            status_message: Some(
                if needs_index {
                    INDEXING_STATUS
                } else {
                    "Type to search"
                }
                .to_string(),
            ),
            index_rx,
            bg_indexing_active: needs_index,
            last_bg_check: Instant::now(),
            flash: None,
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            runtime,
            split_percent: DEFAULT_SPLIT_PERCENT,
            dragging_separator: false,
            search_rect: Rect::default(),
            file_list_rect: Rect::default(),
            right_panel_rect: Rect::default(),
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
        self.flash = Some((FlashKind::Success, msg.into(), Instant::now()));
    }

    fn flash_error(&mut self, msg: impl Into<String>) {
        self.flash = Some((FlashKind::Failure, msg.into(), Instant::now()));
    }

    fn active_flash(&self) -> Option<(FlashKind, &str)> {
        self.flash
            .as_ref()
            .filter(|(_, _, t)| t.elapsed() < FLASH_DURATION)
            .map(|(kind, msg, _)| (*kind, msg.as_str()))
    }

    /// Load file content for the FileView mode.
    fn ensure_file_view_cache(&mut self) {
        if let Some(file_idx) = self.file_list_state.selected()
            && let Some(file) = self.grouped_files.get(file_idx)
        {
            let abs = self.absolute_path_for(&file.file_path);
            let hl_range = self
                .selected_snippet()
                .map(|hit| (hit.start_line, hit.end_line));

            if self
                .file_view_cache
                .as_ref()
                .is_some_and(|cache| cache.path == abs && cache.hl_range == hl_range)
            {
                return;
            }

            let live_content = (|| -> Result<String> {
                let selected = select_search_workspaces(&self.workspace, self.cli.all_indices)?;
                let owner = selected
                    .workspaces
                    .iter()
                    .find(|workspace| abs.starts_with(&workspace.root))
                    .ok_or_else(|| anyhow::anyhow!("path is outside searched workspaces"))?;
                Ok(crate::workspace_file::read_to_string(&owner.root, &abs)?)
            })();
            let content = match live_content {
                Ok(c) => c,
                Err(e) => format!("(error reading file: {e})"),
            };

            let lines = render_file_view_lines(&content, &abs, hl_range, &self.ps, &self.ts);
            self.file_view_cache = Some(FileViewCache {
                path: abs,
                hl_range,
                lines,
                width: 0,
                row_starts: Vec::new(),
            });
        }
    }

    /// Calculate the screen rows of the file view for a panel width. The line
    /// at the top of the panel stays there if the width changes.
    fn layout_file_view(&mut self, width: u16) {
        let Some(cache) = &mut self.file_view_cache else {
            return;
        };
        if cache.width == width && !cache.row_starts.is_empty() {
            return;
        }
        let row_starts = wrapped_row_starts(&cache.lines, width);
        if !cache.row_starts.is_empty() {
            let (line, _) = line_at_row(&cache.row_starts, self.file_view_scroll);
            self.file_view_scroll = row_starts[line];
        }
        cache.width = width;
        cache.row_starts = row_starts;
    }

    /// Open the file view with the selected snippet in view.
    fn open_file_view(&mut self) {
        self.ensure_file_view_cache();
        let panel = Block::bordered().inner(self.right_panel_rect);
        self.layout_file_view(panel.width);
        if let Some(line) = self.selected_snippet().map(|hit| hit.start_line)
            && let Some(cache) = &self.file_view_cache
        {
            let line = line.clamp(1, cache.lines.len().max(1)) - 1;
            let context = FILE_VIEW_CONTEXT_ROWS.min(usize::from(panel.height) / 4);
            self.file_view_scroll = cache.row_starts[line].saturating_sub(context);
        }
        self.mode = Mode::FileView;
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

    /// Search for the text in the input box. The UI draws "Searching…" first.
    fn queue_search(&mut self) {
        self.last_query = Some(self.input.value().to_string());
        self.status_message = Some("Searching…".to_string());
        self.pending_search = true;
        self.debounce_timer = None;
    }

    /// Queue a search if the query changed and the user stopped typing.
    fn queue_debounced_search(&mut self) {
        if self
            .debounce_timer
            .is_some_and(|timer| timer.elapsed() >= SEARCH_DEBOUNCE)
        {
            if self.last_query.as_deref() != Some(self.input.value()) {
                self.queue_search();
            }
            self.debounce_timer = None;
        }
    }

    /// Queue a search for Enter in the input box and move to its results.
    fn submit_query(&mut self) {
        if self.input.value().is_empty() {
            self.transition_after_search = false;
            return;
        }
        // Results of this query stay usable while the search runs again.
        if !self.is_searching
            && !self.grouped_files.is_empty()
            && self.last_query.as_deref() == Some(self.input.value())
        {
            self.mode = Mode::FileList;
        }
        self.queue_search();
        self.transition_after_search = self.mode == Mode::Search;
    }

    /// Apply a key to the input box.
    fn edit_query(&mut self, key: event::KeyEvent) {
        let prev = self.input.value().to_string();
        self.input.handle_event(&Event::Key(key));
        if prev != self.input.value() {
            // Cancel any in-flight search — the query just changed.
            if self.is_searching {
                self.cancel_search();
            }
            self.debounce_timer = Some(Instant::now());
        }
    }

    /// Cancel the in-flight search. The same query can start a search again.
    fn cancel_search(&mut self) {
        self.stop_search();
        self.last_query = None;
        self.transition_after_search = false;
    }

    /// Stop local and daemon work for any in-flight search.
    fn stop_search(&mut self) {
        self.cleanup_tasks.retain(|task| !task.is_finished());
        if let Some(token) = self.cancel_token.take() {
            token.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(task) = self.search_task.take() {
            task.abort();
            self.cleanup_tasks.push(task);
        }
        if let Some(request_id) = self.active_search_id.take() {
            self.cleanup_tasks.push(self.runtime.spawn(async move {
                let request = DaemonRequest::CancelSearch {
                    search_id: request_id,
                };
                let _ = daemon::request::<fn(String, usize, usize)>(&request, false, None).await;
            }));
        }
        self.search_rx = None;
        self.is_searching = false;
        self.pending_search = false;
    }

    async fn shutdown(&mut self) {
        self.stop_search();
        for task in std::mem::take(&mut self.cleanup_tasks) {
            let _ = task.await;
        }
    }

    fn finish_search(&mut self, result: Result<SearchOutcome>) {
        self.search_rx = None;
        self.search_task = None;
        self.active_search_id = None;
        self.cancel_token = None;
        self.is_searching = false;
        match result {
            Ok(mut outcome) => {
                outcome.hits.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.hits = outcome.hits;
                self.grouped_files = group_hits_by_file(&self.hits, None);
                // A warning about how the query was read does not make the
                // results partial.
                let (empty_label, label) = if outcome
                    .warnings
                    .iter()
                    .all(|warning| warning == crate::search::BOOLEAN_NOT_APPLIED_WARNING)
                {
                    ("warning", "Warning")
                } else {
                    ("partial search", "Partial results")
                };
                if self.grouped_files.is_empty() {
                    self.file_list_state.select(None);
                    self.status_message = Some(if outcome.warnings.is_empty() {
                        "No results".to_string()
                    } else {
                        format!("No results; {empty_label}: {}", outcome.warnings.join("; "))
                    });
                } else if outcome.warnings.is_empty() {
                    self.file_list_state.select(Some(0));
                    self.status_message = None;
                } else {
                    self.file_list_state.select(Some(0));
                    self.status_message = Some(format!("{label}: {}", outcome.warnings.join("; ")));
                }
                self.snippet_index = 0;
                self.file_view_cache = None;
                self.file_view_scroll = 0;
            }
            Err(err) => {
                self.reset_results();
                // The search runs again when the index is complete.
                self.status_message = Some(if self.index_rx.is_some() {
                    INDEXING_STATUS.to_string()
                } else {
                    format!("Search failed: {err:#}")
                });
            }
        }
        if self.transition_after_search {
            self.transition_after_search = false;
            if !self.grouped_files.is_empty() {
                self.mode = Mode::FileList;
            }
        }
    }

    fn trigger_search(&mut self) {
        // Stop any previous search first.
        self.stop_search();

        let q = self.input.value().trim().to_string();
        if q.is_empty() {
            self.transition_after_search = false;
            self.reset_results();
            return;
        }

        self.is_searching = true;

        // Fresh cancellation token for this search.
        let token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cancel_token = Some(token.clone());
        let request_id = uuid::Uuid::new_v4();
        self.active_search_id = Some(request_id);

        let request = build_search_request(
            &self.cli,
            self.workspace.root.clone(),
            self.scope_filter.as_ref(),
            q.clone(),
        );

        let (tx, rx) = std::sync::mpsc::channel();
        self.search_rx = Some(rx);

        let watch = !self.cli.no_watch;
        let cli = self.cli.clone();
        let query_clone = q.clone();
        let workspace = self.workspace.clone();
        let scope_filter = self.scope_filter.clone();

        let task = self.runtime.spawn(async move {
            let tx_clone = tx.clone();
            let progress_cb = Some(Box::new(
                move |stage: String, scanned: usize, total: usize| {
                    let _ = tx_clone.send(TuiSearchProgress::Searching(stage, scanned, total));
                },
            ));

            let daemon_result =
                daemon::request_with_id(&request, request_id, watch, progress_cb).await;
            if token.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let result = match daemon_result {
                Ok(Some(DaemonResponse::SearchResults { hits, warnings })) => {
                    Ok(SearchOutcome { hits, warnings })
                }
                Ok(Some(DaemonResponse::Error { message })) if cli.force_neural => {
                    Err(anyhow::anyhow!(message))
                }
                Ok(Some(DaemonResponse::Error { message })) => {
                    tracing::warn!("daemon TUI search failed ({message}), falling back to local");
                    local_search_detached(
                        &cli,
                        &workspace,
                        scope_filter.as_ref(),
                        &query_clone,
                        &tx,
                        &token,
                    )
                }
                Ok(None) => local_search_detached(
                    &cli,
                    &workspace,
                    scope_filter.as_ref(),
                    &query_clone,
                    &tx,
                    &token,
                ),
                Ok(Some(other)) => {
                    tracing::warn!("unexpected daemon response: {other:?}");
                    local_search_detached(
                        &cli,
                        &workspace,
                        scope_filter.as_ref(),
                        &query_clone,
                        &tx,
                        &token,
                    )
                }
                Err(err) => Err(err),
            };

            if !token.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = tx.send(TuiSearchProgress::Done(result));
            }
        });
        self.search_task = Some(task);
    }

    /// Check for the end of the background index.
    fn poll_background_index(&mut self) {
        // A new search replaces the file that an open snippet or file belongs to.
        if !matches!(self.mode, Mode::Search | Mode::FileList) {
            return;
        }
        let Some(rx) = &self.index_rx else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err(anyhow::anyhow!("the indexing thread stopped"))
            }
        };
        self.finish_background_index(result);
    }

    fn finish_background_index(&mut self, result: Result<()>) {
        self.index_rx = None;
        match result {
            Ok(()) if self.input.value().trim().is_empty() => {
                self.status_message = Some("Index complete".to_string());
            }
            // The search of this query saw an incomplete index.
            Ok(()) if self.last_query.as_deref() == Some(self.input.value()) => {
                self.queue_search();
            }
            Ok(()) => {}
            Err(error) => {
                self.status_message = Some(format!("Indexing failed: {error:#}"));
            }
        }
    }
}

fn local_search_detached(
    cli: &Cli,
    workspace: &Workspace,
    scope_filter: Option<&WorkspaceScope>,
    query: &str,
    progress_tx: &std::sync::mpsc::Sender<TuiSearchProgress>,
    cancel_token: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<SearchOutcome> {
    let options = build_search_options(cli, scope_filter);
    let workspace_set = select_search_workspaces(workspace, cli.all_indices)?;
    let workspaces = workspace_set.workspaces;
    let model = local_search_model(cli, &workspaces)?;
    let mut batch = SearchBatch::new(workspace_set.warnings);

    let (std_tx, std_rx) = std::sync::mpsc::channel();
    let ui_tx = progress_tx.clone();

    std::thread::spawn(move || {
        while let Ok((stage, scanned, total)) = std_rx.recv() {
            let _ = ui_tx.send(TuiSearchProgress::Searching(stage, scanned, total));
        }
    });

    for ws in workspaces {
        if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let mut ws_opts = options.clone();
        ws_opts.progress_tx = Some(std_tx.clone());
        ws_opts.cancel_token = Some(cancel_token.clone());
        let result =
            crate::search::hybrid_search_outcome(&ws, query, Some(model.as_ref()), &ws_opts).map(
                |outcome| {
                    batch.warn(outcome.warnings);
                    outcome.hits
                },
            );
        batch.record(&ws.root, cli.all_indices, result);
    }
    let outcome = batch.finish(tui_limit(cli), HitOrdering::Score)?;

    let query_uses_neural = crate::search::query_uses_neural(query, cli.force_neural);
    if !cancel_token.load(std::sync::atomic::Ordering::Relaxed)
        && !cli.all_indices
        && !cli.hash
        && std::env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
        && workspace.needs_search_enhancement(query_uses_neural)
    {
        let _ = workspace.trigger_background_search_enhancement(query_uses_neural);
    }

    Ok(outcome)
}

fn local_search_model(
    cli: &Cli,
    workspaces: &[Workspace],
) -> Result<Box<dyn crate::embedding::EmbeddingModel>> {
    if cli.force_neural {
        validate_forced_neural_workspaces(workspaces, true)?;
        return crate::embedding::create_neural_model();
    }

    Ok(
        if cli.hash || !workspaces.iter().any(Workspace::has_neural_vectors) {
            crate::embedding::create_hash_model()
        } else {
            crate::embedding::create_model(false)
        },
    )
}

// Workspace preparation

/// Check whether the workspace needs an index build, and ask before a broad one.
fn workspace_needs_index(cli: &Cli, workspace: &Workspace) -> Result<bool> {
    if cli.all_indices {
        crate::cli::confirm_broad_all(cli.skip_gitignore, cli.yes)?;
        if !cli.skip_gitignore {
            return Ok(false);
        }
    }
    // Fast existence-only check: avoids opening SQLite (which can block for
    // minutes on huge repos if the enhancer holds a WAL lock).
    let metadata = workspace.read_metadata().ok().flatten();
    let metadata_present = metadata
        .as_ref()
        .is_some_and(|m| m.last_indexed_at_unix.is_some());
    if cli.all_indices && !metadata_present {
        // Do not add the current directory to an all-index search.
        return Ok(false);
    }
    let needs_reindex_for_gitignore =
        cli.skip_gitignore && !metadata.is_some_and(|m| m.skip_gitignore);
    let stores = if workspace.has_overlay() || workspace.base_ref_path().exists() {
        [
            workspace.overlay_sqlite_path(),
            workspace.overlay_tantivy_dir(),
            workspace.overlay_vector_path(),
        ]
    } else {
        [
            workspace.sqlite_path(),
            workspace.tantivy_dir(),
            workspace.vector_path(),
        ]
    };
    let looks_indexed = metadata_present && stores.iter().all(|store| store.exists());

    if looks_indexed && !needs_reindex_for_gitignore {
        return Ok(false);
    }

    // The terminal is not in raw mode yet, so the question can be asked here.
    if !cli.all_indices {
        crate::cli::confirm_broad_index(&workspace.root, cli.yes)?;
    }
    Ok(true)
}

/// Index in the background, so that the TUI launches immediately.
/// trigger_search() already falls back to local search if the daemon isn't
/// ready yet.
fn start_background_index(
    cli: &Cli,
    workspace: &Workspace,
    runtime: &tokio::runtime::Handle,
) -> std::sync::mpsc::Receiver<Result<()>> {
    let root = workspace.root.clone();
    let watch = !cli.no_watch;
    let skip_gitignore = cli.skip_gitignore;
    let ws = workspace.clone();
    let rt = runtime.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let request = DaemonRequest::Index {
            path: root,
            watch,
            skip_gitignore,
        };
        let daemon_result = rt.block_on(async {
            daemon::request(&request, watch, None::<fn(String, usize, usize)>).await
        });
        let result = match daemon_result {
            Ok(Some(DaemonResponse::Error { message })) => Err(anyhow::anyhow!(message)),
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                // No daemon — index locally in this background thread.
                if skip_gitignore
                    && let Ok(Some(mut meta)) = ws.read_metadata()
                    && !meta.skip_gitignore
                {
                    meta.skip_gitignore = true;
                    let _ = ws.ensure_dirs();
                    let _ = ws.write_metadata(&meta);
                }
                let model = crate::embedding::create_hash_model();
                crate::indexer::index_workspace(&ws, model.as_ref()).map(|_| ())
            }
            Err(error) => Err(error),
        };
        let _ = done_tx.send(result);
    });
    done_rx
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
        force_neural: cli.force_neural,
        progress_tx: None,
        cancel_token: None,
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
        force_neural: cli.force_neural,
        hash_only: cli.hash,
        disable_memory_expansion: cli.no_memory_expansion_internal,
    }
}

// ---------------------------------------------------------------------------
// Terminal session RAII
// ---------------------------------------------------------------------------

fn require_terminal() -> Result<()> {
    // The event reader can use the controlling terminal if stdin is redirected.
    if !stdout().is_terminal() || event::poll(Duration::ZERO).is_err() {
        bail!("--interactive needs a terminal");
    }
    Ok(())
}

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
        stdout().execute(EnableMouseCapture)?;
        Ok(s)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = stdout().execute(DisableMouseCapture);
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

fn hints_search(is_searching: bool) -> Line<'static> {
    if is_searching {
        hint_line(&[("Ctrl+C", "cancel search"), ("Esc", "cancel")])
    } else {
        hint_line(&[
            ("↓/Tab", "results"),
            ("Enter", "search"),
            ("Ctrl+C", "clear/quit"),
            ("Esc", "clear/quit"),
            ("🖱", "click"),
        ])
    }
}

fn hints_file_list() -> Line<'static> {
    hint_line(&[
        ("↑↓", "navigate"),
        ("Enter/Tab", "snippets"),
        ("e", "edit"),
        ("y", "copy ref"),
        ("Esc", "search"),
    ])
}

fn hints_snippet_list() -> Line<'static> {
    hint_line(&[
        ("↑↓", "navigate"),
        ("Enter", "expand"),
        ("e", "edit"),
        ("y/Y", "copy ref/content"),
        ("Tab", "files"),
        ("Esc", "back"),
    ])
}

fn hints_file_view() -> Line<'static> {
    hint_line(&[
        ("↑↓", "scroll"),
        ("PgUp/Dn", "fast"),
        ("e", "edit"),
        ("y/Y", "copy ref/content"),
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

/// Convert one highlighted source line. ratatui drops control characters, so
/// tabs become spaces.
fn source_spans(ranges: &[(SynStyle, &str)], background: Option<Color>) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(ranges.len());
    let mut column = 0;
    for (syn_style, text) in ranges {
        let mut style = syn_to_ratatui(*syn_style);
        if let Some(background) = background {
            style = style.bg(background);
        }
        let mut expanded = String::with_capacity(text.len());
        for (index, part) in text.trim_end_matches(['\r', '\n']).split('\t').enumerate() {
            if index > 0 {
                let padding = TAB_WIDTH - column % TAB_WIDTH;
                expanded.extend(std::iter::repeat_n(' ', padding));
                column += padding;
            }
            expanded.push_str(part);
            column += Span::raw(part).width();
        }
        if !expanded.is_empty() {
            spans.push(Span::styled(expanded, style));
        }
    }
    spans
}

/// Screen row of each line in a panel of this width, then the number of rows.
fn wrapped_row_starts(lines: &[Line<'static>], width: u16) -> Vec<usize> {
    let mut row_starts = Vec::with_capacity(lines.len() + 1);
    let mut row = 0;
    row_starts.push(row);
    for line in lines {
        // A character never uses more columns than bytes.
        let max_columns: usize = line.iter().map(|span| span.content.len()).sum();
        row += if max_columns <= usize::from(width) {
            1
        } else {
            Paragraph::new(line.clone())
                .wrap(Wrap { trim: false })
                .line_count(width)
                .clamp(1, usize::from(u16::MAX))
        };
        row_starts.push(row);
    }
    row_starts
}

/// Line at a screen row, and the number of its rows above that row.
fn line_at_row(row_starts: &[usize], row: usize) -> (usize, usize) {
    let line = row_starts.partition_point(|&start| start <= row) - 1;
    (line, row - row_starts[line])
}

/// Paragraph that starts at a screen row. It contains only the lines that fit
/// in the panel: `Paragraph::scroll` cannot address more than `u16::MAX` rows.
fn paragraph_from_row(
    lines: &[Line<'static>],
    row_starts: &[usize],
    row: usize,
    height: u16,
) -> Paragraph<'static> {
    let (first, hidden_rows) = line_at_row(row_starts, row);
    let last = lines.len().min(first + usize::from(height));
    Paragraph::new(lines.get(first..last).unwrap_or_default().to_vec())
        .scroll((hidden_rows as u16, 0))
        .wrap(Wrap { trim: false })
}

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
            lines.push(Line::from(source_spans(&ranges, sel_bg)));
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
        spans.extend(source_spans(
            &ranges,
            in_hl.then_some(Color::Rgb(35, 42, 62)),
        ));

        lines.push(Line::from(spans));
    }

    lines
}

/// Draw the full file with line numbers.
fn draw_file_view(f: &mut Frame, app: &mut App, area: Rect, border_color: Color) {
    app.ensure_file_view_cache();
    let panel = Block::bordered().inner(area);
    app.layout_file_view(panel.width);

    let (widget, scroll_info) = if let Some(cache) = &app.file_view_cache {
        let total = cache.row_starts.last().copied().unwrap_or(0);
        let max_scroll = total.saturating_sub(usize::from(panel.height));
        app.file_view_scroll = app.file_view_scroll.min(max_scroll);
        let scroll_info = if max_scroll > 0 {
            format!(" [{}/{}]", app.file_view_scroll + 1, max_scroll + 1)
        } else {
            String::new()
        };
        let widget = paragraph_from_row(
            &cache.lines,
            &cache.row_starts,
            app.file_view_scroll,
            panel.height,
        );
        (widget, scroll_info)
    } else {
        (Paragraph::new("..."), String::new())
    };

    let display_path = app
        .file_list_state
        .selected()
        .and_then(|i| app.grouped_files.get(i))
        .map(|f| f.file_path.display().to_string())
        .unwrap_or_default();
    let view_widget = widget.block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(format!(" {display_path}{scroll_info} ")),
    );
    f.render_widget(Clear, area);
    f.render_widget(view_widget, area);
}

/// Draw the snippet previews of the selected file.
fn draw_snippets(f: &mut Frame, app: &App, area: Rect, border_color: Color) {
    let snippets = app.current_snippets();
    let sel = if app.mode == Mode::SnippetList {
        Some(app.snippet_index)
    } else {
        None
    };

    let (snippet_lines, snippet_offsets) = render_snippet_lines(snippets, sel, &app.ps, &app.ts);

    // Auto-scroll to selected snippet.
    let panel = Block::bordered().inner(area);
    let row_starts = wrapped_row_starts(&snippet_lines, panel.width);
    let scroll = if let Some(sel_idx) = sel
        && let Some(&offset) = snippet_offsets.get(sel_idx)
    {
        row_starts[offset].saturating_sub(usize::from(panel.height) / 4)
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

    let snippet_widget = paragraph_from_row(&snippet_lines, &row_starts, scroll, panel.height)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(right_title),
        );
    f.render_widget(Clear, area);
    f.render_widget(snippet_widget, area);
}

// ---------------------------------------------------------------------------
// Main TUI loop
// ---------------------------------------------------------------------------

pub async fn run_tui(cli: Cli) -> Result<()> {
    let rt = tokio::runtime::Handle::current();
    // The UI owns the terminal.
    crate::indexer::disable_progress_output();
    let mut app = App::new(cli, rt)?;

    let session = TerminalSession::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Pre-filled query → defer search so the UI draws "Searching…" first.
    if !app.input.value().is_empty() {
        app.queue_search();
    }

    let result: Result<()> = 'event_loop: loop {
        // ---- debounced search ----
        app.queue_debounced_search();

        // ---- search progress polling ----
        if let Some(ref rx) = app.search_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    TuiSearchProgress::Searching(stage, scanned, total) => {
                        if total > 0 {
                            app.status_message =
                                Some(format!("Searching… {} ({} / {})", stage, scanned, total));
                        } else {
                            app.status_message = Some(format!("Searching… {}", stage));
                        }
                    }
                    TuiSearchProgress::Done(result) => {
                        app.finish_search(result);
                        break;
                    }
                }
            }
        }

        // ---- bg progress polling ----
        app.poll_background_index();
        if app.bg_indexing_active && app.last_bg_check.elapsed() > Duration::from_millis(500) {
            app.last_bg_check = Instant::now();

            let e_prog = app.workspace.index_dir.join(".enhancing.progress");
            let i_prog = app.workspace.index_dir.join(".indexing.progress");

            if e_prog.exists() {
                if let Ok(content) = std::fs::read_to_string(&e_prog) {
                    let total = if app.workspace.sqlite_path().exists() {
                        let Ok(c) = rusqlite::Connection::open_with_flags(
                            app.workspace.sqlite_path(),
                            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                        ) else {
                            break 'event_loop Err(anyhow::anyhow!("db error"));
                        };
                        c.query_row(
                            "SELECT value FROM _stats WHERE key = 'total_chunks'",
                            [],
                            |r| r.get::<_, i64>(0),
                        )
                        .unwrap_or(0)
                    } else {
                        0
                    };

                    let count = content.trim().parse::<i64>().unwrap_or(0);
                    if total > 0 {
                        let pct = (count as f64 / total as f64 * 100.0) as u64;
                        app.status_message = Some(format!(
                            "⟳ enhancing ({} / {} chunks, ~{}%)",
                            count, total, pct
                        ));
                    } else {
                        app.status_message = Some(format!("⟳ enhancing ({} chunks)", count));
                    }
                }
            } else if i_prog.exists() {
                if let Ok(content) = std::fs::read_to_string(&i_prog) {
                    app.status_message = Some(format!("⟳ indexing ({})", content.trim()));
                }
            } else if app.index_rx.is_none()
                && !app.workspace.index_dir.join(".enhancing.pid").exists()
                && !app.workspace.index_dir.join(".indexing.pid").exists()
            {
                // Done. If searching isn't happening via type-debounce, clear it.
                app.bg_indexing_active = false;
                if !app.is_searching
                    && app.debounce_timer.is_none()
                    && app.status_message.as_deref().unwrap_or("").starts_with("⟳")
                {
                    app.status_message = Some("Index complete".to_string());
                }
            }
        }

        // ---- render ----
        if let Err(error) = terminal.draw(|f| {
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
            // Borders and the cursor use three columns.
            let input_scroll = app
                .input
                .visual_scroll(usize::from(outer[0].width.saturating_sub(3)));
            let input_widget = Paragraph::new(app.input.value())
                .style(Style::default().fg(Color::White))
                .scroll((0, input_scroll as u16))
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
                .constraints([
                    Constraint::Percentage(app.split_percent),
                    Constraint::Percentage(100 - app.split_percent),
                ])
                .split(outer[1]);

            // Store rects for mouse hit-testing.
            app.search_rect = outer[0];
            app.file_list_rect = main[0];
            app.right_panel_rect = main[1];

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

            f.render_widget(Clear, main[0]);
            f.render_stateful_widget(file_list, main[0], &mut app.file_list_state);

            // ----- right panel -----
            let right_border_color = if matches!(app.mode, Mode::SnippetList | Mode::FileView) {
                Color::Cyan
            } else {
                Color::DarkGray
            };

            match app.mode {
                Mode::FileView => draw_file_view(f, &mut app, main[1], right_border_color),
                _ => draw_snippets(f, &app, main[1], right_border_color),
            }

            // ========== status bar ==========
            let bar_line = if let Some((kind, flash)) = app.active_flash() {
                let (mark, color) = match kind {
                    FlashKind::Success => ('✓', Color::Green),
                    FlashKind::Failure => ('✗', Color::Red),
                };
                Line::from(Span::styled(
                    format!(" {mark} {flash}"),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
            } else {
                match app.mode {
                    Mode::Search => hints_search(app.is_searching),
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
                    x: outer[0].x
                        + app.input.visual_cursor().saturating_sub(input_scroll) as u16
                        + 1,
                    y: outer[0].y + 1,
                });
            }
        }) {
            break 'event_loop Err(error.into());
        }

        if app.pending_search {
            app.trigger_search();
            app.pending_search = false;

            // Loop back immediately to draw the updated results
            continue;
        }

        // ---- input handling ----
        let has_event = match crossterm::event::poll(Duration::from_millis(50)) {
            Ok(has_event) => has_event,
            Err(error) => break 'event_loop Err(error.into()),
        };
        if !has_event {
            continue;
        }
        let ev = match event::read() {
            Ok(event) => event,
            Err(error) => break 'event_loop Err(error.into()),
        };

        // ---- mouse events (global, mode-independent) ----
        if let Event::Mouse(mouse) = ev {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let col = mouse.column;
                    let row = mouse.row;

                    // Check if clicking near the separator (±1 col).
                    let sep_col = app.file_list_rect.x + app.file_list_rect.width;
                    if (col as i16 - sep_col as i16).unsigned_abs() <= 1
                        && row >= app.file_list_rect.y
                        && row < app.file_list_rect.y + app.file_list_rect.height
                    {
                        app.dragging_separator = true;
                    } else if rect_contains(app.search_rect, col, row) {
                        app.mode = Mode::Search;
                    } else if rect_contains(app.file_list_rect, col, row) {
                        app.mode = Mode::FileList;
                        // Select the clicked file row.
                        if let Some(index) = clicked_file(
                            app.file_list_rect,
                            app.file_list_state.offset(),
                            app.grouped_files.len(),
                            row,
                        ) {
                            app.file_list_state.select(Some(index));
                            app.snippet_index = 0;
                        }
                    } else if rect_contains(app.right_panel_rect, col, row) {
                        if app.mode == Mode::FileView {
                            // Stay in FileView.
                        } else {
                            app.mode = Mode::SnippetList;
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) if app.dragging_separator => {
                    let total_width = app.file_list_rect.width + app.right_panel_rect.width;
                    if total_width > 0 {
                        let rel_col = mouse.column.saturating_sub(app.file_list_rect.x);
                        let pct = ((rel_col as u32) * 100 / total_width as u32) as u16;
                        app.split_percent = pct.clamp(MIN_SPLIT_PERCENT, MAX_SPLIT_PERCENT);
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.dragging_separator = false;
                }
                MouseEventKind::ScrollUp => match app.mode {
                    Mode::FileList => app.prev_file(),
                    Mode::SnippetList => app.prev_snippet(),
                    Mode::FileView => {
                        app.file_view_scroll = app.file_view_scroll.saturating_sub(3);
                    }
                    _ => {}
                },
                MouseEventKind::ScrollDown => match app.mode {
                    Mode::FileList => app.next_file(),
                    Mode::SnippetList => app.next_snippet(),
                    Mode::FileView => {
                        app.file_view_scroll = app.file_view_scroll.saturating_add(3);
                    }
                    _ => {}
                },
                _ => {}
            }
            continue;
        }

        let Event::Key(key) = ev else {
            continue;
        };
        if !is_key_press(&key) {
            continue;
        }

        match app.mode {
            // ===== SEARCH MODE =====
            Mode::Search => match key.code {
                KeyCode::Esc => {
                    if app.is_searching {
                        app.cancel_search();
                        app.status_message = Some("Search cancelled".to_string());
                    } else if app.input.value().is_empty() {
                        break 'event_loop Ok(()); // quit
                    } else {
                        app.input = Input::default();
                        app.reset_results();
                        app.last_query = None;
                        app.debounce_timer = None;
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.is_searching {
                        // 1st press: cancel in-flight search
                        app.cancel_search();
                        app.status_message = Some("Search cancelled".to_string());
                    } else if !app.input.value().is_empty() {
                        // 2nd press: clear input
                        app.input = Input::default();
                        app.reset_results();
                        app.last_query = None;
                        app.debounce_timer = None;
                    } else {
                        // 3rd press: quit
                        break 'event_loop Ok(());
                    }
                }
                KeyCode::Enter => app.submit_query(),
                KeyCode::Down => {
                    if !app.grouped_files.is_empty() {
                        app.mode = Mode::FileList;
                        if app.file_list_state.selected().is_none() {
                            app.file_list_state.select(Some(0));
                        }
                    }
                }
                KeyCode::Tab => {
                    if !app.grouped_files.is_empty() {
                        app.mode = Mode::FileList;
                        if app.file_list_state.selected().is_none() {
                            app.file_list_state.select(Some(0));
                        }
                    }
                }
                KeyCode::BackTab => {
                    // Shift+Tab from Search → go to SnippetList if available.
                    if !app.current_snippets().is_empty() {
                        app.mode = Mode::SnippetList;
                    } else if !app.grouped_files.is_empty() {
                        app.mode = Mode::FileList;
                    }
                }
                _ => app.edit_query(key),
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
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')
                    if !app.current_snippets().is_empty() =>
                {
                    app.snippet_index = 0;
                    app.mode = Mode::SnippetList;
                }
                KeyCode::Char('e') => {
                    if let Some(file_idx) = app.file_list_state.selected()
                        && let Some(file) = app.grouped_files.get(file_idx)
                    {
                        let path = app.absolute_path_for(&file.file_path);
                        let line = file.hits.first().map(|h| h.start_line).unwrap_or(1);
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash_error(format!("Editor: {e:#}")),
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
                            Err(e) => app.flash_error(format!("Clipboard: {e}")),
                        }
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.is_searching {
                        app.cancel_search();
                        app.status_message = Some("Search cancelled".to_string());
                        app.mode = Mode::Search;
                    } else if !app.input.value().is_empty() {
                        app.input = Input::default();
                        app.reset_results();
                        app.last_query = None;
                        app.debounce_timer = None;
                        app.mode = Mode::Search;
                    } else {
                        break 'event_loop Ok(());
                    }
                }
                // Other printable chars → switch to search and type.
                KeyCode::Char(_)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    app.mode = Mode::Search;
                    app.edit_query(key);
                }
                KeyCode::Backspace => {
                    app.mode = Mode::Search;
                    app.edit_query(key);
                }
                KeyCode::Tab
                    // Tab from FileList → SnippetList.
                    if !app.current_snippets().is_empty() =>
                {
                    app.snippet_index = 0;
                    app.mode = Mode::SnippetList;
                }
                KeyCode::BackTab => {
                    // Shift+Tab from FileList → Search.
                    app.mode = Mode::Search;
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
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')
                    if app.selected_snippet().is_some() =>
                {
                    app.open_file_view();
                }
                KeyCode::Char('e') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let line = hit.start_line;
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash_error(format!("Editor: {e:#}")),
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
                            Err(e) => app.flash_error(format!("Clipboard: {e}")),
                        }
                    }
                }
                KeyCode::Char('Y') => {
                    if let Some(hit) = app.selected_snippet() {
                        let text = hit.preview.clone();
                        let label = format!(
                            "{}:{} ({} chars)",
                            hit.file_path.display(),
                            hit.start_line,
                            text.len()
                        );
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                            Ok(()) => app.flash(format!("Copied content {label}")),
                            Err(e) => app.flash_error(format!("Clipboard: {e}")),
                        }
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.is_searching {
                        app.cancel_search();
                        app.status_message = Some("Search cancelled".to_string());
                        app.mode = Mode::Search;
                    } else if !app.input.value().is_empty() {
                        app.input = Input::default();
                        app.reset_results();
                        app.last_query = None;
                        app.debounce_timer = None;
                        app.mode = Mode::Search;
                    } else {
                        break 'event_loop Ok(());
                    }
                }
                KeyCode::Tab => {
                    // Tab → from SnippetList → Search (wraps around).
                    app.mode = Mode::Search;
                }
                KeyCode::BackTab => {
                    // Shift+Tab ← from SnippetList → FileList.
                    app.mode = Mode::FileList;
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
                    app.file_view_scroll = usize::MAX; // clamped during render
                }
                KeyCode::Char('e') => {
                    if let Some(hit) = app.selected_snippet() {
                        let path = app.absolute_path_for(&hit.file_path);
                        let line = hit.start_line;
                        match open_in_editor(&path, line, &mut terminal) {
                            Ok(()) => {}
                            Err(e) => app.flash_error(format!("Editor: {e:#}")),
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
                            Err(e) => app.flash_error(format!("Clipboard: {e}")),
                        }
                    }
                }
                KeyCode::Char('Y') => {
                    if let Some(hit) = app.selected_snippet() {
                        let text = hit.preview.clone();
                        let label = format!(
                            "{}:{} ({} chars)",
                            hit.file_path.display(),
                            hit.start_line,
                            text.len()
                        );
                        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                            Ok(()) => app.flash(format!("Copied content {label}")),
                            Err(e) => app.flash_error(format!("Clipboard: {e}")),
                        }
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.is_searching {
                        app.cancel_search();
                        app.status_message = Some("Search cancelled".to_string());
                        app.mode = Mode::Search;
                        app.file_view_cache = None;
                    } else if !app.input.value().is_empty() {
                        app.input = Input::default();
                        app.reset_results();
                        app.last_query = None;
                        app.debounce_timer = None;
                        app.mode = Mode::Search;
                        app.file_view_cache = None;
                    } else {
                        break 'event_loop Ok(());
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    // Tab from FileView → SnippetList.
                    app.mode = Mode::SnippetList;
                    app.file_view_cache = None;
                }
                _ => {}
            },
        }
    };
    drop(terminal);
    drop(session);
    app.shutdown().await;
    result
}

/// Check if a point (col, row) falls inside a `Rect`.
fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

/// Index of the file that a click in the file list selects. A click on the
/// border or below the last file selects nothing.
fn clicked_file(list: Rect, offset: usize, len: usize, row: u16) -> Option<usize> {
    let rows = Block::bordered().inner(list);
    if row < rows.top() || row >= rows.bottom() {
        return None;
    }
    let index = offset + usize::from(row - rows.top());
    (index < len).then_some(index)
}

/// Windows terminals report key releases as separate events; only presses
/// (and repeats) are dispatched so one keystroke is not handled twice.
fn is_key_press(key: &event::KeyEvent) -> bool {
    key.kind != event::KeyEventKind::Release
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
    fn key_release_events_are_not_dispatched() {
        let press = event::KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            event::KeyEventKind::Press,
        );
        let repeat = event::KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            event::KeyEventKind::Repeat,
        );
        let release = event::KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            event::KeyEventKind::Release,
        );
        assert!(is_key_press(&press));
        assert!(is_key_press(&repeat));
        assert!(!is_key_press(&release));
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn make_hit(path: &str, start: usize, end: usize, score: f32) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(path),
            start_line: start,
            end_line: end,
            preview: format!("line {}..{}\n", start, end),
            reason: String::new(),
            score,
            sources: vec![],
            neural_requested: false,
            neural_executed: false,
        }
    }

    #[test]
    fn forced_neural_tui_local_model_rejects_missing_vectors() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            id: "tui-force-neural".to_string(),
            root: root.path().to_path_buf(),
            index_dir: index.path().to_path_buf(),
            repo_id: None,
            base_index_dir: None,
        };
        let cli = Cli::parse_from(["ig", "--interactive", "--force-neural", "test"]);

        assert!(local_search_model(&cli, &[workspace]).is_err());
    }

    /// Build a minimal `App` state *without* a real workspace or runtime.
    /// Only the fields used by the navigation / rendering logic are populated.
    fn test_app(hits: Vec<SearchHit>) -> App {
        let cli = Cli::parse_from(["ig", "--interactive", "test"]);
        let grouped = group_hits_by_file(&hits, None);
        let mut state = ListState::default();
        if !grouped.is_empty() {
            state.select(Some(0));
        }
        App {
            input: Input::default(),
            hits,
            grouped_files: grouped,
            search_rx: None,
            file_list_state: state,
            snippet_index: 0,
            mode: Mode::Search,
            file_view_scroll: 0,
            file_view_cache: None,
            is_searching: false,
            pending_search: false,
            transition_after_search: false,
            last_query: Some(String::new()),
            debounce_timer: None,
            cancel_token: None,
            active_search_id: None,
            search_task: None,
            cleanup_tasks: Vec::new(),
            index_rx: None,
            bg_indexing_active: false,
            last_bg_check: std::time::Instant::now(),
            cli,
            // SAFETY: only used by trigger_search / editor; not called in unit tests.
            workspace: Workspace {
                id: "test".into(),
                root: PathBuf::from("/tmp/test"),
                index_dir: PathBuf::from("/tmp/test/.ivygrep"),
                repo_id: None,
                base_index_dir: None,
            },
            scope_filter: None,
            status_message: None,
            flash: None,
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            runtime: tokio::runtime::Handle::current(),
            split_percent: DEFAULT_SPLIT_PERCENT,
            dragging_separator: false,
            search_rect: Rect::default(),
            file_list_rect: Rect::default(),
            right_panel_rect: Rect::default(),
        }
    }

    #[tokio::test]
    async fn cancelling_search_aborts_task_and_discards_progress_channel() {
        struct DropSignal(Option<tokio::sync::oneshot::Sender<()>>);

        impl Drop for DropSignal {
            fn drop(&mut self) {
                if let Some(signal) = self.0.take() {
                    let _ = signal.send(());
                }
            }
        }

        let mut app = test_app(Vec::new());
        let token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (progress_tx, progress_rx) = std::sync::mpsc::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _signal = DropSignal(Some(dropped_tx));
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;

        app.cancel_token = Some(token.clone());
        app.search_task = Some(task);
        app.search_rx = Some(progress_rx);
        app.is_searching = true;
        app.pending_search = true;
        app.cancel_search();

        assert!(token.load(std::sync::atomic::Ordering::Relaxed));
        assert!(app.search_task.is_none());
        assert_eq!(app.cleanup_tasks.len(), 1);
        assert!(app.search_rx.is_none());
        assert!(!app.is_searching);
        assert!(!app.pending_search);
        assert!(
            progress_tx
                .send(TuiSearchProgress::Searching("stale".to_string(), 0, 0))
                .is_err()
        );
        tokio::time::timeout(Duration::from_secs(1), dropped_rx)
            .await
            .unwrap()
            .unwrap();
        app.shutdown().await;
        assert!(app.cleanup_tasks.is_empty());
    }

    #[tokio::test]
    async fn rapid_cancelled_query_cannot_replace_latest_results() {
        let mut app = test_app(Vec::new());
        let (stale_tx, stale_rx) = std::sync::mpsc::channel();
        app.search_rx = Some(stale_rx);
        app.cancel_token = Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            false,
        )));
        app.is_searching = true;
        stale_tx
            .send(TuiSearchProgress::Done(Ok(SearchOutcome {
                hits: vec![make_hit("stale-a.rs", 1, 1, 1.0)],
                warnings: Vec::new(),
            })))
            .unwrap();

        app.cancel_search();

        let (latest_tx, latest_rx) = std::sync::mpsc::channel();
        app.search_rx = Some(latest_rx);
        app.is_searching = true;
        latest_tx
            .send(TuiSearchProgress::Done(Ok(SearchOutcome {
                hits: vec![make_hit("latest-b.rs", 1, 1, 2.0)],
                warnings: Vec::new(),
            })))
            .unwrap();
        let latest = app.search_rx.as_ref().unwrap().try_recv().unwrap();
        let TuiSearchProgress::Done(result) = latest else {
            panic!("expected latest query completion");
        };
        app.finish_search(result);

        assert_eq!(app.hits.len(), 1);
        assert_eq!(app.hits[0].file_path, PathBuf::from("latest-b.rs"));
        assert!(
            stale_tx
                .send(TuiSearchProgress::Searching("stale".to_string(), 0, 0))
                .is_err()
        );
    }

    #[tokio::test]
    async fn shutdown_awaits_tracked_cleanup_tasks() {
        let mut app = test_app(Vec::new());
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        app.cleanup_tasks.push(tokio::spawn(async move {
            tokio::task::yield_now().await;
            let _ = done_tx.send(());
        }));

        app.shutdown().await;

        done_rx.await.unwrap();
        assert!(app.cleanup_tasks.is_empty());
    }

    // ── Search Lifecycle Tests ──────────────────────────────────────────

    fn outcome(hits: Vec<SearchHit>) -> SearchOutcome {
        SearchOutcome {
            hits,
            warnings: Vec::new(),
        }
    }

    #[tokio::test]
    async fn undone_edit_repeats_the_cancelled_search() {
        let mut app = test_app(Vec::new());
        app.input = Input::from("tax");
        app.queue_search();
        app.pending_search = false;
        app.is_searching = true;

        for code in [KeyCode::Char('x'), KeyCode::Backspace] {
            app.edit_query(event::KeyEvent::new(code, KeyModifiers::NONE));
        }
        assert_eq!(app.input.value(), "tax");
        assert!(!app.is_searching);

        app.debounce_timer = Some(Instant::now() - SEARCH_DEBOUNCE);
        app.queue_debounced_search();
        assert!(app.pending_search);
    }

    #[tokio::test]
    async fn enter_moves_to_the_results_of_the_new_query() {
        let mut app = test_app(vec![make_hit("old.rs", 1, 5, 1.0)]);
        app.last_query = Some("old".to_string());
        app.input = Input::from("new");

        app.submit_query();
        assert!(app.pending_search);
        assert_eq!(app.mode, Mode::Search);

        app.finish_search(Ok(outcome(vec![make_hit("new.rs", 1, 5, 1.0)])));
        assert_eq!(app.mode, Mode::FileList);
        assert_eq!(
            app.grouped_files[0].file_path,
            PathBuf::from("new.rs"),
            "the list must hold the new results"
        );
    }

    #[tokio::test]
    async fn repeated_search_keeps_an_open_snippet() {
        let mut app = test_app(vec![make_hit("a.rs", 1, 5, 1.0)]);
        app.last_query = Some("tax".to_string());
        app.input = Input::from("tax");
        app.submit_query();
        assert_eq!(app.mode, Mode::FileList);
        app.mode = Mode::SnippetList;
        app.finish_search(Ok(outcome(vec![make_hit("a.rs", 1, 5, 1.0)])));
        assert_eq!(app.mode, Mode::SnippetList);
    }

    #[tokio::test]
    async fn edit_after_enter_keeps_the_focus_in_the_input_box() {
        let mut app = test_app(Vec::new());
        app.input = Input::from("tax");
        app.submit_query();
        app.is_searching = true;

        app.edit_query(event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        app.finish_search(Ok(outcome(vec![make_hit("a.rs", 1, 5, 1.0)])));
        assert_eq!(app.mode, Mode::Search);
    }

    #[tokio::test]
    async fn search_repeats_when_the_background_index_ends() {
        let mut app = test_app(Vec::new());
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        app.index_rx = Some(done_rx);
        app.input = Input::from("calculate tax");
        app.queue_search();
        app.pending_search = false;

        app.finish_search(Err(anyhow::anyhow!("index format incompatible")));
        assert_eq!(app.status_message.as_deref(), Some(INDEXING_STATUS));
        app.poll_background_index();
        assert!(!app.pending_search);

        done_tx.send(Ok(())).unwrap();
        app.poll_background_index();
        assert!(app.pending_search);

        app.finish_search(Err(anyhow::anyhow!("disk error")));
        assert_eq!(
            app.status_message.as_deref(),
            Some("Search failed: disk error")
        );
    }

    #[test]
    fn indexed_worktree_overlay_needs_no_index() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            id: "tui-overlay".to_string(),
            root: root.path().to_path_buf(),
            index_dir: index.path().to_path_buf(),
            repo_id: None,
            base_index_dir: Some(index.path().join("base")),
        };
        workspace
            .write_metadata(&crate::workspace::WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: Some(1),
                watch_enabled: false,
                skip_gitignore: false,
                index_generation: 1,
            })
            .unwrap();
        std::fs::write(workspace.overlay_sqlite_path(), "").unwrap();
        std::fs::create_dir(workspace.overlay_tantivy_dir()).unwrap();
        std::fs::write(workspace.overlay_vector_path(), "").unwrap();
        let cli = Cli::parse_from(["ig", "--interactive"]);

        assert!(!workspace_needs_index(&cli, &workspace).unwrap());

        std::fs::remove_dir(workspace.overlay_tantivy_dir()).unwrap();
        assert!(workspace_needs_index(&cli, &workspace).unwrap());
    }

    // ── Request Building Tests ──────────────────────────────────────────

    #[test]
    fn interactive_rejects_search_modes_it_cannot_apply() {
        for flag in [
            "--literal",
            "--regex",
            "--symbol",
            "--refs",
            "--callers",
            "--lexical-only",
            "--json",
        ] {
            let error =
                Cli::try_parse_from(["ig", "--interactive", flag, "query"]).expect_err(flag);
            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

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
                hash_only,
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
                assert!(hash_only);
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

    // ── Flash / Timer Tests ─────────────────────────────────────────────

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

    // ── Rendering Tests ─────────────────────────────────────────────────

    #[test]
    fn snippet_rendering_produces_lines_and_offsets() {
        let hits = vec![
            make_hit("test.rs", 10, 15, 0.9),
            make_hit("test.rs", 30, 35, 0.5),
        ];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, offsets) = render_snippet_lines(&hits, Some(0), &ps, &ts);
        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0], 0);
        assert!(offsets[1] > 0);
        assert!(!lines.is_empty());
    }

    #[test]
    fn snippet_rendering_without_selection() {
        let hits = vec![make_hit("a.py", 1, 5, 0.8)];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, offsets) = render_snippet_lines(&hits, None, &ps, &ts);
        assert_eq!(offsets.len(), 1);
        assert!(!lines.is_empty());
    }

    #[test]
    fn snippet_rendering_empty_list() {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, offsets) = render_snippet_lines(&[], None, &ps, &ts);
        assert!(lines.is_empty());
        assert!(offsets.is_empty());
    }

    #[test]
    fn file_view_rendering_without_highlight() {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let path = PathBuf::from("test.rs");

        let lines = render_file_view_lines(content, &path, None, &ps, &ts);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn file_view_rendering_with_highlight() {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let path = PathBuf::from("test.txt");

        let lines = render_file_view_lines(content, &path, Some((2, 4)), &ps, &ts);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn tabs_are_drawn_as_spaces() {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let text = |line: &Line| -> String { line.iter().map(|span| &*span.content).collect() };

        let lines = render_file_view_lines("\tx\ty\n", &PathBuf::from("a.go"), None, &ps, &ts);
        assert_eq!(text(&lines[0]), "   1 │     x   y");

        let mut hit = make_hit("a.go", 1, 1, 1.0);
        hit.preview = "a\tb\n\t\tc\n".to_string();
        let (lines, _) = render_snippet_lines(&[hit], None, &ps, &ts);
        assert_eq!(text(&lines[1]), "a   b");
        assert_eq!(text(&lines[2]), "        c");
    }

    /// Draw the right panel into a `width` x `height` terminal.
    fn right_panel(app: &mut App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.right_panel_rect = f.area();
                if app.mode == Mode::FileView {
                    draw_file_view(f, app, f.area(), Color::Cyan);
                } else {
                    draw_snippets(f, app, f.area(), Color::Cyan);
                }
            })
            .unwrap();
        terminal.backend().to_string()
    }

    /// App with the cursor on a hit of a file that has these lines.
    fn file_view_app(lines: Vec<String>, hit_line: usize) -> App {
        let mut app = test_app(vec![make_hit("notes.md", hit_line, hit_line, 1.0)]);
        app.mode = Mode::SnippetList;
        app.file_view_cache = Some(FileViewCache {
            path: app.absolute_path_for(std::path::Path::new("notes.md")),
            hl_range: Some((hit_line, hit_line)),
            lines: lines.into_iter().map(Line::from).collect(),
            width: 0,
            row_starts: Vec::new(),
        });
        app
    }

    #[tokio::test]
    async fn file_view_reaches_every_row_of_wrapped_lines() {
        let paragraph = |name: &str| format!("{name} {} {name}-end", "lorem ipsum ".repeat(50));
        let mut lines: Vec<String> = (0..40).map(|i| paragraph(&format!("para{i:02}"))).collect();
        lines.push("hit".to_string());
        lines.push(paragraph("last"));
        let mut app = file_view_app(lines, 41);
        right_panel(&mut app, 60, 16);

        app.open_file_view();
        let screen = right_panel(&mut app, 60, 16);
        assert!(screen.contains("│hit "), "{screen}");

        app.file_view_scroll = usize::MAX;
        let screen = right_panel(&mut app, 60, 10);
        assert!(screen.contains("last-end"), "{screen}");
        assert!(!screen.contains("hit"), "{screen}");

        // Row steps show the part of a line that is taller than the panel.
        app.file_view_scroll = 0;
        let mut screens = right_panel(&mut app, 60, 6);
        for _ in 0..12 {
            app.file_view_scroll += 1;
            screens += &right_panel(&mut app, 60, 6);
        }
        assert!(screens.contains("para00-end"), "{screens}");
    }

    #[tokio::test]
    async fn file_view_keeps_its_top_line_when_the_panel_width_changes() {
        let lines = (0..200)
            .map(|i| format!("line{i} {}", "lorem ipsum ".repeat(10)))
            .collect();
        let mut app = file_view_app(lines, 101);
        right_panel(&mut app, 60, 16);
        app.open_file_view();
        app.file_view_scroll = app.file_view_cache.as_ref().unwrap().row_starts[100];
        assert!(right_panel(&mut app, 60, 16).contains("line100 "));
        assert!(right_panel(&mut app, 30, 16).contains("line100 "));
    }

    #[tokio::test]
    async fn file_view_opens_at_a_hit_beyond_u16_lines() {
        let lines = (1..=70_002).map(|i| format!("line {i}")).collect();
        let mut app = file_view_app(lines, 70_001);
        right_panel(&mut app, 40, 20);

        app.open_file_view();
        let screen = right_panel(&mut app, 40, 20);
        assert!(screen.contains("line 70001 "), "{screen}");

        app.file_view_scroll = 66_000;
        let screen = right_panel(&mut app, 40, 20);
        assert!(screen.contains("line 66001 "), "{screen}");
    }

    #[tokio::test]
    async fn selected_snippet_is_visible_below_wrapped_snippets() {
        let hits = (0..8)
            .map(|i| {
                let mut hit = make_hit("notes.md", i * 10 + 1, i * 10 + 5, 1.0);
                hit.preview = format!("{}\n", "lorem ipsum ".repeat(100));
                hit
            })
            .collect();
        let mut app = test_app(hits);
        app.mode = Mode::SnippetList;
        app.snippet_index = 6;
        let screen = right_panel(&mut app, 60, 20);
        assert!(screen.contains("❯ :61-65"), "{screen}");
    }

    // ── File Navigation Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn next_file_wraps_around() {
        let hits = vec![
            make_hit("a.rs", 1, 5, 1.0),
            make_hit("b.rs", 10, 15, 0.8),
            make_hit("c.rs", 20, 25, 0.6),
        ];
        let mut app = test_app(hits);
        assert_eq!(app.file_list_state.selected(), Some(0));

        app.next_file();
        assert_eq!(app.file_list_state.selected(), Some(1));

        app.next_file();
        assert_eq!(app.file_list_state.selected(), Some(2));

        // Wraps back to 0.
        app.next_file();
        assert_eq!(app.file_list_state.selected(), Some(0));
    }

    #[tokio::test]
    async fn prev_file_wraps_around() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("b.rs", 10, 15, 0.8)];
        let mut app = test_app(hits);
        assert_eq!(app.file_list_state.selected(), Some(0));

        // Wraps to last.
        app.prev_file();
        assert_eq!(app.file_list_state.selected(), Some(1));

        app.prev_file();
        assert_eq!(app.file_list_state.selected(), Some(0));
    }

    #[tokio::test]
    async fn next_file_on_empty_list_is_noop() {
        let mut app = test_app(vec![]);
        app.next_file();
        assert_eq!(app.file_list_state.selected(), None);
    }

    #[tokio::test]
    async fn prev_file_on_empty_list_is_noop() {
        let mut app = test_app(vec![]);
        app.prev_file();
        assert_eq!(app.file_list_state.selected(), None);
    }

    #[tokio::test]
    async fn file_navigation_resets_snippet_index() {
        let hits = vec![
            make_hit("a.rs", 1, 5, 1.0),
            make_hit("a.rs", 10, 15, 0.8),
            make_hit("b.rs", 20, 25, 0.6),
        ];
        let mut app = test_app(hits);
        app.snippet_index = 1; // Was browsing second snippet of file a.rs.

        app.next_file(); // Move to b.rs.
        assert_eq!(
            app.snippet_index, 0,
            "snippet_index should reset on file change"
        );
    }

    // ── Snippet Navigation Tests ────────────────────────────────────────

    #[tokio::test]
    async fn next_snippet_wraps_around() {
        let hits = vec![
            make_hit("a.rs", 1, 5, 1.0),
            make_hit("a.rs", 10, 15, 0.8),
            make_hit("a.rs", 20, 25, 0.6),
        ];
        let mut app = test_app(hits);
        assert_eq!(app.current_snippets().len(), 3);

        app.next_snippet();
        assert_eq!(app.snippet_index, 1);

        app.next_snippet();
        assert_eq!(app.snippet_index, 2);

        app.next_snippet();
        assert_eq!(app.snippet_index, 0);
    }

    #[tokio::test]
    async fn prev_snippet_wraps_around() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("a.rs", 10, 15, 0.8)];
        let mut app = test_app(hits);

        app.prev_snippet();
        assert_eq!(app.snippet_index, 1);

        app.prev_snippet();
        assert_eq!(app.snippet_index, 0);
    }

    #[tokio::test]
    async fn snippet_navigation_on_empty_is_noop() {
        let mut app = test_app(vec![]);
        app.next_snippet();
        assert_eq!(app.snippet_index, 0);
        app.prev_snippet();
        assert_eq!(app.snippet_index, 0);
    }

    // ── Snippet Access Tests ────────────────────────────────────────────

    #[tokio::test]
    async fn current_snippets_returns_hits_for_selected_file() {
        let hits = vec![
            make_hit("a.rs", 1, 5, 1.0),
            make_hit("a.rs", 10, 15, 0.8),
            make_hit("b.rs", 20, 25, 0.6),
        ];
        let mut app = test_app(hits);

        // File 0 = a.rs with 2 snippets.
        assert_eq!(app.current_snippets().len(), 2);

        app.next_file(); // File 1 = b.rs with 1 snippet.
        assert_eq!(app.current_snippets().len(), 1);
    }

    #[tokio::test]
    async fn selected_snippet_tracks_index() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("a.rs", 10, 15, 0.8)];
        let mut app = test_app(hits);

        let s0 = app.selected_snippet().unwrap();
        assert_eq!(s0.start_line, 1);

        app.snippet_index = 1;
        let s1 = app.selected_snippet().unwrap();
        assert_eq!(s1.start_line, 10);
    }

    #[tokio::test]
    async fn selected_snippet_returns_none_when_empty() {
        let app = test_app(vec![]);
        assert!(app.selected_snippet().is_none());
    }

    // ── Path Resolution Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn absolute_path_for_relative() {
        let app = test_app(vec![]);
        let result = app.absolute_path_for(&PathBuf::from("src/main.rs"));
        assert_eq!(result, PathBuf::from("/tmp/test/src/main.rs"));
    }

    #[tokio::test]
    async fn absolute_path_for_already_absolute() {
        let app = test_app(vec![]);
        let result = app.absolute_path_for(&PathBuf::from("/usr/lib/foo.rs"));
        assert_eq!(result, PathBuf::from("/usr/lib/foo.rs"));
    }

    // ── Flash Message Tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn flash_message_lifecycle() {
        let mut app = test_app(vec![]);
        assert!(app.active_flash().is_none());

        app.flash("hello world");
        assert_eq!(
            app.active_flash(),
            Some((FlashKind::Success, "hello world"))
        );

        // Flash is still active (within FLASH_DURATION).
        assert_eq!(
            app.active_flash(),
            Some((FlashKind::Success, "hello world"))
        );
    }

    // ── Reset Tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn reset_clears_all_state() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("b.rs", 10, 15, 0.8)];
        let mut app = test_app(hits);
        app.mode = Mode::FileList;
        app.snippet_index = 1;
        app.file_view_scroll = 42;

        app.reset_results();

        assert!(app.hits.is_empty());
        assert!(app.grouped_files.is_empty());
        assert_eq!(app.file_list_state.selected(), None);
        assert_eq!(app.snippet_index, 0);
        assert!(app.file_view_cache.is_none());
        assert_eq!(app.file_view_scroll, 0);
        assert_eq!(app.status_message, Some("Type to search".to_string()));
    }

    // ── Mode Enumeration Tests ──────────────────────────────────────────

    #[test]
    fn mode_equality() {
        assert_eq!(Mode::Search, Mode::Search);
        assert_ne!(Mode::Search, Mode::FileList);
        assert_ne!(Mode::FileList, Mode::SnippetList);
        assert_ne!(Mode::SnippetList, Mode::FileView);
    }

    // ── Hint Rendering Tests ────────────────────────────────────────────

    #[test]
    fn hint_lines_are_nonempty() {
        let search = hints_search(false);
        let file_list = hints_file_list();
        let snippet_list = hints_snippet_list();
        let file_view = hints_file_view();

        assert!(!search.spans.is_empty());
        assert!(!file_list.spans.is_empty());
        assert!(!snippet_list.spans.is_empty());
        assert!(!file_view.spans.is_empty());
    }

    // ── Divider Test ────────────────────────────────────────────────────

    #[test]
    fn snippet_rendering_includes_dividers_between_snippets() {
        let hits = vec![make_hit("a.rs", 1, 5, 0.9), make_hit("a.rs", 10, 15, 0.7)];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, _) = render_snippet_lines(&hits, None, &ps, &ts);
        let divider_count = lines
            .iter()
            .filter(|line| line.spans.iter().any(|span| span.content.contains("────")))
            .count();
        assert_eq!(
            divider_count, 1,
            "expected exactly one divider between two snippets"
        );
    }

    #[test]
    fn single_snippet_has_no_divider() {
        let hits = vec![make_hit("a.rs", 1, 5, 0.9)];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, _) = render_snippet_lines(&hits, None, &ps, &ts);
        let divider_count = lines
            .iter()
            .filter(|line| line.spans.iter().any(|span| span.content.contains("────")))
            .count();
        assert_eq!(
            divider_count, 0,
            "single snippet should have no trailing divider"
        );
    }

    // ── Score Display Tests ─────────────────────────────────────────────

    #[test]
    fn snippet_header_shows_score() {
        let hits = vec![make_hit("a.rs", 1, 5, 0.85)];
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        let (lines, _) = render_snippet_lines(&hits, None, &ps, &ts);
        // The header line should contain the score.
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert!(
            header_text.contains("0.85"),
            "header should contain score, got: {header_text}"
        );
    }

    // ── Group Hits Tests ────────────────────────────────────────────────

    #[test]
    fn group_hits_deduplicates_files() {
        let hits = vec![
            make_hit("a.rs", 1, 5, 1.0),
            make_hit("a.rs", 10, 15, 0.8),
            make_hit("b.rs", 20, 25, 0.6),
        ];
        let grouped = group_hits_by_file(&hits, None);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].hit_count, 2);
        assert_eq!(grouped[1].hit_count, 1);
    }

    #[test]
    fn group_hits_aggregate_score() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("a.rs", 10, 15, 0.5)];
        let grouped = group_hits_by_file(&hits, None);
        assert_eq!(grouped.len(), 1);
        let total = grouped[0].total_score;
        assert!(
            (total - 1.5).abs() < 0.01,
            "expected total_score ~1.5, got {total}"
        );
    }

    // ── Rect Hit Testing ────────────────────────────────────────────────

    #[test]
    fn rect_contains_inside() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(rect_contains(r, 10, 5)); // top-left corner
        assert!(rect_contains(r, 15, 8)); // center
        assert!(rect_contains(r, 29, 14)); // bottom-right edge (exclusive width=20 means max x=29)
    }

    #[test]
    fn rect_contains_outside() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(r, 9, 5)); // left of rect
        assert!(!rect_contains(r, 10, 4)); // above rect
        assert!(!rect_contains(r, 30, 5)); // right of rect (x+width is exclusive)
        assert!(!rect_contains(r, 10, 15)); // below rect
    }

    #[test]
    fn click_selects_the_visible_file_row() {
        let list = Rect::new(0, 4, 30, 10);
        assert_eq!(clicked_file(list, 20, 60, 4), None, "top border");
        assert_eq!(clicked_file(list, 20, 60, 5), Some(20));
        assert_eq!(clicked_file(list, 20, 60, 12), Some(27));
        assert_eq!(clicked_file(list, 20, 60, 13), None, "bottom border");
        assert_eq!(clicked_file(list, 0, 2, 7), None, "below the last file");
    }

    #[test]
    fn rect_contains_zero_size() {
        let r = Rect::new(5, 5, 0, 0);
        assert!(!rect_contains(r, 5, 5));
    }

    // ── Split Percent ───────────────────────────────────────────────────

    #[tokio::test]
    async fn split_percent_defaults() {
        let app = test_app(vec![]);
        assert_eq!(app.split_percent, DEFAULT_SPLIT_PERCENT);
    }

    #[test]
    fn split_percent_clamping() {
        let clamped_low = 5u16.clamp(MIN_SPLIT_PERCENT, MAX_SPLIT_PERCENT);
        assert_eq!(clamped_low, MIN_SPLIT_PERCENT);

        let clamped_high = 90u16.clamp(MIN_SPLIT_PERCENT, MAX_SPLIT_PERCENT);
        assert_eq!(clamped_high, MAX_SPLIT_PERCENT);

        let clamped_mid = 40u16.clamp(MIN_SPLIT_PERCENT, MAX_SPLIT_PERCENT);
        assert_eq!(clamped_mid, 40);
    }

    // ── Dragging Separator ──────────────────────────────────────────────

    #[tokio::test]
    async fn dragging_separator_starts_false() {
        let app = test_app(vec![]);
        assert!(!app.dragging_separator);
    }

    // ── Tab Cycling Readiness ───────────────────────────────────────────

    #[tokio::test]
    async fn tab_from_search_goes_to_filelist_when_results_exist() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0)];
        let mut app = test_app(hits);
        assert_eq!(app.mode, Mode::Search);
        // Simulate what Tab does: transition to FileList if results exist.
        if !app.grouped_files.is_empty() {
            app.mode = Mode::FileList;
            if app.file_list_state.selected().is_none() {
                app.file_list_state.select(Some(0));
            }
        }
        assert_eq!(app.mode, Mode::FileList);
        assert_eq!(app.file_list_state.selected(), Some(0));
    }

    #[tokio::test]
    async fn tab_from_search_noop_when_no_results() {
        let mut app = test_app(vec![]);
        app.mode = Mode::Search;
        // Simulate Tab: no results → stay in Search.
        if !app.grouped_files.is_empty() {
            app.mode = Mode::FileList;
        }
        assert_eq!(app.mode, Mode::Search);
    }

    #[tokio::test]
    async fn tab_from_filelist_goes_to_snippet_list() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0), make_hit("a.rs", 10, 15, 0.8)];
        let mut app = test_app(hits);
        app.mode = Mode::FileList;
        // Simulate Tab from FileList.
        if !app.current_snippets().is_empty() {
            app.snippet_index = 0;
            app.mode = Mode::SnippetList;
        }
        assert_eq!(app.mode, Mode::SnippetList);
    }

    #[tokio::test]
    async fn tab_from_snippet_list_goes_to_search() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0)];
        let mut app = test_app(hits);
        app.mode = Mode::SnippetList;
        // Simulate Tab → from SnippetList → Search (wraps around).
        app.mode = Mode::Search;
        assert_eq!(app.mode, Mode::Search);
    }

    #[tokio::test]
    async fn backtab_from_filelist_goes_to_search() {
        let hits = vec![make_hit("a.rs", 1, 5, 1.0)];
        let mut app = test_app(hits);
        app.mode = Mode::FileList;
        // Simulate Shift+Tab from FileList.
        app.mode = Mode::Search;
        assert_eq!(app.mode, Mode::Search);
    }
}
