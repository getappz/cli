//! Yes/No confirmation dialog.

use crate::terminal;
use crate::theme;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Wrap},
    Frame,
};
use std::io;

/// Run a confirmation dialog. Returns `true` for Yes, `false` for No/Esc.
pub fn confirm(prompt: &str, default: bool) -> io::Result<bool> {
    let mut terminal = terminal::init()?;
    let mut selected = default; // true = Yes, false = No

    let result = run_confirm(&mut terminal, prompt, &mut selected);

    terminal::restore();
    result
}

fn run_confirm(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>,
    prompt: &str,
    selected: &mut bool,
) -> io::Result<bool> {
    loop {
        terminal.draw(|frame| draw_confirm(frame, prompt, *selected))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Enter => return Ok(*selected),
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Left | KeyCode::Char('h') => *selected = true,
                    KeyCode::Right | KeyCode::Char('l') => *selected = false,
                    KeyCode::Char('y') => return Ok(true),
                    KeyCode::Char('n') => return Ok(false),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn draw_confirm(frame: &mut Frame, prompt: &str, selected: bool) {
    let width = 50;
    let height = 7;
    let area = centered_rect(width, height, frame.area());
    let block = theme::bordered_block(prompt);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let yes_style = if selected {
        theme::selected()
    } else {
        theme::default_item()
    };
    let no_style = if selected {
        theme::default_item()
    } else {
        theme::selected()
    };

    let line = Line::from(vec![
        ratatui::text::Span::raw("   "),
        ratatui::text::Span::styled(" Yes ", yes_style),
        ratatui::text::Span::raw("    "),
        ratatui::text::Span::styled(" No ", no_style),
    ]);

    let paragraph = Paragraph::new(line)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, chunks[0]);

    let hints = Paragraph::new(Line::from("y Yes  n No  Enter confirm  Esc cancel"))
        .style(theme::hints());
    frame.render_widget(hints, chunks[1]);
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
