//! Empty state display components.

use crate::layout;
use miette::Result;
use owo_colors::OwoColorize;
use std::io::{self, Write};

/// Display an empty state message.
///
/// # Arguments
/// * `message` - Main empty state message
/// * `suggestion` - Optional suggestion or help text
///
/// # Returns
/// `Result` indicating success or failure
pub fn display(message: &str, suggestion: Option<&str>) -> Result<()> {
    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    println!("{}", message.bright_black());

    if let Some(suggestion) = suggestion {
        println!("  {}", suggestion.bright_black().italic());
    }

    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}

/// Display an empty state with an icon.
///
/// # Arguments
/// * `icon` - Icon character or emoji
/// * `message` - Main empty state message
/// * `suggestion` - Optional suggestion or help text
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_with_icon(icon: &str, message: &str, suggestion: Option<&str>) -> Result<()> {
    println!("{} {}", icon.bright_black(), message.bright_black());

    if let Some(suggestion) = suggestion {
        println!("  {}", suggestion.bright_black().italic());
    }

    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}
