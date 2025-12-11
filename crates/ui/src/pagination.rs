//! Pagination display components.

use crate::format;
use miette::Result;
use owo_colors::OwoColorize;
use std::io::{self, Write};

/// Pagination information for display.
#[derive(Debug, Clone)]
pub struct PaginationInfo {
    /// Current count of items
    pub count: i64,
    /// Optional timestamp for next page
    pub next: Option<i64>,
    /// Optional timestamp for previous page
    pub prev: Option<i64>,
}

impl PaginationInfo {
    /// Create a new pagination info struct.
    pub fn new(count: i64, next: Option<i64>, prev: Option<i64>) -> Self {
        Self { count, next, prev }
    }
}

/// Display pagination information.
///
/// # Arguments
/// * `info` - Pagination information
///
/// # Returns
/// `Result` indicating success or failure
pub fn display(info: &PaginationInfo) -> Result<()> {
    let count = format::number(info.count);
    print!("{}", format!("Total: {} item(s)", count).bright_black());

    if info.next.is_some() || info.prev.is_some() {
        print!(" (");
        let mut parts = Vec::new();

        if info.prev.is_some() {
            parts.push("has previous page".to_string());
        }
        if info.next.is_some() {
            parts.push("has next page".to_string());
        }

        print!("{}", parts.join(", ").bright_black());
        print!(")");
    }

    println!();

    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}

/// Display pagination information in a compact format.
///
/// # Arguments
/// * `info` - Pagination information
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_compact(info: &PaginationInfo) -> Result<()> {
    let count = format::number(info.count);
    println!("{}", format!("Total: {}", count).bright_black());

    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}

/// Display pagination information with page numbers (if available).
///
/// # Arguments
/// * `current_page` - Current page number (1-indexed)
/// * `total_pages` - Total number of pages (if known)
/// * `count` - Total count of items
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_pages(current_page: usize, total_pages: Option<usize>, count: i64) -> Result<()> {
    let count_str = format::number(count);

    if let Some(total) = total_pages {
        println!(
            "{}",
            format!(
                "Page {} of {} ({} total items)",
                current_page, total, count_str
            )
            .bright_black()
        );
    } else {
        println!(
            "{}",
            format!("Page {} ({} total items)", current_page, count_str).bright_black()
        );
    }

    io::stdout()
        .flush()
        .map_err(|e| miette::miette!("Failed to flush stdout: {}", e))?;
    Ok(())
}
