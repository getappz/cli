mod html2md;
mod postprocess;

use std::{io::Write, net::SocketAddr};

use axum::{
  body::Bytes,
  extract::DefaultBodyLimit,
  http::StatusCode,
  response::{IntoResponse, Response},
  routing::{get, post},
  Json, Router,
};
use base64::Engine;
use firecrawl_rs::{
  convert_pdf_to_html, extract_assets, extract_attributes, extract_base_href, extract_images,
  extract_links, extract_metadata, filter_links, get_inner_json, get_pdf_metadata, post_process_markdown,
  process_sitemap, transform_html, DocumentConverter, DocumentType,
  AttributeSelector, ExtractAttributesOptions, FilterLinksCall, TransformHtmlOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tempfile::NamedTempFile;

#[derive(Serialize)]
struct ErrorBody {
  error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
  (status, Json(ErrorBody { error: message.into() })).into_response()
}

#[derive(Deserialize)]
struct TransformBody {
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

#[derive(Deserialize)]
struct AttrSelectorBody {
  selector: String,
  attribute: String,
}

#[derive(Deserialize)]
struct ExtractAttributesBody {
  html: String,
  options: ExtractAttributesBodyOptions,
}

#[derive(Deserialize)]
struct ExtractAttributesBodyOptions {
  selectors: Vec<AttrSelectorBody>,
}

#[derive(Deserialize)]
struct ExtractImagesBody {
  html: String,
  base_url: String,
}

#[derive(Deserialize)]
struct ExtractAssetsBody {
  html: String,
  base_url: String,
  #[serde(default)]
  formats: Vec<String>,
}

#[derive(Deserialize)]
struct PdfBase64Body {
  #[serde(default, alias = "pdfBase64")]
  pdf_base64: Option<String>,
}

#[derive(Deserialize)]
struct ConvertDocumentBody {
  #[serde(alias = "dataBase64")]
  data_base64: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default, alias = "contentType")]
  content_type: Option<String>,
}

#[derive(Deserialize)]
struct PostProcessMarkdownBody {
  markdown: String,
  #[serde(default, alias = "baseUrl")]
  base_url: Option<String>,
  #[serde(default)]
  citations: bool,
}

#[derive(Deserialize)]
struct ParseSitemapBody {
  xml: String,
}

#[derive(Deserialize)]
struct ExtractBaseHrefBody {
  html: String,
  url: String,
}

async fn health() -> impl IntoResponse {
  Json(json!({ "ok": true }))
}

fn parse_json_or_text(bytes: &Bytes) -> Option<Value> {
  if bytes.is_empty() {
    return None;
  }

  serde_json::from_slice::<Value>(bytes).ok().or_else(|| {
    String::from_utf8(bytes.to_vec())
      .ok()
      .map(Value::String)
  })
}

fn get_html_field(body: Option<Value>) -> Option<String> {
  match body {
    Some(Value::Object(map)) => map.get("html").and_then(|v| v.as_str().map(ToOwned::to_owned)),
    Some(Value::String(s)) => Some(s),
    _ => None,
  }
}

