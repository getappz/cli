//! Terminal init/restore using stderr.
//! Using stderr allows the TUI to work when stdout is piped (e.g. cargo run in an IDE).

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stderr};

/// Initialize terminal with stderr as output (works when stdout is piped).
pub fn init() -> io::Result<Terminal<CrosstermBackend<Stderr>>> {
    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stderr());
    Terminal::new(backend)
}

/// Restore terminal state.
pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stderr(), LeaveAlternateScreen);
}
