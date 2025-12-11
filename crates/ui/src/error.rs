//! Error display and formatting utilities.

use crate::layout;
use miette::Result;
use owo_colors::OwoColorize;
use std::error::Error;
use std::fmt::Display;
use std::io::{self, Write};

/// Display a formatted error message.
///
/// # Arguments
/// * `message` - Error message
/// * `error` - Optional underlying error
///
/// # Returns
/// `Result` indicating success or failure
pub fn display(message: &str, error: Option<&dyn Error>) -> Result<()> {
    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    eprintln!("{} {}", "✗".red(), message.red());

    if let Some(err) = error {
        eprintln!("  {}", err.to_string().bright_red());
    }

    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    io::stderr()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stderr: {}", e))?;
    Ok(())
}

/// Display a formatted error message with a custom prefix.
///
/// # Arguments
/// * `prefix` - Custom prefix (e.g., "Failed to connect")
/// * `message` - Error message
/// * `error` - Optional underlying error
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_with_prefix(prefix: &str, message: &str, error: Option<&dyn Error>) -> Result<()> {
    eprintln!("{} {}: {}", "✗".red(), prefix.red(), message.red());

    if let Some(err) = error {
        eprintln!("  {}", err.to_string().bright_red());
    }

    io::stderr()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stderr: {}", e))?;
    Ok(())
}

/// Format an API error for display.
///
/// # Arguments
/// * `message` - Error message
/// * `status_code` - Optional HTTP status code
/// * `details` - Optional error details
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_api_error(
    message: &str,
    status_code: Option<u16>,
    details: Option<&dyn Display>,
) -> Result<()> {
    eprint!("{} {}", "✗".red(), message.red());

    if let Some(code) = status_code {
        eprint!(" (HTTP {})", code);
    }

    eprintln!();

    if let Some(details) = details {
        eprintln!("  {}", details.to_string().bright_red());
    }

    io::stderr()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stderr: {}", e))?;
    Ok(())
}
