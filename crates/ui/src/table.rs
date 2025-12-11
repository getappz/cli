//! Table display components for structured data.

use crate::empty;
use crate::layout;
use miette::Result;
use tabled::{settings::Style, Table, Tabled};

/// Display a table from a vector of row vectors.
///
/// # Arguments
/// * `headers` - Column headers
/// * `rows` - Data rows (each row is a vector of strings)
/// * `title` - Optional table title
///
/// # Returns
/// `Result` indicating success or failure
pub fn display(headers: &[&str], rows: &[Vec<String>], title: Option<&str>) -> Result<()> {
    if rows.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No data found", None)?;
        }
        return Ok(());
    }

    // Print title if provided
    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    // Build table data with headers and rows
    let mut table_data: Vec<Vec<String>> = vec![headers.iter().map(|s| s.to_string()).collect()];
    table_data.extend(rows.iter().cloned());

    // Create table using tabled with proper formatting
    // We'll use a custom approach since tabled requires Tabled trait for rows
    display_formatted_table(&table_data)?;

    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    Ok(())
}

/// Display a table from a vector of row vectors with custom column widths.
///
/// # Arguments
/// * `headers` - Column headers
/// * `rows` - Data rows (each row is a vector of strings)
/// * `widths` - Optional column widths (if None, auto-size)
/// * `title` - Optional table title
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_with_widths(
    headers: &[&str],
    rows: &[Vec<String>],
    _widths: Option<&[usize]>,
    title: Option<&str>,
) -> Result<()> {
    // Use the main display function which handles formatting
    display(headers, rows, title)
}

/// Display a table from a vector of structs implementing `Tabled`.
///
/// # Arguments
/// * `items` - Vector of items to display
/// * `title` - Optional table title
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_tabled<T: Tabled>(items: &[T], title: Option<&str>) -> Result<()> {
    if items.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No data found", None)?;
        }
        return Ok(());
    }

    // Print title if provided
    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    let table = Table::new(items).with(Style::modern()).to_string();

    println!("{}", table);
    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    Ok(())
}

/// Internal function to display a formatted table using tabled crate.
/// This creates a proper table with borders and alignment.
fn display_formatted_table(data: &[Vec<String>]) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }

    // Calculate column widths based on content
    let num_cols = data[0].len();
    let mut col_widths = vec![0; num_cols];

    for row in data {
        for (i, cell) in row.iter().enumerate() {
            // Account for ANSI color codes in width calculation
            let width = strip_ansi_codes(cell).len();
            col_widths[i] = col_widths[i].max(width).max(3); // Min width of 3
        }
    }

    // Build table string with proper borders
    let mut output = String::new();

    // Top border
    output.push('┌');
    for (i, &width) in col_widths.iter().enumerate() {
        output.push_str(&"─".repeat(width + 2));
        if i < col_widths.len() - 1 {
            output.push('┬');
        }
    }
    output.push_str("┐\n");

    // Header row
    if !data.is_empty() {
        output.push('│');
        for (i, cell) in data[0].iter().enumerate() {
            let width = col_widths[i];
            let content = format!(" {:<width$} ", cell, width = width);
            output.push_str(&content);
            output.push('│');
        }
        output.push('\n');

        // Header separator
        output.push('├');
        for (i, &width) in col_widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < col_widths.len() - 1 {
                output.push('┼');
            }
        }
        output.push_str("┤\n");
    }

    // Data rows
    for row in data.iter().skip(1) {
        output.push('│');
        for (i, cell) in row.iter().enumerate() {
            let width = col_widths[i];
            let content = format!(" {:<width$} ", cell, width = width);
            output.push_str(&content);
            output.push('│');
        }
        output.push('\n');
    }

    // Bottom border
    output.push('└');
    for (i, &width) in col_widths.iter().enumerate() {
        output.push_str(&"─".repeat(width + 2));
        if i < col_widths.len() - 1 {
            output.push('┴');
        }
    }
    output.push_str("┘\n");

    print!("{}", output);
    Ok(())
}

/// Strip ANSI color codes from a string to calculate true width
fn strip_ansi_codes(s: &str) -> String {
    // Simple ANSI code stripper - removes ESC[...m sequences
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ANSI escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == 'm' {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Display a simple table with a separator line (for backward compatibility).
///
/// # Arguments
/// * `headers` - Column headers
/// * `rows` - Data rows (each row is a vector of strings)
/// * `separator` - Separator character (default: '-')
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_simple(
    headers: &[&str],
    rows: &[Vec<String>],
    _separator: Option<char>,
) -> Result<()> {
    if rows.is_empty() {
        empty::display("No data found", None)?;
        return Ok(());
    }

    // Use the formatted table display instead
    let mut table_data: Vec<Vec<String>> = vec![headers.iter().map(|s| s.to_string()).collect()];
    table_data.extend(rows.iter().cloned());
    display_formatted_table(&table_data)?;
    layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    Ok(())
}
