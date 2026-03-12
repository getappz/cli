//! Empty state display components.

use crate::layout;
use crate::theme;
use design::ColorRole;
use miette::Result;
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
    println!("{}", theme::style(message, ColorRole::Muted));

    if let Some(suggestion) = suggestion {
        println!("  {}", theme::style_muted_italic(suggestion));
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
    println!(
        "{} {}",
        theme::style(icon, ColorRole::Muted),
        theme::style(message, ColorRole::Muted)
    );

    if let Some(suggestion) = suggestion {
        println!("  {}", theme::style_muted_italic(suggestion));
    }

    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}
