//! Status message components for success, error, warning, and info messages.

use crate::layout;
use crate::theme;
use design::{icons, ColorRole};
use std::io::{self, Write};

/// Print a success message with a checkmark icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn success(message: &str) -> io::Result<()> {
    println!(
        "{} {}",
        theme::style(icons::SUCCESS, ColorRole::Success),
        theme::style(message, ColorRole::Success)
    );
    io::stdout().flush()
}

/// Print a success message with a checkmark icon and spacing.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn success_with_spacing(message: &str) -> io::Result<()> {
    layout::blank_line()?;
    success(message)?;
    layout::blank_line()
}

/// Print an error message with an X icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn error(message: &str) -> io::Result<()> {
    eprintln!(
        "{} {}",
        theme::style(icons::ERROR, ColorRole::Error),
        theme::style(message, ColorRole::Error)
    );
    io::stderr().flush()
}

/// Print a warning message with a warning icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn warning(message: &str) -> io::Result<()> {
    eprintln!(
        "{} {}",
        theme::style(icons::WARNING, ColorRole::Warning),
        theme::style(message, ColorRole::Warning)
    );
    io::stderr().flush()
}

/// Print an info message with an info icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn info(message: &str) -> io::Result<()> {
    println!(
        "{} {}",
        theme::style(icons::INFO, ColorRole::Info),
        theme::style(message, ColorRole::Info)
    );
    io::stdout().flush()
}

/// Print a success message without an icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn success_plain(message: &str) -> io::Result<()> {
    println!("{}", theme::style(message, ColorRole::Success));
    io::stdout().flush()
}

/// Print an error message without an icon.
///
/// # Arguments
/// * `message` - Message to display
///
/// # Returns
/// `io::Result` indicating success or failure
pub fn error_plain(message: &str) -> io::Result<()> {
    eprintln!("{}", theme::style(message, ColorRole::Error));
    io::stderr().flush()
}
