//! WordPress WXR (WordPress eXtended RSS) XML parser.
//!
//! Deserializes the XML export into strongly-typed Rust structs using
//! `quick-xml` with serde. WXR is RSS 2.0 with custom namespaces:
//! `wp:`, `content:`, `excerpt:`, `dc:`.

use miette::{miette, Result};
use quick_xml::de::from_str;
use serde::Deserialize;

// ============================================================================
// Top-level structures
// ============================================================================

/// Root `<rss>` element.
#[derive(Debug, Deserialize)]
pub struct Rss {
    pub channel: Channel,
}

/// The `<channel>` element containing all items.
#[derive(Debug, Deserialize)]
pub struct Channel {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(rename = "item", default)]
    pub items: Vec<Item>,
}

// ============================================================================
// Item (post / page / attachment / etc.)
// ============================================================================

/// A single `<item>` in the export, representing a post, page, attachment, or
/// other WordPress content type.
#[derive(Debug, Deserialize)]
pub struct Item {
    /// Post title.
    #[serde(default)]
    pub title: Option<String>,

    /// Permalink.
    #[serde(default)]
    pub link: Option<String>,

    /// Author username (`<dc:creator>`).
    #[serde(rename = "creator", default)]
    pub creator: Option<String>,

    /// HTML body content (`<content:encoded>`).
    #[serde(rename = "encoded", default)]
    pub content_encoded: Option<String>,

    // Note: `<excerpt:encoded>` also uses the local name "encoded".
    // quick-xml flattens namespaces, so the second "encoded" field
    // may collide. We handle excerpt extraction in the parser instead.

    /// WordPress post ID (`<wp:post_id>`).
    #[serde(rename = "post_id", default)]
    pub post_id: Option<String>,

    /// Post date string (`<wp:post_date>`, local time).
    #[serde(rename = "post_date", default)]
    pub post_date: Option<String>,

    /// Post date in GMT (`<wp:post_date_gmt>`).
    #[serde(rename = "post_date_gmt", default)]
    pub post_date_gmt: Option<String>,

    /// Post type (`<wp:post_type>`): "post", "page", "attachment", etc.
    #[serde(rename = "post_type", default)]
    pub post_type: Option<String>,

    /// Post status (`<wp:status>`): "publish", "draft", "private", etc.
    #[serde(rename = "status", default)]
    pub status: Option<String>,

    /// Post slug (`<wp:post_name>`), URL-encoded.
    #[serde(rename = "post_name", default)]
    pub post_name: Option<String>,

    /// Parent post ID (`<wp:post_parent>`).
    #[serde(rename = "post_parent", default)]
    pub post_parent: Option<String>,

    /// Attachment URL (`<wp:attachment_url>`), for type "attachment".
    #[serde(rename = "attachment_url", default)]
    pub attachment_url: Option<String>,

    /// Categories and tags (`<category>` elements).
    #[serde(rename = "category", default)]
    pub categories: Vec<WpCategory>,

    /// Post meta (`<wp:postmeta>` elements).
    #[serde(rename = "postmeta", default)]
    pub postmeta: Vec<WpPostMeta>,

    /// Post excerpt (`<excerpt:encoded>`) — extracted via fallback.
    /// quick-xml may map this to the second `encoded` field. If it
    /// collides with `content_encoded`, the parser manually extracts it.
    #[serde(rename = "excerpt", default)]
    pub excerpt: Option<WpExcerpt>,
}

// ============================================================================
// Supporting types
// ============================================================================

/// Wrapper for `<excerpt:encoded>` to avoid namespace collision.
/// quick-xml may deserialize it as a child element named `excerpt`
/// whose text content is the excerpt body.
#[derive(Debug, Deserialize)]
pub struct WpExcerpt {
    #[serde(rename = "$text", default)]
    pub text: Option<String>,
    #[serde(rename = "encoded", default)]
    pub encoded: Option<String>,
}

impl WpExcerpt {
    pub fn value(&self) -> Option<&str> {
        self.encoded
            .as_deref()
            .or(self.text.as_deref())
            .filter(|s| !s.is_empty())
    }
}

/// A `<category>` element which represents either a category or a tag,
/// distinguished by the `domain` attribute.
#[derive(Debug, Deserialize)]
pub struct WpCategory {
    /// "category" or "post_tag".
    #[serde(rename = "@domain", default)]
    pub domain: Option<String>,
    /// The URL-safe slug.
    #[serde(rename = "@nicename", default)]
    pub nicename: Option<String>,
    /// The display name (may contain HTML entities).
    #[serde(rename = "$text", default)]
    pub display_name: Option<String>,
}

/// A `<wp:postmeta>` element containing a key-value pair.
#[derive(Debug, Deserialize)]
pub struct WpPostMeta {
    #[serde(rename = "meta_key", default)]
    pub meta_key: Option<String>,
    #[serde(rename = "meta_value", default)]
    pub meta_value: Option<String>,
}

// ============================================================================
// Parser entry point
// ============================================================================

/// Parse a WordPress WXR XML string into the typed data model.
pub fn parse_wxr(xml_content: &str) -> Result<Rss> {
    // Strip XML declaration if present (quick-xml + serde can choke on it sometimes)
    let content = xml_content
        .trim_start_matches('\u{feff}') // BOM
        .trim();

    from_str::<Rss>(content).map_err(|e| miette!("Failed to parse WXR XML: {}", e))
}

// ============================================================================
// Convenience helpers
// ============================================================================

/// Image file extensions that WordPress typically uses for attachments.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico",
];

/// Post types to exclude from processing.
pub const EXCLUDED_POST_TYPES: &[&str] = &[
    "nav_menu_item",
    "revision",
    "custom_css",
    "customize_changeset",
    "oembed_cache",
    "wp_block",
    "wp_template",
    "wp_template_part",
    "wp_global_styles",
    "wp_navigation",
    "wp_font_family",
    "wp_font_face",
    "user_request",
    "wp_block_pattern",
];

impl Item {
    /// Get the post type, defaulting to "post".
    pub fn get_post_type(&self) -> &str {
        self.post_type.as_deref().unwrap_or("post")
    }

    /// Get the post ID as an i64.
    pub fn get_post_id(&self) -> Option<i64> {
        self.post_id.as_deref().and_then(|s| s.parse().ok())
    }

    /// Get the value of a specific postmeta key.
    pub fn get_meta_value(&self, key: &str) -> Option<&str> {
        self.postmeta
            .iter()
            .find(|m| m.meta_key.as_deref() == Some(key))
            .and_then(|m| m.meta_value.as_deref())
            .filter(|v| !v.is_empty())
    }
}
