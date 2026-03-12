//! Semantic color roles for consistent styling.
//!
//! Maps to ANSI standard colors for terminal consistency.
//! - Accent: Primary brand (cyan, 8-bit 51)
//! - Success: Green (10)
//! - Error: Red (9)
//! - Warning: Yellow (11)
//! - Info: Blue (12)
//! - Muted: Dim/secondary (bright black, 240)
//! - Border: Borders, separators (240)

/// Semantic color roles used across UI and TUI.
///
/// Implementations in crates/ui (owo_colors) and crates/tui (ratatui)
/// map these to their respective color systems.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorRole {
    /// Primary brand color (cyan)
    Accent,

    /// Success state (green)
    Success,

    /// Error state (red)
    Error,

    /// Warning state (yellow)
    Warning,

    /// Informational (blue)
    Info,

    /// Dim/secondary text (bright black)
    Muted,

    /// Borders and separators
    Border,
}

/// Check if colors should be disabled (respects NO_COLOR env).
///
/// See: https://no-color.org/
pub fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
}
