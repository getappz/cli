//! WASM bindings for firecrawl_rs.
//!
//! Provides `#[wasm_bindgen]` exports for all WASM-safe functions from the
//! native crate. All functions accept/return JSON strings for complex types.
//! Built with `wasm-pack build --target bundler`.w

use base64::Engine;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Helper: wrap a Result<T: Serialize, String> into a JSON string Result
// ---------------------------------------------------------------------------

fn json_ok<T: serde::Serialize>(val: &T) -> Result<String, JsError> {
  serde_json::to_string(val).map_err(|e| JsError::new(&e.to_string()))
}

fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, JsError> {
  serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))
}

fn map_err(e: String) -> JsError {
  JsError::new(&e)
}

// ===========================================================================
// HTML functions
// ===========================================================================

/// Extract the base href from an HTML document.
/// Returns JSON: `string`
#[wasm_bindgen]
pub fn extract_base_href(html: &str, url: &str) -> Result<String, JsError> {
  let result = firecrawl_rs::extract_base_href(html.to_string(), url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all links from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_links(html: &str) -> Result<String, JsError> {
  let input = if html.is_empty() {
    None
  } else {
    Some(html.to_string())
  };
  let result = firecrawl_rs::extract_links(input).map_err(map_err)?;
  json_ok(&result)
}

/// Extract metadata from an HTML document.
/// Returns JSON: `Record<string, any>`
#[wasm_bindgen]
pub fn extract_metadata(html: &str) -> Result<String, JsError> {
  let input = if html.is_empty() {
    None
  } else {
    Some(html.to_string())
  };
  let result = firecrawl_rs::extract_metadata(input).map_err(map_err)?;
  json_ok(&result)
}

/// Transform and clean HTML content based on provided options.
/// Accepts JSON: `TransformHtmlOptions`
/// Returns JSON: `string` (cleaned HTML)
#[wasm_bindgen]
pub fn transform_html(opts_json: &str) -> Result<String, JsError> {
  let opts: firecrawl_rs::TransformHtmlOptions = from_json(opts_json)?;
  let result = firecrawl_rs::transform_html(opts).map_err(map_err)?;
  json_ok(&result)
}

/// Extract inner text content from HTML body.
/// Returns JSON: `string`
#[wasm_bindgen]
pub fn get_inner_json(html: &str) -> Result<String, JsError> {
  let result = firecrawl_rs::get_inner_json(html.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract specified attributes from HTML elements matching selectors.
/// Accepts JSON: `ExtractAttributesOptions`
/// Returns JSON: `ExtractedAttributeResult[]`
#[wasm_bindgen]
pub fn extract_attributes(html: &str, opts_json: &str) -> Result<String, JsError> {
  let options: firecrawl_rs::ExtractAttributesOptions = from_json(opts_json)?;
  let result =
    firecrawl_rs::extract_attributes(html.to_string(), options).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all image URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_images(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_images(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all CSS stylesheet URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_css(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_css(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all JavaScript URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_js(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_js(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all font URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_fonts(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_fonts(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all video URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_videos(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_videos(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all audio URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_audio(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_audio(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract all iframe/embed/object URLs from an HTML document.
/// Returns JSON: `string[]`
#[wasm_bindgen]
pub fn extract_iframes(html: &str, base_url: &str) -> Result<String, JsError> {
  let result =
    firecrawl_rs::extract_iframes(html.to_string(), base_url.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Extract assets from HTML based on requested formats.
/// `formats_json` is a JSON array of format strings, e.g. `["css","js","fonts"]` or `["assets"]`.
/// Returns JSON: `ExtractedAssets`
#[wasm_bindgen]
pub fn extract_assets(html: &str, base_url: &str, formats_json: &str) -> Result<String, JsError> {
  let formats: Vec<String> = from_json(formats_json)?;
  let result = firecrawl_rs::extract_assets(html.to_string(), base_url.to_string(), formats)
    .map_err(map_err)?;
  json_ok(&result)
}

/// Process multi-line links in markdown (escape newlines inside link text, strip "Skip to Content" links).
/// Returns JSON: `string`
#[wasm_bindgen]
pub fn post_process_markdown(markdown: &str) -> Result<String, JsError> {
  let result = firecrawl_rs::post_process_markdown(markdown.to_string()).map_err(map_err)?;
  json_ok(&result)
}

// ===========================================================================
// Crawler functions
// ===========================================================================

/// Filter links based on crawling rules and constraints.
/// Accepts JSON: `FilterLinksCall`
/// Returns JSON: `FilterLinksResult`
#[wasm_bindgen]
pub fn filter_links(params_json: &str) -> Result<String, JsError> {
  let data: firecrawl_rs::FilterLinksCall = from_json(params_json)?;
  let result = firecrawl_rs::filter_links(data).map_err(map_err)?;
  json_ok(&result)
}

/// Filter a single URL based on crawling rules and constraints.
/// Accepts JSON: `FilterUrlCall`
/// Returns JSON: `FilterUrlResult`
#[wasm_bindgen]
pub fn filter_url(params_json: &str) -> Result<String, JsError> {
  let data: firecrawl_rs::FilterUrlCall = from_json(params_json)?;
  let result = firecrawl_rs::filter_url(data).map_err(map_err)?;
  json_ok(&result)
}

/// Parse XML sitemap content into structured data.
/// Returns JSON: `ParsedSitemap`
#[wasm_bindgen]
pub fn parse_sitemap_xml(xml_content: &str) -> Result<String, JsError> {
  let result = firecrawl_rs::parse_sitemap_xml(xml_content.to_string()).map_err(map_err)?;
  json_ok(&result)
}

/// Process sitemap XML and extract crawling instructions.
/// Returns JSON: `SitemapProcessingResult`
#[wasm_bindgen]
pub fn process_sitemap(xml_content: &str) -> Result<String, JsError> {
  let result = firecrawl_rs::process_sitemap(xml_content.to_string()).map_err(map_err)?;
  json_ok(&result)
}

// ===========================================================================
// Engpicker
// ===========================================================================

/// Compute engpicker verdict using Levenshtein distance comparison.
/// Accepts JSON: `{ results: EngpickerUrlResult[], similarity_threshold: number, success_rate_threshold: number, cdp_failure_threshold: number }`
/// Returns JSON: `EngpickerVerdict`
#[wasm_bindgen]
pub fn compute_engpicker_verdict(params_json: &str) -> Result<String, JsError> {
  #[derive(serde::Deserialize)]
  struct Params {
    results: Vec<firecrawl_rs::EngpickerUrlResult>,
    similarity_threshold: f64,
    success_rate_threshold: f64,
    cdp_failure_threshold: f64,
  }
  let p: Params = from_json(params_json)?;
  let result = firecrawl_rs::compute_engpicker_verdict(
    p.results,
    p.similarity_threshold,
    p.success_rate_threshold,
    p.cdp_failure_threshold,
  )
  .map_err(map_err)?;
  json_ok(&result)
}

// ===========================================================================
// Document conversion
// ===========================================================================

/// Convert a document (DOCX/XLSX/ODT/RTF/DOC) to HTML.
/// `data_base64` is the document bytes as a base64 string.
/// `doc_type` is one of: "doc", "docx", "rtf", "odt", "xlsx".
/// Returns JSON: `string` (HTML)
#[wasm_bindgen]
pub fn convert_document(data_base64: &str, doc_type: &str) -> Result<String, JsError> {
  let data = base64::engine::general_purpose::STANDARD
    .decode(data_base64)
    .map_err(|e| JsError::new(&format!("base64 decode error: {e}")))?;

  let dtype = match doc_type.to_lowercase().as_str() {
    "doc" => firecrawl_rs::DocumentType::Doc,
    "docx" => firecrawl_rs::DocumentType::Docx,
    "rtf" => firecrawl_rs::DocumentType::Rtf,
    "odt" => firecrawl_rs::DocumentType::Odt,
    "xlsx" => firecrawl_rs::DocumentType::Xlsx,
    _ => return Err(JsError::new(&format!("Unknown document type: {doc_type}"))),
  };

  let converter = firecrawl_rs::DocumentConverter::new();
  let html = converter
    .convert_buffer_to_html(&data, dtype)
    .map_err(map_err)?;
  json_ok(&html)
}
