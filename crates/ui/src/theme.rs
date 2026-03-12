//! Theme adapter mapping design tokens to owo_colors styling.
//!
//! All UI components should use these helpers for consistent colors.

use design::ColorRole;
use owo_colors::OwoColorize;

/// Style a string with the given color role.
/// Respects NO_COLOR; returns plain string when colors are disabled.
pub fn style(s: &str, role: ColorRole) -> String {
    if design::no_color() {
        return s.to_string();
    }
    match role {
        ColorRole::Accent => format!("{}", s.cyan()),
        ColorRole::Success => format!("{}", s.green()),
        ColorRole::Error => format!("{}", s.red()),
        ColorRole::Warning => format!("{}", s.yellow()),
        ColorRole::Info => format!("{}", s.blue()),
        ColorRole::Muted | ColorRole::Border => format!("{}", s.bright_black()),
    }
}

/// Style a string with accent and bold (for titles, banner name).
pub fn style_accent_bold(s: &str) -> String {
    if design::no_color() {
        return s.to_string();
    }
    format!("{}", s.bold().cyan())
}

/// Style a string with muted and italic (for suggestions).
pub fn style_muted_italic(s: &str) -> String {
    if design::no_color() {
        return s.to_string();
    }
    format!("{}", s.bright_black().italic())
}