async fn extract_links_handler(body: Bytes) -> Response {
  let html = get_html_field(parse_json_or_text(&body));
  match extract_links(html).await {
    Ok(links) => Json(json!({ "links": links })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn extract_base_href_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ExtractBaseHrefBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "html and url required"),
  };
  match extract_base_href(parsed.html, parsed.url).await {
    Ok(base_href) => Json(json!({ "baseHref": base_href })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn transform_html_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<TransformBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "html and url required"),
  };

  let opts = TransformHtmlOptions {
    html: parsed.html,
    url: parsed.url,
    include_tags: parsed.include_tags,
    exclude_tags: parsed.exclude_tags,
    only_main_content: parsed.only_main_content,
    omce_signatures: parsed.omce_signatures,
  };

  match transform_html(opts).await {
    Ok(html) => Json(json!({ "html": html })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn get_inner_json_handler(body: Bytes) -> Response {
  let html = get_html_field(parse_json_or_text(&body));
  let Some(html) = html else {
    return json_error(StatusCode::BAD_REQUEST, "html required");
  };

  match get_inner_json(html).await {
    Ok(content) => Json(json!({ "content": content })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn extract_metadata_handler(body: Bytes) -> Response {
  let html = get_html_field(parse_json_or_text(&body));
  match extract_metadata(html).await {
    Ok(metadata) => Json(json!({ "metadata": metadata })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn extract_attributes_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ExtractAttributesBody>(&body) {
    Ok(v) => v,
    Err(_) => {
      return json_error(
        StatusCode::BAD_REQUEST,
        "html and options.selectors required",
      );
    }
  };

  let options = ExtractAttributesOptions {
    selectors: parsed
      .options
      .selectors
      .into_iter()
      .map(|s| AttributeSelector {
        selector: s.selector,
        attribute: s.attribute,
      })
      .collect(),
  };

  match extract_attributes(parsed.html, options).await {
    Ok(results) => Json(json!({ "results": results })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn extract_images_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ExtractImagesBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "html and base_url required"),
  };

  match extract_images(parsed.html, parsed.base_url).await {
    Ok(images) => Json(json!({ "images": images })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn extract_assets_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ExtractAssetsBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "html and base_url required"),
  };

  // Default to all assets if no formats specified
  let formats = if parsed.formats.is_empty() {
    vec!["assets".to_string()]
  } else {
    parsed.formats
  };

  match extract_assets(parsed.html, parsed.base_url, formats).await {
    Ok(assets) => Json(json!(assets)).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn post_process_markdown_handler(body: Bytes) -> Response {
  let (markdown, base_url, citations) = match serde_json::from_slice::<PostProcessMarkdownBody>(&body)
  {
    Ok(parsed) => (Some(parsed.markdown), parsed.base_url, parsed.citations),
    Err(_) => {
      match parse_json_or_text(&body) {
        Some(Value::Object(map)) => {
          let markdown = map
            .get("markdown")
            .and_then(|x| x.as_str())
            .map(ToOwned::to_owned);
          let base_url = map
            .get("baseUrl")
            .or_else(|| map.get("base_url"))
            .and_then(|x| x.as_str())
            .map(ToOwned::to_owned);
          let citations = map
            .get("citations")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
          (markdown, base_url, citations)
        }
        Some(Value::String(s)) => (Some(s), None, false),
        _ => (None, None, false),
      }
    }
  };

  let Some(markdown) = markdown else {
    return json_error(StatusCode::BAD_REQUEST, "markdown required");
  };

  match post_process_markdown(markdown).await {
    Ok(md) => {
      let md = postprocess::fix_code_blocks(&md);
      let md = if citations {
        postprocess::convert_links_to_citations(
          &md,
          base_url.as_deref().unwrap_or_default(),
        )
      } else {
        md
      };
      Json(json!({ "markdown": md })).into_response()
    }
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn get_pdf_metadata_handler(body: Bytes) -> Response {
  let input_pdf = if body.first() == Some(&b'{') {
    let parsed = match serde_json::from_slice::<PdfBase64Body>(&body) {
      Ok(v) => v,
      Err(_) => return json_error(StatusCode::BAD_REQUEST, "pdfBase64 required"),
    };

    let b64 = parsed.pdf_base64;
    let Some(b64) = b64 else {
      return json_error(StatusCode::BAD_REQUEST, "pdfBase64 required");
    };

    match base64::engine::general_purpose::STANDARD.decode(b64) {
      Ok(v) => v,
      Err(e) => return json_error(StatusCode::BAD_REQUEST, format!("invalid base64: {e}")),
    }
  } else {
    body.to_vec()
  };

  let mut tmp = match NamedTempFile::new() {
    Ok(f) => f,
    Err(e) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  };

  if let Err(e) = tmp.write_all(&input_pdf) {
    return json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
  }

  let path = tmp.path().to_string_lossy().to_string();
  match get_pdf_metadata(path) {
    Ok(meta) => Json(json!({ "num_pages": meta.num_pages, "title": meta.title })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  }
}

async fn html_to_markdown_handler(body: Bytes) -> Response {
  let html = get_html_field(parse_json_or_text(&body));
  let Some(html) = html else {
    return json_error(StatusCode::BAD_REQUEST, "html required");
  };
  let markdown = html2md::convert_html_to_markdown(&html);
  Json(json!({ "markdown": markdown })).into_response()
}

async fn filter_links_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<FilterLinksCall>(&body) {
    Ok(v) => v,
    Err(e) => {
      return json_error(
        StatusCode::BAD_REQUEST,
        format!("invalid filter-links body: {e}"),
      );
    }
  };

  match filter_links(parsed).await {
    Ok(result) => Json(json!({
      "links": result.links,
      "denialReasons": result.denial_reasons,
    }))
    .into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
  }
}

async fn parse_sitemap_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ParseSitemapBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "xml required"),
  };

  let result = match process_sitemap(parsed.xml).await {
    Ok(r) => r,
    Err(e) => return json_error(StatusCode::BAD_REQUEST, e.to_string()),
  };

  let mut urls: Vec<String> = Vec::new();
  let mut sitemap_urls: Vec<String> = Vec::new();
  for inst in &result.instructions {
    if inst.action == "process" {
      urls.extend(inst.urls.clone());
    } else if inst.action == "recurse" {
      sitemap_urls.extend(inst.urls.clone());
    }
  }

  Json(serde_json::json!({
    "urls": urls,
    "sitemapUrls": sitemap_urls,
  }))
  .into_response()
}

async fn search_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<appzcrawl_search::SearchOptions>(&body) {
    Ok(mut v) => {
      // Set default timeout if not provided
      if v.timeout_ms == 0 {
        v.timeout_ms = 5000;
      }
      v
    }
    Err(e) => {
      return json_error(
        StatusCode::BAD_REQUEST,
        format!("invalid search body: {}", e),
      );
    }
  };

  // Get query from body
  let body_json: Value = match serde_json::from_slice(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "invalid JSON"),
  };

  let query = match body_json.get("query").and_then(|q| q.as_str()) {
    Some(q) => q,
    None => return json_error(StatusCode::BAD_REQUEST, "query required"),
  };

  // Check for SEARXNG_ENDPOINT env var
  let searxng_endpoint = std::env::var("SEARXNG_ENDPOINT").ok();

  match appzcrawl_search::search_with_fallback(query, searxng_endpoint.as_deref(), &parsed).await {
    Ok(result) => Json(serde_json::to_value(&result).unwrap()).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
  }
}

fn document_type_from_url(url: &str) -> DocumentType {
  let url_lower = url.to_lowercase();
  if url_lower.ends_with(".docx") || url_lower.contains(".docx/") {
    return DocumentType::Docx;
  }
  if url_lower.ends_with(".doc") || url_lower.contains(".doc/") {
    return DocumentType::Doc;
  }
  if url_lower.ends_with(".odt") || url_lower.contains(".odt/") {
    return DocumentType::Odt;
  }
  if url_lower.ends_with(".rtf") || url_lower.contains(".rtf/") {
    return DocumentType::Rtf;
  }
  if url_lower.ends_with(".xlsx")
    || url_lower.ends_with(".xls")
    || url_lower.contains(".xlsx/")
    || url_lower.contains(".xls/")
  {
    return DocumentType::Xlsx;
  }
  DocumentType::Docx
}

fn document_type_from_content_type(ct: &str) -> Option<DocumentType> {
  let ct = ct.to_lowercase();
  if ct.contains("application/vnd.openxmlformats-officedocument.wordprocessingml.document") {
    return Some(DocumentType::Docx);
  }
  if ct.contains("application/msword") {
    return Some(DocumentType::Doc);
  }
  if ct.contains("application/vnd.oasis.opendocument.text") {
    return Some(DocumentType::Odt);
  }
  if ct.contains("application/rtf") || ct.contains("text/rtf") {
    return Some(DocumentType::Rtf);
  }
  if ct.contains("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    || ct.contains("application/vnd.ms-excel")
  {
    return Some(DocumentType::Xlsx);
  }
  None
}

async fn convert_document_handler(body: Bytes) -> Response {
  let (data, url, content_type) = if body.first() == Some(&b'{') {
    let parsed = match serde_json::from_slice::<ConvertDocumentBody>(&body) {
      Ok(v) => v,
      Err(_) => return json_error(StatusCode::BAD_REQUEST, "dataBase64 required"),
    };

    let b64 = parsed.data_base64;
    let Some(b64) = b64 else {
      return json_error(StatusCode::BAD_REQUEST, "dataBase64 required");
    };

    let data = match base64::engine::general_purpose::STANDARD.decode(b64) {
      Ok(v) => v,
      Err(e) => return json_error(StatusCode::BAD_REQUEST, format!("invalid base64: {e}")),
    };

    (data, parsed.url, parsed.content_type)
  } else {
    return json_error(
      StatusCode::BAD_REQUEST,
      "JSON body with dataBase64, url (optional), contentType (optional) required",
    );
  };

  let doc_type = content_type
    .as_ref()
    .and_then(|ct| document_type_from_content_type(ct))
    .or_else(|| url.as_ref().map(|u| document_type_from_url(u)))
    .unwrap_or(DocumentType::Docx);

  let converter = DocumentConverter::new();
  match converter.convert_buffer_to_html(&data, doc_type) {
    Ok(html) => Json(json!({ "html": html })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
  }
}

async fn convert_pdf_handler(body: Bytes) -> Response {
  let parsed = match serde_json::from_slice::<ConvertDocumentBody>(&body) {
    Ok(v) => v,
    Err(_) => return json_error(StatusCode::BAD_REQUEST, "dataBase64 required"),
  };

  let Some(b64) = parsed.data_base64 else {
    return json_error(StatusCode::BAD_REQUEST, "dataBase64 required");
  };

  let data = match base64::engine::general_purpose::STANDARD.decode(b64) {
    Ok(v) => v,
    Err(e) => return json_error(StatusCode::BAD_REQUEST, format!("invalid base64: {e}")),
  };

  match convert_pdf_to_html(&data) {
    Ok(html) => Json(json!({ "html": html })).into_response(),
    Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, e),
  }
}

#[tokio::main]
async fn main() {
  let port = std::env::var("PORT")
    .ok()
    .and_then(|x| x.parse::<u16>().ok())
    .unwrap_or(4000);

  let app = Router::new()
    .route("/health", get(health))
    .route("/extract-links", post(extract_links_handler))
    .route("/filter-links", post(filter_links_handler))
    .route("/extract-base-href", post(extract_base_href_handler))
    .route("/transform-html", post(transform_html_handler))
    .route("/get-inner-json", post(get_inner_json_handler))
    .route("/extract-metadata", post(extract_metadata_handler))
    .route("/extract-attributes", post(extract_attributes_handler))
    .route("/extract-images", post(extract_images_handler))
    .route("/extract-assets", post(extract_assets_handler))
    .route("/post-process-markdown", post(post_process_markdown_handler))
    .route("/html-to-markdown", post(html_to_markdown_handler))
    .route("/get-pdf-metadata", post(get_pdf_metadata_handler))
    .route("/convert-document", post(convert_document_handler))
    .route("/convert-pdf", post(convert_pdf_handler))
    .route("/parse-sitemap", post(parse_sitemap_handler))
    .route("/search", post(search_handler))
    .layer(DefaultBodyLimit::max(10 * 1024 * 1024));

  let addr = SocketAddr::from(([0, 0, 0, 0], port));
  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .expect("bind failed");
  axum::serve(listener, app).await.expect("server failed");
}
