//! Configuration types for the WordPress-to-Markdown converter.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the conversion process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wp2mdConfig {
    /// Path to the WordPress WXR export XML file, or WordPress site URL for wp-json.
    pub input: String,
    /// Output directory for generated files.
    pub output: String,
    /// Posts per page when fetching from wp-json API (default 100).
    pub wpjson_per_page: u32,
    /// Whether to fetch pages in addition to posts when using wp-json (default true).
    pub wpjson_include_pages: bool,
    /// Whether to create a separate folder for each post.
    pub post_folders: bool,
    /// Whether to prepend the date to post folder/file names.
    pub prefix_date: bool,
    /// How to organize output into date-based folders.
    pub date_folders: DateFolders,
    /// Which images to save.
    pub save_images: SaveImages,
    /// Ordered list of frontmatter fields to include.
    pub frontmatter_fields: Vec<FrontmatterField>,
    /// Delay in milliseconds between image download requests.
    pub request_delay_ms: u64,
    /// IANA timezone to apply to post dates (e.g. "utc", "America/New_York").
    pub timezone: String,
    /// Whether to include time in frontmatter date values.
    pub include_time: bool,
    /// Custom date format string (Luxon/chrono-compatible tokens).
    /// When set, takes precedence over `include_time`.
    pub date_format: Option<String>,
    /// Whether to wrap frontmatter date values in double quotes.
    pub quote_date: bool,
    /// Whether to use strict SSL when downloading images.
    pub strict_ssl: bool,
}

impl Default for Wp2mdConfig {
    fn default() -> Self {
        Self {
            input: "export.xml".to_string(),
            output: "output".to_string(),
            wpjson_per_page: 100,
            wpjson_include_pages: true,
            post_folders: true,
            prefix_date: false,
            date_folders: DateFolders::None,
            save_images: SaveImages::All,
            frontmatter_fields: vec![
                FrontmatterField::new("title", None),
                FrontmatterField::new("date", None),
                FrontmatterField::new("categories", None),
                FrontmatterField::new("tags", None),
                FrontmatterField::new("coverImage", None),
                FrontmatterField::new("draft", None),
            ],
            request_delay_ms: 500,
            timezone: "utc".to_string(),
            include_time: false,
            date_format: None,
            quote_date: false,
            strict_ssl: true,
        }
    }
}

/// How to organize output into date-based sub-folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateFolders {
    /// No date-based folders.
    None,
    /// Organize into `<year>/` folders.
    Year,
    /// Organize into `<year>/<month>/` folders.
    YearMonth,
}

/// Which images to download and save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveImages {
    /// Don't save any images.
    None,
    /// Save images attached to posts (uploaded via Add Media / Set Featured Image).
    Attached,
    /// Save images scraped from `<img>` tags in post body content.
    Scraped,
    /// Save all images (attached + scraped).
    All,
}

impl SaveImages {
    pub fn includes_attached(self) -> bool {
        matches!(self, SaveImages::Attached | SaveImages::All)
    }

    pub fn includes_scraped(self) -> bool {
        matches!(self, SaveImages::Scraped | SaveImages::All)
    }
}

/// A frontmatter field with an optional alias.
///
/// The `name` is the canonical field (e.g. "date") and `alias` is
/// the output name (e.g. "created"). If `alias` is `None`, the
/// canonical name is used as-is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontmatterField {
    pub name: String,
    pub alias: Option<String>,
}

impl FrontmatterField {
    pub fn new(name: &str, alias: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            alias: alias.map(|a| a.to_string()),
        }
    }

    /// The key to use in the YAML frontmatter output.
    pub fn output_key(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// Parse a comma-separated frontmatter fields string.
///
/// Supports aliases via colon notation: `"date:created,title"`.
pub fn parse_frontmatter_fields(input: &str) -> Vec<FrontmatterField> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some((name, alias)) = s.split_once(':') {
                FrontmatterField::new(name.trim(), Some(alias.trim()))
            } else {
                FrontmatterField::new(s, None)
            }
        })
        .collect()
}
