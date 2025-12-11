//! Layout utilities for consistent spacing and formatting.

use owo_colors::OwoColorize;
use std::io::{self, Write};

/// Spacing constants for consistent layout
pub mod spacing {
    /// Vertical spacing between major sections
    pub const SECTION_SPACING: usize = 1;

    /// Vertical spacing between items in a list
    pub const ITEM_SPACING: usize = 0;

    /// Padding around tables
    pub const TABLE_PADDING: usize = 0;

    /// Indentation for nested items
    pub const INDENT: &str = "  ";

    /// Double indentation
    pub const DOUBLE_INDENT: &str = "    ";
}

/// Print a blank line for spacing
pub fn blank_line() -> io::Result<()> {
    println!();
    io::stdout().flush()
}

/// Print multiple blank lines
pub fn blank_lines(count: usize) -> io::Result<()> {
    for _ in 0..count {
        println!();
    }
    io::stdout().flush()
}

/// Print a section title with proper formatting
pub fn section_title(title: &str) -> io::Result<()> {
    println!("{}", title.bold());
    io::stdout().flush()
}

/// Print a subsection title with proper formatting
pub fn subsection_title(title: &str) -> io::Result<()> {
    println!("{}", title);
    io::stdout().flush()
}

/// Print a separator line
pub fn separator() -> io::Result<()> {
    println!("{}", "─".repeat(80).bright_black());
    io::stdout().flush()
}

/// Print a separator with custom character
pub fn separator_with_char(ch: char) -> io::Result<()> {
    println!("{}", ch.to_string().repeat(80).bright_black());
    io::stdout().flush()
}

/// Print indented text
pub fn indented(text: &str, level: usize) -> io::Result<()> {
    let indent = spacing::INDENT.repeat(level);
    println!("{}{}", indent, text);
    io::stdout().flush()
}
