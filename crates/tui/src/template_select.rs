//! Template selector with search and custom git/npm/path input.

use crate::terminal;
use crate::theme;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{ListItem, List, ListState, Paragraph},
    Frame,
};
use std::io;

/// Check if the typed string looks like a custom template source (URL, npm, or path).
/// Returns (is_custom, normalized_value) - for npm packages without prefix, we add "npm:".
fn parse_custom_source(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with("http://") || s.starts_with("https://")
        || s.contains("github.com") || s.contains("gitlab.com") || s.contains("bitbucket.")
        || s.starts_with("npm:")
        || s.starts_with("./") || s.starts_with("../") || s.starts_with('/')
        || (s.contains('/') && !s.contains(' ') && s.len() > 3)
    {
        return Some(s.to_string());
    }
    // Plain package name (no slashes, no colons) -> treat as npm
    if !s.contains('/') && !s.contains(':') && s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') && s.len() > 1
    {
        return Some(format!("npm:{}", s));
    }
    None
}

/// Select a template with search. Options are (display, value) pairs.
/// Returns the selected value, or the custom-typed string if "Use: ..." was selected.
pub fn select_template(
    title: &str,
    options: &[(String, String)],
) -> io::Result<Option<String>> {
    if options.is_empty() {
        return Ok(None);
    }

    let mut terminal = terminal::init()?;
    let mut state = SelectState::new(options);

    let result = run_template_select(&mut terminal, title, &mut state);

    terminal::restore();
    result
}

struct SelectState<'a> {
    options: &'a [(String, String)],
    input: String,
    input_cursor: usize,
    list_state: ListState,
}

impl<'a> SelectState<'a> {
    fn new(options: &'a [(String, String)]) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            options,
            input: String::new(),
            input_cursor: 0,
            list_state,
        }
    }

    fn filtered_options(&self) -> Vec<(String, String)> {
        let search = self.input.to_lowercase();
        let mut out = Vec::new();
        if let Some(custom_val) = parse_custom_source(&self.input) {
            out.push((format!("Use: {}", custom_val), custom_val));
        }
        for (disp, val) in self.options.iter() {
            if search.is_empty()
                || disp.to_lowercase().contains(&search)
                || val.to_lowercase().contains(&search)
            {
                out.push((disp.clone(), val.clone()));
            }
        }
        out
    }

    fn select_prev(&mut self) {
        let items = self.filtered_options();
        if items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let prev = i.saturating_sub(1);
        self.list_state.select(Some(prev.min(items.len().saturating_sub(1))));
    }

    fn select_next(&mut self) {
        let items = self.filtered_options();
        if items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let next = (i + 1).min(items.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }
}

fn run_template_select(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>,
    title: &str,
    state: &mut SelectState<'_>,
) -> io::Result<Option<String>> {
    loop {
        terminal.draw(|frame| draw_template_select(frame, title, &mut *state))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Enter => {
                        let items = state.filtered_options();
                        if let Some(idx) = state.list_state.selected() {
                            if let Some((_, value)) = items.get(idx) {
                                return Ok(Some(value.clone()));
                            }
                        }
                    }
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Up | KeyCode::Char('k') => state.select_prev(),
                    KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                    KeyCode::Home | KeyCode::Char('g') => {
                        state.list_state.select(Some(0));
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        let items = state.filtered_options();
                        let n = items.len().saturating_sub(1);
                        state.list_state.select(Some(n));
                    }
                    KeyCode::Backspace => {
                        if state.input_cursor > 0 {
                            state.input_cursor -= 1;
                            state.input.remove(state.input_cursor);
                            state.list_state.select(Some(0));
                        }
                    }
                    KeyCode::Char(c) => {
                        state.input.insert(state.input_cursor, c);
                        state.input_cursor += 1;
                        state.list_state.select(Some(0));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn draw_template_select(
    frame: &mut Frame,
    title: &str,
    state: &mut SelectState<'_>,
) {
    let width = 70;
    let height = 24;
    let area = centered_rect(width, height, frame.area());
    let block = theme::bordered_block(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    // Search input
    let input_display = if state.input.is_empty() {
        "Type to search or enter git URL / npm:package / path..."
    } else {
        state.input.as_str()
    };
    let input_line = Line::from(vec![
        Span::styled("> ", theme::selected()),
        Span::raw(input_display),
        Span::styled(" ", theme::default_item()),
    ]);
    let input_para = Paragraph::new(input_line);
    frame.render_widget(input_para, chunks[0]);

    // Filtered list
    let visible = state.filtered_options();
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, (disp, _))| {
            let prefix = if state.list_state.selected() == Some(i) {
                "> "
            } else {
                "  "
            };
            let line = Line::from(format!("{}{}", prefix, disp));
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], &mut state.list_state);

    let hints = Paragraph::new(Line::from(
        "↑↓ navigate  Type to search  Enter git/npm/path or select  Esc cancel",
    ))
    .style(theme::hints());
    frame.render_widget(hints, chunks[2]);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    let popup_layout = Layout::vertical([
        Constraint::Length((r.height.saturating_sub(h)) / 2),
        Constraint::Length(h),
        Constraint::Min(0),
    ])
    .split(r);

    let inner = Layout::horizontal([
        Constraint::Length((r.width.saturating_sub(w)) / 2),
        Constraint::Length(w),
        Constraint::Min(0),
    ])
    .split(popup_layout[1]);

    inner[1]
}
