//! Error display and formatting utilities.

use crate::layout;
use crate::theme;
use design::icons;
use design::ColorRole;
use miette::Result;
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
    eprintln!(
        "{} {}",
        theme::style(icons::ERROR, ColorRole::Error),
        theme::style(message, ColorRole::Error)
    );

    if let Some(err) = error {
        eprintln!("  {}", theme::style(&err.to_string(), ColorRole::Error));
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
    eprintln!(
        "{} {}: {}",
        theme::style(icons::ERROR, ColorRole::Error),
        theme::style(prefix, ColorRole::Error),
        theme::style(message, ColorRole::Error)
    );

    if let Some(err) = error {
        eprintln!("  {}", theme::style(&err.to_string(), ColorRole::Error));
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
    eprint!(
        "{} {}",
        theme::style(icons::ERROR, ColorRole::Error),
        theme::style(message, ColorRole::Error)
    );

    if let Some(code) = status_code {
        eprint!(" (HTTP {})", code);
    }

    eprintln!();

    if let Some(details) = details {
        eprintln!("  {}", theme::style(&details.to_string(), ColorRole::Error));
    }

    io::stderr()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stderr: {}", e))?;
    Ok(())
}
