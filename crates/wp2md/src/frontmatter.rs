//! YAML frontmatter generation for Markdown files.
//!
//! Mirrors the reference `frontmatter.js` — each field is extracted
//! from the `Post` and formatted according to the `Wp2mdConfig`.

use crate::config::Wp2mdConfig;
use crate::types::Post;
use chrono::NaiveDateTime;

/// Build a YAML frontmatter string for a post based on the configured fields.
///
/// Returns a complete frontmatter block including the `---` delimiters.
pub fn build_frontmatter(post: &Post, config: &Wp2mdConfig) -> String {
    let mut lines = Vec::new();

    for field in &config.frontmatter_fields {
        if let Some(value) = get_field_value(post, &field.name, config) {
            let key = field.output_key();
            lines.push(format!("{}: {}", key, value));
        }
    }

    if lines.is_empty() {
        return String::new();
    }

    format!("---\n{}\n---\n", lines.join("\n"))
}

/// Get the formatted value for a single frontmatter field.
fn get_field_value(post: &Post, field_name: &str, config: &Wp2mdConfig) -> Option<String> {
    match field_name {
        "title" => format_title(post),
        "date" => format_date(post, config),
        "categories" => format_string_list(&post.categories),
        "tags" => format_string_list(&post.tags),
        "coverImage" | "cover_image" => format_cover_image(post),
        "draft" => format_draft(post),
        "author" => post.author.as_ref().map(|a| format!("\"{}\"", a)),
        "excerpt" => format_excerpt(post),
        "id" => Some(post.id.to_string()),
        "slug" => {
            if post.slug.is_empty() {
                None
            } else {
                Some(format!("\"{}\"", post.slug))
            }
        }
        "type" => Some(format!("\"{}\"", post.post_type)),
        _ => None,
    }
}

/// Format the title, quoting it to handle special YAML characters.
fn format_title(post: &Post) -> Option<String> {
    if post.title.is_empty() {
        return None;
    }
    Some(format!("\"{}\"", post.title.replace('"', "\\\"")))
}

/// Format the date according to config settings.
fn format_date(post: &Post, config: &Wp2mdConfig) -> Option<String> {
    let dt = post.date?;

    let formatted = if let Some(ref fmt) = config.date_format {
        format_with_custom(dt, fmt)
    } else if config.include_time {
        dt.format("%Y-%m-%dT%H:%M:%S.000Z").to_string()
    } else {
        dt.format("%Y-%m-%d").to_string()
    };

    if config.quote_date {
        Some(format!("\"{}\"", formatted))
    } else {
        Some(formatted)
    }
}

/// Format a date with a custom format string.
/// Maps common Luxon tokens to chrono equivalents.
fn format_with_custom(dt: NaiveDateTime, fmt: &str) -> String {
    // The format string uses Luxon-style tokens; chrono uses strftime.
    // Common mappings: yyyy->%Y, MM->%m, dd->%d, HH->%H, mm->%M, ss->%S
    let chrono_fmt = fmt
        .replace("yyyy", "%Y")
        .replace("yy", "%y")
        .replace("MM", "%m")
        .replace("dd", "%d")
        .replace("HH", "%H")
        .replace("mm", "%M")
        .replace("ss", "%S");

    dt.format(&chrono_fmt).to_string()
}

/// Format a string list as a YAML array.
fn format_string_list(items: &[String]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let entries: Vec<String> = items.iter().map(|s| format!("  - \"{}\"", s.replace('"', "\\\""))).collect();
    Some(format!("\n{}", entries.join("\n")))
}

/// Format the cover image filename.
fn format_cover_image(post: &Post) -> Option<String> {
    post.cover_image
        .as_ref()
        .map(|img| format!("\"{}\"", img.replace('"', "\\\"")))
}

/// Format the draft status — only included when true.
fn format_draft(post: &Post) -> Option<String> {
    if post.is_draft {
        Some("true".to_string())
    } else {
        None
    }
}

/// Format the excerpt, collapsing newlines and quoting.
fn format_excerpt(post: &Post) -> Option<String> {
    post.excerpt
        .as_ref()
        .filter(|e| !e.is_empty())
        .map(|e| format!("\"{}\"", e.replace('"', "\\\"")))
}
