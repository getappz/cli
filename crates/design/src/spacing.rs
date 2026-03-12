//! Spacing scale for consistent layout.
//!
//! Provides a unified spacing system similar to web design frameworks.

/// Vertical spacing between major sections (blank lines).
pub const SECTION: usize = 1;

/// Vertical spacing between items in a list.
pub const ITEM: usize = 0;

/// Horizontal padding around table cells.
pub const TABLE_CELL: usize = 1;

/// Indentation for nested items (one level).
pub const INDENT: &str = "  ";

/// Double indentation (two levels).
pub const DOUBLE_INDENT: &str = "    ";

/// Default width for separator lines (characters).
pub const SEPARATOR_WIDTH: usize = 80;

/// Minimum table column width.
pub const TABLE_COLUMN_MIN: usize = 3;
