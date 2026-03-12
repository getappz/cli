//! Firecrawl API response types.

use serde::{Deserialize, Serialize};

/// Full crawl data collected from a website.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlData {
    /// The root URL that was crawled.
    pub site_url: String,
    /// Branding data extracted from the homepage.
    pub branding: Option<BrandingData>,
    /// Content data for each discovered page.
    pub pages: Vec<PageData>,
    /// All discovered URLs on the site.
    pub site_map: Vec<String>,
}

/// Branding data extracted via Firecrawl's `branding` format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandingData {
    /// Color scheme: "dark" or "light".
    #[serde(default)]
    pub color_scheme: String,
    /// Logo URL.
    pub logo: Option<String>,
    /// Color palette.
    #[serde(default)]
    pub colors: BrandingColors,
    /// Font specifications.
    #[serde(default)]
    pub fonts: Vec<FontSpec>,
    /// Typography details.
    #[serde(default)]
    pub typography: TypographySpec,
    /// Spacing configuration.
    #[serde(default)]
    pub spacing: SpacingSpec,
    /// Component styles (button, etc).
    #[serde(default)]
    pub components: Option<serde_json::Value>,
    /// Brand images (logo, favicon, OG image).
    #[serde(default)]
    pub images: BrandingImages,
}

/// Color palette from branding data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrandingColors {
    #[serde(default = "default_primary")]
    pub primary: String,
    #[serde(default = "default_secondary")]
    pub secondary: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_text_primary")]
    pub text_primary: String,
    #[serde(default = "default_text_secondary")]
    pub text_secondary: String,
}

fn default_primary() -> String { "#3B82F6".to_string() }
fn default_secondary() -> String { "#1E293B".to_string() }
fn default_accent() -> String { "#F59E0B".to_string() }
fn default_background() -> String { "#FFFFFF".to_string() }
fn default_text_primary() -> String { "#1F2937".to_string() }
fn default_text_secondary() -> String { "#6B7280".to_string() }

/// A font specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSpec {
    pub family: String,
}

/// Typography details.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TypographySpec {
    #[serde(default)]
    pub font_families: FontFamilies,
    #[serde(default)]
    pub font_sizes: FontSizes,
    #[serde(default)]
    pub font_weights: FontWeights,
}

/// Font family assignments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontFamilies {
    #[serde(default = "default_font")]
    pub primary: String,
    #[serde(default = "default_font")]
    pub heading: String,
    #[serde(default)]
    pub code: Option<String>,
}

fn default_font() -> String { "Inter".to_string() }

/// Font sizes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontSizes {
    #[serde(default = "default_h1")]
    pub h1: String,
    #[serde(default = "default_h2")]
    pub h2: String,
    #[serde(default = "default_h3")]
    pub h3: String,
    #[serde(default = "default_body")]
    pub body: String,
}

fn default_h1() -> String { "48px".to_string() }
fn default_h2() -> String { "36px".to_string() }
fn default_h3() -> String { "24px".to_string() }
fn default_body() -> String { "16px".to_string() }

/// Font weights.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontWeights {
    #[serde(default = "default_regular_weight")]
    pub regular: u32,
    #[serde(default = "default_medium_weight")]
    pub medium: u32,
    #[serde(default = "default_bold_weight")]
    pub bold: u32,
}

fn default_regular_weight() -> u32 { 400 }
fn default_medium_weight() -> u32 { 500 }
fn default_bold_weight() -> u32 { 700 }

/// Spacing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpacingSpec {
    #[serde(default = "default_base_unit")]
    pub base_unit: u32,
    #[serde(default = "default_border_radius")]
    pub border_radius: String,
}

fn default_base_unit() -> u32 { 8 }
fn default_border_radius() -> String { "8px".to_string() }

/// Brand images.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrandingImages {
    pub logo: Option<String>,
    pub favicon: Option<String>,
    #[serde(rename = "ogImage")]
    pub og_image: Option<String>,
}

/// Content data for a single scraped page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageData {
    /// The page URL.
    pub url: String,
    /// Markdown content of the page.
    #[serde(default)]
    pub markdown: String,
    /// HTML content of the page.
    #[serde(default)]
    pub html: String,
    /// Links found on the page.
    #[serde(default)]
    pub links: Vec<String>,
    /// Page title (from metadata).
    pub title: Option<String>,
    /// Meta description.
    pub meta_description: Option<String>,
}

/// Firecrawl scrape API response.
#[derive(Debug, Deserialize)]
pub struct ScrapeResponse {
    pub success: bool,
    pub data: Option<ScrapeData>,
}

/// Data payload from a scrape response.
#[derive(Debug, Deserialize)]
pub struct ScrapeData {
    pub markdown: Option<String>,
    pub html: Option<String>,
    #[serde(rename = "rawHtml")]
    pub raw_html: Option<String>,
    pub links: Option<Vec<String>>,
    pub metadata: Option<ScrapeMetadata>,
    pub branding: Option<BrandingData>,
}

/// Metadata from a scrape response.
#[derive(Debug, Deserialize)]
pub struct ScrapeMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "ogImage")]
    pub og_image: Option<String>,
}

/// Firecrawl map API response.
#[derive(Debug, Deserialize)]
pub struct MapResponse {
    pub success: bool,
    pub links: Option<Vec<String>>,
}
