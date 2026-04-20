use std::io::{self, stdout};
use std::time::{Duration, Instant};
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::cli::Cli;
use crate::daemon;
use crate::protocol::{DaemonRequest, DaemonResponse, SearchHit};

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
    ps: SyntaxSet,
    ts: ThemeSet,
    runtime: tokio::runtime::Handle,
}

impl App {
    fn new(cli: Cli, runtime: tokio::runtime::Handle) -> Self {
        Self {
            input: Input::default().with_value(cli.query.clone().unwrap_or_default()),
            hits: Vec::new(),
            list_state: ListState::default(),
            is_searching: false,
            last_query: String::new(),
            debounce_timer: Some(Instant::now()),
            cli,
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            runtime,
        }
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
            self.is_searching = false;
            return;
        }

        self.is_searching = true;
        
        let path = match &self.cli.query_path {
            Some(p) => Some(p.clone()),
            None => Some(std::env::current_dir().unwrap_or_default()),
        };

        let request = DaemonRequest::Search {
            path,
            query: q,
            limit: Some(100),
            context: 3,
            type_filter: self.cli.type_filter.clone(),
            include_globs: self.cli.include.clone(),
            exclude_globs: self.cli.exclude.clone(),
            scope_path: None,
            scope_is_file: false,
            skip_gitignore: self.cli.skip_gitignore,
        };

        let resp = tokio::task::block_in_place(|| {
            self.runtime.block_on(async {
                daemon::request(&request, false).await
            })
        });

        self.is_searching = false;
        
        if let Ok(Some(DaemonResponse::SearchResults { mut hits })) = resp {
            hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            self.hits = hits;
            if !self.hits.is_empty() {
                self.list_state.select(Some(0));
            } else {
                self.list_state.select(None);
            }
        } else {
            self.hits.clear();
            self.list_state.select(None);
        }
    }
}

pub async fn run_tui(cli: Cli) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let rt = tokio::runtime::Handle::current();
    let mut app = App::new(cli, rt);
    
    // Immediate search if query provided
    if !app.input.value().is_empty() {
        app.trigger_search();
        app.last_query = app.input.value().to_string();
    }

    loop {
        if let Some(timer) = app.debounce_timer {
            if timer.elapsed() >= Duration::from_millis(300) {
                let current_query = app.input.value().to_string();
                if current_query != app.last_query {
                    app.last_query = current_query;
                    app.trigger_search();
                }
                app.debounce_timer = None;
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(f.area());

            let input_style = Style::default().fg(Color::Cyan);
            let input_widget = Paragraph::new(app.input.value())
                .style(input_style)
                .block(Block::default().borders(Borders::ALL).title(Span::styled(
                    " ⟐ ivygrep ",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )));
            f.render_widget(input_widget, chunks[0]);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(chunks[1]);

            // Hits list
            let items: Vec<ListItem> = app
                .hits
                .iter()
                .map(|hit| {
                    let filename = hit.file_path.file_name().unwrap_or_default().to_string_lossy();
                    let path_dir = hit.file_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                    let content = Line::from(vec![
                        Span::styled(format!("{} ", filename), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled(format!(":{}", hit.start_line), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("  {} - {:.2}", path_dir, hit.score), Style::default().fg(Color::DarkGray)),
                    ]);
                    ListItem::new(content)
                })
                .collect();

            let hits_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Results "))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Rgb(40, 40, 40)))
                .highlight_symbol("❯ ");

            f.render_stateful_widget(hits_list, main_chunks[0], &mut app.list_state);

            // Preview
            let preview_block = Block::default().borders(Borders::ALL).title(" Preview ");
            if let Some(selected) = app.list_state.selected() {
                if let Some(hit) = app.hits.get(selected) {
                    let syntax = app.ps.find_syntax_by_extension(
                        hit.file_path.extension().unwrap_or_default().to_str().unwrap_or("")
                    ).unwrap_or_else(|| app.ps.find_syntax_plain_text());
                    
                    // We'll use base16-ocean.dark if available, otherwise fallback
                    let theme = app.ts.themes.get("base16-ocean.dark").or_else(|| app.ts.themes.values().next()).unwrap();
                    let mut h = HighlightLines::new(syntax, theme);
                    
                    let mut lines = Vec::new();
                    for line in syntect::util::LinesWithEndings::from(&hit.preview) {
                        let ranges: Vec<(SynStyle, &str)> = h.highlight_line(line, &app.ps).unwrap_or_default();
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

        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
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
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
