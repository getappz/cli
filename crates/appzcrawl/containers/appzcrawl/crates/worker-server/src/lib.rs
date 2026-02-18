//! appzcrawl worker-server: Cloudflare Worker (via workers-rs) exposing
//! firecrawl_rs functions as **RPC exports** via `#[wasm_bindgen]`.
//!
//! Other Workers (e.g. the main appzcrawl API) call these directly through
//! a Service Binding — no HTTP routing, no JSON serialization overhead.
//!
//! ```ts
//! // In the main Worker (TypeScript):
//! const links = env.WORKER_SERVER.extract_links(html);
//! const md    = env.WORKER_SERVER.html_to_markdown(html);
//! ```
//!
//! ## What's included
//! - All HTML/crawl functions from firecrawl_rs (sync, compiled to WASM)
//! - html-to-markdown via htmd (pure Rust, replaces Go FFI)
//! - postProcessMarkdown with citations support
//!
//! ## What's NOT included (returns error string)
//! - PDF conversion/metadata (lopdf needs filesystem)

mod postprocess;
mod search;

use base64::Engine;
use firecrawl_rs::{
  extract_assets, extract_attributes, extract_base_href, extract_images, extract_links,
  extract_metadata, filter_links, get_inner_json, post_process_markdown, process_sitemap,
  transform_html, DocumentConverter, DocumentType, AttributeSelector, ExtractAttributesOptions,
  FilterLinksCall, TransformHtmlOptions,
};
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::prelude::wasm_bindgen;
use worker::*;

// =========================================================================
// RPC exports — called directly from other Workers via Service Binding
// All functions: JSON string in → JSON string out
// =========================================================================

