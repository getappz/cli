//! Layout utilities for consistent spacing and formatting.

use crate::theme;
use design::{layout as design_layout, spacing as design_spacing, ColorRole};
use std::io::{self, Write};

/// Spacing constants (re-exported from design crate).
pub mod spacing {
    pub use design::{
        DOUBLE_INDENT, INDENT, ITEM, SECTION, SEPARATOR_WIDTH, TABLE_CELL, TABLE_COLUMN_MIN,
    };
    /// Alias for backward compatibility.
    pub const SECTION_SPACING: usize = design::SECTION;
    /// Alias for backward compatibility.
    pub const ITEM_SPACING: usize = design::ITEM;
    /// Alias for backward compatibility.
    pub const TABLE_PADDING: usize = design::TABLE_CELL;
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
    println!("{}", theme::style_accent_bold(title));
    io::stdout().flush()
}

/// Print a subsection title with proper formatting
pub fn subsection_title(title: &str) -> io::Result<()> {
    println!("{}", title);
    io::stdout().flush()
}

/// Print a separator line
pub fn separator() -> io::Result<()> {
    let line = design_layout::SEPARATOR_CHAR
        .to_string()
        .repeat(design_spacing::SEPARATOR_WIDTH);
    println!("{}", theme::style(&line, ColorRole::Border));
    io::stdout().flush()
}

/// Print a separator with custom character
pub fn separator_with_char(ch: char) -> io::Result<()> {
    let line = ch.to_string().repeat(design_spacing::SEPARATOR_WIDTH);
    println!("{}", theme::style(&line, ColorRole::Border));
    io::stdout().flush()
}

/// Print indented text
pub fn indented(text: &str, level: usize) -> io::Result<()> {
    let indent = design_spacing::INDENT.repeat(level);
    println!("{}{}", indent, text);
    io::stdout().flush()
}
