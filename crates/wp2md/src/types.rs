//! Core data types for the WordPress-to-Markdown converter.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// A single post/page extracted from the WordPress export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    /// WordPress post ID.
    pub id: i64,
    /// Post title (HTML entities preserved, not decoded).
    pub title: String,
    /// URL-decoded slug.
    pub slug: String,
    /// Publication date (parsed from `wp:post_date`).
    pub date: Option<NaiveDateTime>,
    /// Post type: "post", "page", or custom type.
    pub post_type: String,
    /// WordPress status: "publish", "draft", "private", etc.
    pub status: String,
    /// Post author username (from `dc:creator`).
    pub author: Option<String>,
    /// Post excerpt (newlines collapsed).
    pub excerpt: Option<String>,
    /// Raw HTML body content (from `content:encoded`).
    pub content_html: String,
    /// Markdown-converted body content (populated by translator).
    pub content_md: String,
    /// Decoded category names (excluding "uncategorized").
    pub categories: Vec<String>,
    /// Decoded tag names.
    pub tags: Vec<String>,
    /// Cover/featured image filename (decoded).
    pub cover_image: Option<String>,
    /// All images associated with this post (attached + scraped).
    pub images: Vec<ImageRef>,
    /// Whether this post is a draft.
    pub is_draft: bool,
}

/// A reference to an image associated with a post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    /// Absolute URL of the image.
    pub url: String,
    /// Sanitized filename extracted from the URL.
    pub filename: String,
    /// The WordPress post ID this image is attached to (if applicable).
    pub post_id: Option<i64>,
    /// Whether this image is set as the featured/cover image.
    pub is_cover: bool,
}

/// Result statistics from a conversion run.
#[derive(Debug, Clone, Default)]
pub struct ConvertResult {
    pub posts_written: usize,
    pub posts_skipped: usize,
    pub images_downloaded: usize,
    pub images_skipped: usize,
}
