//! Theme and styles for TUI components.
//! Aligns with appz banner cyan accent.

use ratatui::style::{Color, Modifier, Style};

/// Cyan accent (matches appz banner)
pub const ACCENT: Color = Color::Cyan;
/// Muted/dim text
pub const MUTED: Color = Color::DarkGray;
/// Highlight for selected item
pub const HIGHLIGHT: Color = Color::Cyan;
/// Border color
pub const BORDER: Color = Color::DarkGray;

/// Style for the dialog title
pub fn title() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Style for the selected/highlighted item
pub fn selected() -> Style {
    Style::new().fg(Color::Black).bg(HIGHLIGHT).add_modifier(Modifier::BOLD)
}

/// Style for unselected items
pub fn default_item() -> Style {
    Style::new().fg(Color::White)
}

/// Style for the key hints bar
pub fn hints() -> Style {
    Style::new().fg(MUTED)
}

/// Style for borders
pub fn border() -> Style {
    Style::new().fg(BORDER)
}

/// Create a bordered block with styled title and border
pub fn bordered_block(block_title: &str) -> ratatui::widgets::Block<'_> {
    use ratatui::widgets::Block;
    Block::bordered()
        .border_style(border())
        .title(block_title)
        .title_style(title())
}
