//! Banner component for displaying app name and version on startup.

use crate::theme;
use design::{layout, no_color};
use std::io::{self, Write};

/// Display a compact banner with app name and version.
/// Single line, minimal and professional.
///
/// # Arguments
/// * `name` - Application name
/// * `version` - Version string
/// * `tagline` - Optional tagline/subtitle
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn display(name: &str, version: &str, tagline: Option<&str>) -> io::Result<()> {
    let use_colors = !no_color();

    if let Some(tagline) = tagline {
        if use_colors {
            println!(
                "{} {}  {}  {}",
                theme::style_accent_bold(name),
                theme::style(version, design::ColorRole::Muted),
                theme::style("·", design::ColorRole::Muted),
                theme::style(tagline, design::ColorRole::Muted)
            );
        } else {
            println!("{} {}  ·  {}", name, version, tagline);
        }
    } else if use_colors {
        println!(
            "{} {}",
            theme::style_accent_bold(name),
            theme::style(version, design::ColorRole::Muted)
        );
    } else {
        println!("{} {}", name, version);
    }

    println!();

    io::stdout().flush()
}

/// Display a minimal single-line banner.
///
/// # Arguments
/// * `name` - Application name
/// * `version` - Version string
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn display_minimal(name: &str, version: &str) -> io::Result<()> {
    display(name, version, None)
}

/// Display a compact banner with a subtle border.
///
/// # Arguments
/// * `name` - Application name
/// * `version` - Version string
/// * `tagline` - Optional tagline/subtitle
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn display_with_border(name: &str, version: &str, tagline: Option<&str>) -> io::Result<()> {
    let use_colors = !no_color();

    let content = if let Some(tagline) = tagline {
        format!("{} {}  {}", name, version, tagline)
    } else {
        format!("{} {}", name, version)
    };

    let width = content.len().max(40);
    let border_char = layout::SEPARATOR_CHAR.to_string();

    // Top border
    if use_colors {
        println!(
            "{}",
            theme::style(&border_char.repeat(width + 2), design::ColorRole::Border)
        );
        print!(" ");
        print!("{}", theme::style_accent_bold(name));
        print!(" {}", theme::style(version, design::ColorRole::Muted));
        if let Some(tagline) = tagline {
            print!("  {}", theme::style(tagline, design::ColorRole::Muted));
        }
        println!();
        println!(
            "{}",
            theme::style(&border_char.repeat(width + 2), design::ColorRole::Border)
        );
    } else {
        println!("{}", border_char.repeat(width + 2));
        println!(" {} {} {}", name, version, tagline.unwrap_or(""));
        println!("{}", border_char.repeat(width + 2));
    }

    io::stdout().flush()
}

/// Check if we should display the banner (interactive terminal).
/// Uses a simple heuristic: check if NO_COLOR is set or if output might be piped.
pub fn should_display() -> bool {
    // Don't display if NO_COLOR is explicitly set
    if no_color() {
        return false;
    }

    // For now, always display unless explicitly disabled
    // In the future, we could add atty crate for better terminal detection
    true
}
