//! Banner component for displaying app name and version on startup.

use owo_colors::OwoColorize;
use std::io::{self, Write};

/// Display a compact banner with app name and version.
/// Single line, minimal space usage.
///
/// # Arguments
/// * `name` - Application name
/// * `version` - Version string
/// * `tagline` - Optional tagline/subtitle
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn display(name: &str, version: &str, tagline: Option<&str>) -> io::Result<()> {
    // Check if we should display colors
    let use_colors = std::env::var("NO_COLOR").is_err();

    if let Some(tagline) = tagline {
        if use_colors {
            println!(
                "{} {}  {}",
                name.bold().cyan(),
                version.bright_black(),
                tagline.bright_black()
            );
        } else {
            println!("{} {}  {}", name, version, tagline);
        }
    } else if use_colors {
        println!("{} {}", name.bold().cyan(), version.bright_black());
    } else {
        println!("{} {}", name, version);
    }

    // Add two blank lines after banner
    println!();
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
    let use_colors = std::env::var("NO_COLOR").is_err();

    let content = if let Some(tagline) = tagline {
        format!("{} {}  {}", name, version, tagline)
    } else {
        format!("{} {}", name, version)
    };

    let width = content.len().max(40);

    // Top border
    if use_colors {
        println!("{}", "─".repeat(width + 2).bright_black());
        print!(" ");
        print!("{}", name.bold().cyan());
        print!(" {}", version.bright_black());
        if let Some(tagline) = tagline {
            print!("  {}", tagline.bright_black());
        }
        println!();
        println!("{}", "─".repeat(width + 2).bright_black());
    } else {
        println!("{}", "─".repeat(width + 2));
        println!(" {} {} {}", name, version, tagline.unwrap_or(""));
        println!("{}", "─".repeat(width + 2));
    }

    io::stdout().flush()
}

/// Check if we should display the banner (interactive terminal).
/// Uses a simple heuristic: check if NO_COLOR is set or if output might be piped.
pub fn should_display() -> bool {
    // Don't display if NO_COLOR is explicitly set
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // For now, always display unless explicitly disabled
    // In the future, we could add atty crate for better terminal detection
    true
}
