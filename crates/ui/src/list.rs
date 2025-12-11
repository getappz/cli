//! List display components for grouped and hierarchical data.

use crate::empty;
use crate::layout;
use miette::Result;
use std::collections::BTreeMap;
use std::fmt::Display;

/// Display a grouped list (e.g., tasks by namespace).
///
/// # Arguments
/// * `groups` - Map of group names to items (item name, description)
/// * `title` - Optional title for the list
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_grouped(
    groups: &BTreeMap<String, Vec<(String, Option<String>)>>,
    title: Option<&str>,
) -> Result<()> {
    if groups.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No items found", None)?;
        }
        return Ok(());
    }

    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    for (group_name, items) in groups {
        println!("[{}]", group_name);
        for (name, description) in items {
            if let Some(desc) = description {
                println!("  {}\t{}", name, desc);
            } else {
                println!("  {}", name);
            }
        }
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    Ok(())
}

/// Display a simple bullet list.
///
/// # Arguments
/// * `items` - Vector of items to display
/// * `title` - Optional title for the list
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_bullet<T: Display>(items: &[T], title: Option<&str>) -> Result<()> {
    if items.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No items found", None)?;
        }
        return Ok(());
    }

    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    for item in items {
        println!("  • {}", item);
    }

    Ok(())
}

/// Display a numbered list.
///
/// # Arguments
/// * `items` - Vector of items to display
/// * `title` - Optional title for the list
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_numbered<T: Display>(items: &[T], title: Option<&str>) -> Result<()> {
    if items.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No items found", None)?;
        }
        return Ok(());
    }

    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    for (i, item) in items.iter().enumerate() {
        println!("  {}. {}", i + 1, item);
    }

    Ok(())
}

/// Display a hierarchical list with indentation.
///
/// # Arguments
/// * `items` - Vector of (level, item) tuples where level is the indentation depth
/// * `title` - Optional title for the list
///
/// # Returns
/// `Result` indicating success or failure
pub fn display_hierarchical<T: Display>(items: &[(usize, T)], title: Option<&str>) -> Result<()> {
    if items.is_empty() {
        if let Some(title) = title {
            empty::display(&format!("No {} found", title), None)?;
        } else {
            empty::display("No items found", None)?;
        }
        return Ok(());
    }

    if let Some(title) = title {
        layout::section_title(title)
            .map_err(|e| miette::miette!("Failed to print title: {}", e))?;
        layout::blank_line().map_err(|e| miette::miette!("Failed to print blank line: {}", e))?;
    }

    for (level, item) in items {
        let indent = layout::spacing::INDENT.repeat(*level);
        println!("{}{}", indent, item);
    }

    Ok(())
}
