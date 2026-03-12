//! Theme and styles for TUI components.
//! Uses design tokens for consistency with line-based UI.

use design::ColorRole;
use ratatui::style::{Color, Modifier, Style};

/// Map design ColorRole to ratatui Color.
fn role_to_color(role: ColorRole) -> Color {
    match role {
        ColorRole::Accent => Color::Cyan,
        ColorRole::Success => Color::Green,
        ColorRole::Error => Color::Red,
        ColorRole::Warning => Color::Yellow,
        ColorRole::Info => Color::Blue,
        ColorRole::Muted | ColorRole::Border => Color::DarkGray,
    }
}

/// Style for the dialog title
pub fn title() -> Style {
    Style::new()
        .fg(role_to_color(ColorRole::Accent))
        .add_modifier(Modifier::BOLD)
}

/// Style for the selected/highlighted item
pub fn selected() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(role_to_color(ColorRole::Accent))
        .add_modifier(Modifier::BOLD)
}

/// Style for unselected items
pub fn default_item() -> Style {
    Style::new().fg(Color::White)
}

/// Style for the key hints bar
pub fn hints() -> Style {
    Style::new().fg(role_to_color(ColorRole::Muted))
}

/// Style for borders
pub fn border() -> Style {
    Style::new().fg(role_to_color(ColorRole::Border))
}

/// Create a bordered block with styled title and border
pub fn bordered_block(block_title: &str) -> ratatui::widgets::Block<'_> {
    use ratatui::widgets::Block;
    Block::bordered()
        .border_style(border())
        .title(block_title)
        .title_style(title())
}
