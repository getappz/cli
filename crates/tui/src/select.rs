//! Single-select picker component.

use crate::terminal;
use crate::theme;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{ListItem, List, ListState, Paragraph},
    Frame,
};
use std::io;

/// Run a single-select picker. Returns `Some(usize)` for the selected index on Enter,
/// or `None` if the user cancelled (Esc).
pub fn select(title: &str, options: &[String]) -> io::Result<Option<usize>> {
    if options.is_empty() {
        return Ok(None);
    }

    let mut terminal = terminal::init()?;
    let mut state = ListState::default().with_selected(Some(0));

    let result = run_select(&mut terminal, title, options, &mut state);

    terminal::restore();
    result
}

fn run_select(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>,
    title: &str,
    options: &[String],
    state: &mut ListState,
) -> io::Result<Option<usize>> {
    loop {
        terminal.draw(|frame| draw_select(frame, title, options, state))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Enter => {
                        return Ok(state.selected());
                    }
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(1);
                        state.select(Some(prev));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = state.selected().unwrap_or(0);
                        let next = (i + 1).min(options.len().saturating_sub(1));
                        state.select(Some(next));
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        state.select(Some(0));
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        state.select(Some(options.len().saturating_sub(1)));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn draw_select(
    frame: &mut Frame,
    title: &str,
    options: &[String],
    state: &mut ListState,
) {
    let area = centered_rect(60, 50, frame.area());
    let block = theme::bordered_block(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let prefix = if state.selected() == Some(i) { "> " } else { "  " };
            let line = Line::from(format!("{}{}", prefix, s));
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[0], state);

    let hints = Paragraph::new(Line::from("↑↓ jk navigate  Enter select  Esc cancel"))
        .style(theme::hints());
    frame.render_widget(hints, chunks[1]);
}

/// Create a centered rect with given width and height (in character cells)
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