/// Extract all `<a href>` links from HTML.
/// Input: HTML string (or empty string for null).
/// Returns JSON: `["url1", "url2", ...]`
#[wasm_bindgen]
pub fn rpc_extract_links(html: String) -> String {
  let html = if html.is_empty() { None } else { Some(html) };
  match extract_links(html) {
    Ok(links) => serde_json::to_string(&links).unwrap_or_else(|_| "[]".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract `<base href>` or derive from URL.
/// Input: JSON `{"html":"...","url":"..."}`
/// Returns JSON: `"base_href_string"`
#[wasm_bindgen]
pub fn rpc_extract_base_href(html: String, url: String) -> String {
  match extract_base_href(html, url) {
    Ok(href) => serde_json::to_string(&href).unwrap_or_else(|_| "\"\"".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract metadata (title, og:*, meta tags, etc.) from HTML.
/// Input: HTML string.
/// Returns JSON: `{title, description, ...}`
#[wasm_bindgen]
pub fn rpc_extract_metadata(html: String) -> String {
  let html = if html.is_empty() { None } else { Some(html) };
  match extract_metadata(html) {
    Ok(meta) => serde_json::to_string(&meta).unwrap_or_else(|_| "{}".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Transform/clean HTML (remove scripts, apply OMCE, resolve URLs, etc.).
/// Input: JSON `{"html","url","include_tags","exclude_tags","only_main_content","omce_signatures"}`
/// Returns JSON: `"cleaned_html"`
#[wasm_bindgen]
pub fn rpc_transform_html(opts_json: String) -> String {
  #[derive(Deserialize)]
  struct Opts {
    html: String,
    url: String,
    #[serde(default)]
    include_tags: Vec<String>,
    #[serde(default)]
    exclude_tags: Vec<String>,
    #[serde(default)]
    only_main_content: bool,
    #[serde(default)]
    omce_signatures: Option<Vec<String>>,
  }

  let opts: Opts = match serde_json::from_str(&opts_json) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("parse error: {e}")}).to_string(),
  };

  match transform_html(TransformHtmlOptions {
    html: opts.html,
    url: opts.url,
    include_tags: opts.include_tags,
    exclude_tags: opts.exclude_tags,
    only_main_content: opts.only_main_content,
    omce_signatures: opts.omce_signatures,
  }) {
    Ok(html) => serde_json::to_string(&html).unwrap_or_else(|_| "\"\"".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract text content from HTML `<body>`.
/// Input: HTML string.
/// Returns JSON: `"text_content"`
#[wasm_bindgen]
pub fn rpc_get_inner_json(html: String) -> String {
  match get_inner_json(html) {
    Ok(content) => serde_json::to_string(&content).unwrap_or_else(|_| "\"\"".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract specific attributes from CSS selector matches.
/// Input: JSON `{"html","selectors":[{"selector","attribute"}]}`
/// Returns JSON: `[{"selector","attribute","values":[...]}]`
#[wasm_bindgen]
pub fn rpc_extract_attributes(html: String, options_json: String) -> String {
  #[derive(Deserialize)]
  struct Sel {
    selector: String,
    attribute: String,
  }
  #[derive(Deserialize)]
  struct Opts {
    selectors: Vec<Sel>,
  }

  let opts: Opts = match serde_json::from_str(&options_json) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("parse error: {e}")}).to_string(),
  };

  let options = ExtractAttributesOptions {
    selectors: opts
      .selectors
      .into_iter()
      .map(|s| AttributeSelector {
        selector: s.selector,
        attribute: s.attribute,
      })
      .collect(),
  };

  match extract_attributes(html, options) {
    Ok(results) => serde_json::to_string(&results).unwrap_or_else(|_| "[]".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract all image URLs from HTML.
/// Input: html, base_url.
/// Returns JSON: `["url1", "url2", ...]`
#[wasm_bindgen]
pub fn rpc_extract_images(html: String, base_url: String) -> String {
  match extract_images(html, base_url) {
    Ok(images) => serde_json::to_string(&images).unwrap_or_else(|_| "[]".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Extract assets (images, CSS, JS, fonts, etc.) from HTML.
/// Input: html, base_url, formats_json (e.g. `["css","js"]` or `["assets"]`).
/// Returns JSON: `{images:[...], css:[...], ...}`
#[wasm_bindgen]
pub fn rpc_extract_assets(html: String, base_url: String, formats_json: String) -> String {
  let formats: Vec<String> = serde_json::from_str(&formats_json).unwrap_or_else(|_| vec!["assets".into()]);
  match extract_assets(html, base_url, formats) {
    Ok(assets) => serde_json::to_string(&assets).unwrap_or_else(|_| "{}".into()),
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Post-process markdown (fix links, code blocks, optional citations).
/// Input: markdown, base_url (or empty), citations (bool as "true"/"false").
/// Returns JSON: `"processed_markdown"`
#[wasm_bindgen]
pub fn rpc_post_process_markdown(markdown: String, base_url: String, citations: bool) -> String {
  match post_process_markdown(markdown) {
    Ok(md) => {
      let md = postprocess::fix_code_blocks(&md);
      let md = if citations {
        postprocess::convert_links_to_citations(&md, &base_url)
      } else {
        md
      };
      serde_json::to_string(&md).unwrap_or_else(|_| "\"\"".into())
    }
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Convert HTML to markdown using pure Rust (htmd).
/// Replaces the Go FFI html2md library — works in WASM.
/// Input: HTML string.
/// Returns JSON: `"markdown_string"`
#[wasm_bindgen]
pub fn rpc_html_to_markdown(html: String) -> String {
  let md = htmd::convert(&html).unwrap_or_default();
  serde_json::to_string(&md).unwrap_or_else(|_| "\"\"".into())
}

/// Filter links based on crawling rules (depth, robots.txt, excludes, etc.).
/// Input: JSON `FilterLinksCall`.
/// Returns JSON: `{"links":[...],"denial_reasons":{...}}`
#[wasm_bindgen]
pub fn rpc_filter_links(params_json: String) -> String {
  let call: FilterLinksCall = match serde_json::from_str(&params_json) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("parse error: {e}")}).to_string(),
  };
  match filter_links(call) {
    Ok(result) => json!({
      "links": result.links,
      "denial_reasons": result.denial_reasons,
    })
    .to_string(),
    Err(e) => json!({"error": e}).to_string(),
  }
}

/// Parse XML sitemap and extract URLs.
/// Input: XML string.
/// Returns JSON: `{"urls":[...],"sitemapUrls":[...]}`
#[wasm_bindgen]
pub fn rpc_parse_sitemap(xml: String) -> String {
  match process_sitemap(xml) {
    Ok(result) => {
      let mut urls: Vec<String> = Vec::new();
      let mut sitemap_urls: Vec<String> = Vec::new();
      for inst in &result.instructions {
        if inst.action == "process" {
          urls.extend(inst.urls.clone());
        } else if inst.action == "recurse" {
          sitemap_urls.extend(inst.urls.clone());
        }
      }
      json!({"urls": urls, "sitemapUrls": sitemap_urls}).to_string()
    }
    Err(e) => json!({"error": e.to_string()}).to_string(),
  }
}

/// Convert office documents (DOCX, DOC, ODT, RTF, XLSX) to HTML.
/// Input: data_base64, url (optional), content_type (optional) — as JSON.
/// Returns JSON: `"html_string"` or `{"error":"..."}`
#[wasm_bindgen]
pub fn rpc_convert_document(params_json: String) -> String {
  #[derive(Deserialize)]
  struct Params {
    #[serde(alias = "dataBase64")]
    data_base64: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, alias = "contentType")]
    content_type: Option<String>,
  }

  let params: Params = match serde_json::from_str(&params_json) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("parse error: {e}")}).to_string(),
  };

  let Some(b64) = params.data_base64 else {
    return json!({"error": "dataBase64 required"}).to_string();
  };

  let data = match base64::engine::general_purpose::STANDARD.decode(b64) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("invalid base64: {e}")}).to_string(),
  };

  let doc_type = params
    .content_type
    .as_ref()
    .and_then(|ct| document_type_from_content_type(ct))
    .or_else(|| params.url.as_ref().map(|u| document_type_from_url(u)))
    .unwrap_or(DocumentType::Docx);

  let converter = DocumentConverter::new();
  match converter.convert_buffer_to_html(&data, doc_type) {
    Ok(html) => serde_json::to_string(&html).unwrap_or_else(|_| "\"\"".into()),
    Err(e) => json!({"error": e}).to_string(),
  }
}

/// Web search using DuckDuckGo via Workers fetch API.
/// Input: query string, options JSON `{num_results, lang, country, tbs, ...}`.
/// Returns JSON: `{"web":[{"url","title","description"},...]}`
/// This is an async RPC export — requires network (Workers fetch).
#[wasm_bindgen]
pub async fn rpc_search(query: String, options_json: String) -> String {
  let options: search::SearchOptions = match serde_json::from_str(&options_json) {
    Ok(v) => v,
    Err(e) => return json!({"error": format!("parse error: {e}")}).to_string(),
  };
  match search::ddg_search(&query, &options).await {
    Ok(resp) => serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"web":null}"#.into()),
    Err(e) => json!({"error": e}).to_string(),
  }
}

// =========================================================================
// Helpers (shared by RPC and HTTP handlers)
// =========================================================================

fn document_type_from_url(url: &str) -> DocumentType {
  let u = url.to_lowercase();
  if u.ends_with(".docx") || u.contains(".docx/") { return DocumentType::Docx; }
  if u.ends_with(".doc") || u.contains(".doc/") { return DocumentType::Doc; }
  if u.ends_with(".odt") || u.contains(".odt/") { return DocumentType::Odt; }
  if u.ends_with(".rtf") || u.contains(".rtf/") { return DocumentType::Rtf; }
  if u.ends_with(".xlsx") || u.ends_with(".xls") || u.contains(".xlsx/") || u.contains(".xls/") {
    return DocumentType::Xlsx;
  }
  DocumentType::Docx
}

fn document_type_from_content_type(ct: &str) -> Option<DocumentType> {
  let ct = ct.to_lowercase();
  if ct.contains("application/vnd.openxmlformats-officedocument.wordprocessingml.document") {
    return Some(DocumentType::Docx);
  }
  if ct.contains("application/msword") { return Some(DocumentType::Doc); }
  if ct.contains("application/vnd.oasis.opendocument.text") { return Some(DocumentType::Odt); }
  if ct.contains("application/rtf") || ct.contains("text/rtf") { return Some(DocumentType::Rtf); }
  if ct.contains("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    || ct.contains("application/vnd.ms-excel")
  {
    return Some(DocumentType::Xlsx);
  }
  None
}

// =========================================================================
// HTTP fetch handler (for standalone testing / direct HTTP access)
// =========================================================================

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
  Router::new()
    .get_async("/health", |_, _| async { Response::from_json(&json!({"ok": true})) })
    .post_async("/extract-links", |mut req, _| async move {
      let b = req.bytes().await?;
      let html = String::from_utf8(b).unwrap_or_default();
      let result = rpc_extract_links(html);
      let mut resp = Response::ok(result)?;
      resp.headers_mut().set("content-type", "application/json")?;
      Ok(resp)
    })
    .post_async("/html-to-markdown", |mut req, _| async move {
      let b = req.bytes().await?;
      let v: serde_json::Value = serde_json::from_slice(&b).unwrap_or_default();
      let html = v.get("html").and_then(|h| h.as_str()).unwrap_or_default();
      let result = rpc_html_to_markdown(html.to_string());
      let mut resp = Response::ok(format!(r#"{{"markdown":{result}}}"#))?;
      resp.headers_mut().set("content-type", "application/json")?;
      Ok(resp)
    })
    .run(req, env)
    .await
}
