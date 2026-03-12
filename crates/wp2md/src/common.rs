//! Shared utility functions for path building, slug handling, and
//! filename extraction.
//!
//! Mirrors the reference `shared.js` utility functions.

use crate::config::{DateFolders, Wp2mdConfig};
use crate::types::Post;

/// Build the output file path for a post.
///
/// The path structure depends on configuration:
/// `<output>/<type_folder>/<drafts>/<date_folders>/<slug>[/index.md | .md]`
pub fn build_post_path(post: &Post, config: &Wp2mdConfig) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Start with output folder
    parts.push(config.output.clone());

    // Add folder for post type (only for non-"post" types)
    if post.post_type != "post" {
        parts.push(post.post_type.clone());
    }

    // Add drafts folder for draft posts
    if post.is_draft {
        parts.push("_drafts".to_string());
    }

    // Add date folders (year, year/month) if configured and date exists
    if let Some(dt) = post.date {
        match config.date_folders {
            DateFolders::Year => {
                parts.push(dt.format("%Y").to_string());
            }
            DateFolders::YearMonth => {
                parts.push(dt.format("%Y").to_string());
                parts.push(dt.format("%m").to_string());
            }
            DateFolders::None => {}
        }
    }

    // Build the slug (with optional date prefix)
    let slug = slug_with_fallback(post);
    let name = if config.prefix_date {
        if let Some(dt) = post.date {
            format!("{}-{}", dt.format("%Y-%m-%d"), slug)
        } else {
            slug
        }
    } else {
        slug
    };

    // Use slug as folder (with index.md) or as filename
    if config.post_folders {
        parts.push(name);
        parts.push("index.md".to_string());
    } else {
        parts.push(format!("{}.md", name));
    }

    parts.join("/")
}

/// Build the images directory path for a post.
pub fn build_images_path(post: &Post, config: &Wp2mdConfig) -> String {
    let post_path = build_post_path(post, config);
    if config.post_folders {
        // images/ folder is next to index.md
        let parent = post_path
            .rsplit_once('/')
            .map(|(p, _)| p)
            .unwrap_or(&config.output);
        format!("{}/images", parent)
    } else {
        // Shared images/ folder in the output root
        format!("{}/images", config.output)
    }
}

/// Get the slug with a fallback to the post ID.
pub fn slug_with_fallback(post: &Post) -> String {
    if post.slug.is_empty() {
        post.id.to_string()
    } else {
        post.slug.clone()
    }
}

/// Extract a sanitized filename from a URL.
///
/// Removes query parameters and hash fragments, URL-decodes the filename,
/// and replaces invalid characters.
pub fn filename_from_url(url: &str) -> String {
    // Remove query parameters and hash fragments
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);

    // Get the last path segment
    let filename = path.rsplit('/').next().unwrap_or("image");

    // Try to URL-decode
    let decoded = urlencoding::decode(filename)
        .unwrap_or_else(|_| filename.into())
        .into_owned();

    // Replace invalid filename characters
    sanitize_filename(&decoded)
}

/// Replace characters that are invalid in filenames (Windows-compatible).
fn sanitize_filename(name: &str) -> String {
    let mut result = name.to_string();
    for ch in &['<', '>', ':', '"', '|', '?', '*'] {
        result = result.replace(*ch, "_");
    }
    // Replace backslashes
    result = result.replace('\\', "_");
    // Collapse multiple underscores
    while result.contains("__") {
        result = result.replace("__", "_");
    }
    result
}

/// Convert a string to camelCase.
pub fn camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, ch) in s.chars().enumerate() {
        if ch == '-' || ch == '_' || ch == ' ' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}
