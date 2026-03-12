use std::collections::{HashMap, HashSet};

use std::sync::LazyLock;

use kuchikiki::{iter::NodeEdge, parse_html, traits::TendrilSink, NodeRef};
use nodesig::{get_node_signature, SignatureMode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "native")]
use tokio::task;
use url::Url;

static URL_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"url\(['"]?([^'")]+)['"]?\)"#).expect("URL_REGEX is a valid static regex pattern"));

// background-image: url(...) or background: ... url(...) inside <style> blocks
static BACKGROUND_IMAGE_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"(?i)(?:background-image|background)\s*:\s*[^;}]*?url\s*\(\s*['"]?([^'")\s]+)['"]?\s*\)"#)
    .expect("BACKGROUND_IMAGE_URL_REGEX is a valid static regex pattern")
});

use crate::utils::to_err;

fn _extract_base_href_from_document(
  document: &NodeRef,
  url: &Url,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  if let Some(base) = document
    .select("base[href]")
    .map_err(|_| "Failed to select base href".to_string())?
    .next()
    .and_then(|base| base.attributes.borrow().get("href").map(|x| x.to_string()))
  {
    if let Ok(base) = url.join(&base) {
      return Ok(base.to_string());
    }
  }

  Ok(url.to_string())
}

fn _extract_base_href(
  html: &str,
  url: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let url = Url::parse(url)?;
  _extract_base_href_from_document(&document, &url)
}

/// Extract the base href from HTML document.
#[cfg(feature = "native")]
pub async fn extract_base_href(html: String, url: String) -> Result<String, String> {
  let res = task::spawn_blocking(move || _extract_base_href(&html, &url))
    .await
    .map_err(|e| format!("extract_base_href join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract the base href from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_base_href(html: String, url: String) -> Result<String, String> {
  _extract_base_href(&html, &url).map_err(to_err)
}

fn _extract_links(html: Option<String>) -> Result<Vec<String>, String> {
  let html = match html {
    Some(h) => h,
    None => return Ok(Vec::new()),
  };

  let document = parse_html().one(html.as_str());

  let anchors: Vec<_> = document
    .select("a[href]")
    .map_err(|_| to_err("Failed to select links"))?
    .collect();

  let mut out: Vec<String> = Vec::new();

  for anchor in anchors {
    let mut href = match anchor.attributes.borrow().get("href") {
      Some(x) => x.to_string(),
      None => continue,
    };

    if href.starts_with("http:/") && !href.starts_with("http://") {
      href = format!("http://{}", &href[6..]);
    } else if href.starts_with("https:/") && !href.starts_with("https://") {
      href = format!("https://{}", &href[7..]);
    }

    out.push(href);
  }

  Ok(out)
}

/// Extract all links from HTML document.
#[cfg(feature = "native")]
pub async fn extract_links(html: Option<String>) -> Result<Vec<String>, String> {
  task::spawn_blocking(move || _extract_links(html))
    .await
    .map_err(|e| format!("extract_links join error: {e}"))?
}

/// Extract all links from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_links(html: Option<String>) -> Result<Vec<String>, String> {
  _extract_links(html)
}

macro_rules! insert_meta_name {
  ($out:ident, $document:ident, $metaName:expr, $outName:expr) => {
    if let Some(x) = $document
      .select(&format!("meta[name=\"{}\"]", $metaName))
      .map_err(|_| "Failed to select meta name")?
      .next()
      .and_then(|description| {
        description
          .attributes
          .borrow()
          .get("content")
          .map(|x| x.to_string())
      })
    {
      $out.insert(($outName).to_string(), Value::String(x));
    }
  };
}

macro_rules! insert_meta_property {
  ($out:ident, $document:ident, $metaName:expr, $outName:expr) => {
    if let Some(x) = $document
      .select(&format!("meta[property=\"{}\"]", $metaName))
      .map_err(|_| "Failed to select meta property")?
      .next()
      .and_then(|description| {
        description
          .attributes
          .borrow()
          .get("content")
          .map(|x| x.to_string())
      })
    {
      $out.insert(($outName).to_string(), Value::String(x));
    }
  };
}

fn _extract_metadata(
  html: &str,
) -> Result<HashMap<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let mut out = HashMap::<String, Value>::new();

  let head_node = document
    .select("head")
    .map_err(|_| "Failed to select head")?
    .next();

  let search_root = head_node.as_ref().map(|h| h.as_node()).unwrap_or(&document);

  if let Some(title) = search_root
    .select("title")
    .map_err(|_| "Failed to select title")?
    .next()
  {
    out.insert("title".to_string(), Value::String(title.text_contents()));
  }

  if let Some(favicon_link) = search_root
    .select("link[rel=\"icon\"]")
    .map_err(|_| "Failed to select favicon")?
    .next()
    .and_then(|x| x.attributes.borrow().get("href").map(|x| x.to_string()))
    .or_else(|| {
      search_root
        .select("link[rel*=\"icon\"]")
        .ok()
        .and_then(|mut x| {
          x.next()
            .and_then(|x| x.attributes.borrow().get("href").map(|x| x.to_string()))
        })
    })
  {
    out.insert("favicon".to_string(), Value::String(favicon_link));
  }

  if let Some(lang) = document
    .select("html[lang]")
    .map_err(|_| "Failed to select lang")?
    .next()
    .and_then(|x| x.attributes.borrow().get("lang").map(|x| x.to_string()))
  {
    out.insert("language".to_string(), Value::String(lang));
  }

  insert_meta_property!(out, search_root, "og:title", "ogTitle");
  insert_meta_property!(out, search_root, "og:description", "ogDescription");
  insert_meta_property!(out, search_root, "og:url", "ogUrl");
  insert_meta_property!(out, search_root, "og:image", "ogImage");
  insert_meta_property!(out, search_root, "og:audio", "ogAudio");
  insert_meta_property!(out, search_root, "og:determiner", "ogDeterminer");
  insert_meta_property!(out, search_root, "og:locale", "ogLocale");

  for meta in search_root
    .select("meta[property=\"og:locale:alternate\"]")
    .map_err(|_| "Failed to select og locale alternate")?
  {
    let attrs = meta.attributes.borrow();

    if let Some(content) = attrs.get("content") {
      if let Some(v) = out.get_mut("ogLocaleAlternate") {
        match v {
          Value::Array(x) => x.push(Value::String(content.to_string())),
          _ => unreachable!(),
        }
      } else {
        out.insert(
          "ogLocaleAlternate".to_string(),
          Value::Array(vec![Value::String(content.to_string())]),
        );
      }
    }
  }

  insert_meta_property!(out, document, "og:site_name", "ogSiteName");
  insert_meta_property!(out, document, "og:video", "ogVideo");
  insert_meta_name!(out, document, "article:section", "articleSection");
  insert_meta_name!(out, document, "article:tag", "articleTag");
  insert_meta_property!(out, document, "article:published_time", "publishedTime");
  insert_meta_property!(out, document, "article:modified_time", "modifiedTime");
  insert_meta_name!(out, document, "dcterms.keywords", "dcTermsKeywords");
  insert_meta_name!(out, document, "dc.description", "dcDescription");
  insert_meta_name!(out, document, "dc.subject", "dcSubject");
  insert_meta_name!(out, document, "dcterms.subject", "dcTermsSubject");
  insert_meta_name!(out, document, "dcterms.audience", "dcTermsAudience");
  insert_meta_name!(out, document, "dc.type", "dcType");
  insert_meta_name!(out, document, "dcterms.type", "dcTermsType");
  insert_meta_name!(out, document, "dc.date", "dcDate");
  insert_meta_name!(out, document, "dc.date.created", "dcDateCreated");
  insert_meta_name!(out, document, "dcterms.created", "dcTermsCreated");

  for meta in document
    .select("meta")
    .map_err(|_| "Failed to select meta")?
  {
    let meta = meta.as_node().as_element().unwrap();
    let attrs = meta.attributes.borrow();

    if let Some(name) = attrs
      .get("name")
      .or_else(|| attrs.get("property"))
      .or_else(|| attrs.get("itemprop"))
    {
      if let Some(content) = attrs.get("content") {
        if let Some(v) = out.get(name) {
          match v {
            Value::String(existing) => {
              if name == "description" {
                out.insert(
                  name.to_string(),
                  Value::String(format!("{existing}, {content}")),
                );
              } else if name != "title" {
                out.insert(
                  name.to_string(),
                  Value::Array(vec![
                    Value::String(existing.clone()),
                    Value::String(content.to_string()),
                  ]),
                );
              }
            }
            Value::Array(existing_array) => {
              if name == "description" {
                let mut values: Vec<String> = existing_array
                  .iter()
                  .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                  })
                  .collect();
                values.push(content.to_string());
                out.insert(name.to_string(), Value::String(values.join(", ")));
              } else {
                match out.get_mut(name) {
                  Some(Value::Array(x)) => x.push(Value::String(content.to_string())),
                  _ => unreachable!(),
                }
              }
            }
            _ => unreachable!(),
          }
        } else {
          out.insert(name.to_string(), Value::String(content.to_string()));
        }
      }
    }
  }

  // Backfill title from og:title, twitter:title, or meta[name="title"] if primary extraction failed
  if !out.contains_key("title") {
    let fallback_title = out
      .get("ogTitle")
      .or_else(|| out.get("og:title"))
      .or_else(|| out.get("twitter:title"))
      .and_then(|v| match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
      });

    if let Some(title) = fallback_title {
      out.insert("title".to_string(), Value::String(title));
    }
  }

  Ok(out)
}

/// Extract metadata from HTML document.
#[cfg(feature = "native")]
pub async fn extract_metadata(html: Option<String>) -> Result<HashMap<String, Value>, String> {
  task::spawn_blocking(move || {
    let html = match html {
      Some(h) => h,
      None => return Ok(HashMap::new()),
    };

    _extract_metadata(&html).map_err(to_err)
  })
  .await
  .map_err(|e| format!("extract_metadata join error: {e}"))?
}

/// Extract metadata from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_metadata(html: Option<String>) -> Result<HashMap<String, Value>, String> {
  let html = match html {
    Some(h) => h,
    None => return Ok(HashMap::new()),
  };
  _extract_metadata(&html).map_err(to_err)
}

const EXCLUDE_NON_MAIN_TAGS: [&str; 42] = [
  "header",
  "footer",
  "nav",
  "aside",
  ".header",
  ".top",
  ".navbar",
  "#header",
  ".footer",
  ".bottom",
  "#footer",
  ".sidebar",
  ".side",
  ".aside",
  "#sidebar",
  ".modal",
  ".popup",
  "#modal",
  ".overlay",
  ".ad",
  ".ads",
  ".advert",
  "#ad",
  ".lang-selector",
  ".language",
  "#language-selector",
  ".social",
  ".social-media",
  ".social-links",
  "#social",
  ".menu",
  ".navigation",
  "#nav",
  ".breadcrumbs",
  "#breadcrumbs",
  ".share",
  "#share",
  ".widget",
  "#widget",
  ".cookie",
  "#cookie",
  ".fc-decoration",
];

const FORCE_INCLUDE_MAIN_TAGS: [&str; 13] = [
  "#main",
  ".swoogo-cols",
  ".swoogo-text",
  ".swoogo-table-div",
  ".swoogo-space",
  ".swoogo-alert",
  ".swoogo-sponsors",
  ".swoogo-title",
  ".swoogo-tabs",
  ".swoogo-logo",
  ".swoogo-image",
  ".swoogo-button",
  ".swoogo-agenda",
];

#[derive(Deserialize, Serialize)]
pub struct TransformHtmlOptions {
  pub html: String,
  pub url: String,
  #[serde(default)]
  pub include_tags: Vec<String>,
  #[serde(default)]
  pub exclude_tags: Vec<String>,
  pub only_main_content: bool,
  pub omce_signatures: Option<Vec<String>>,
}

struct ImageSource {
  url: String,
  size: f64,
  is_x: bool,
}

fn _transform_html_inner(
  opts: TransformHtmlOptions,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  let mut document = parse_html().one(opts.html.as_ref());
  let url = Url::parse(&_extract_base_href_from_document(
    &document,
    &Url::parse(&opts.url)?,
  )?)?;

  if !opts.include_tags.is_empty() {
    let new_document = parse_html().one("<div></div>");
    let root = new_document
      .select_first("div")
      .map_err(|_| "Failed to select root element")?;

    for x in opts.include_tags.iter() {
      let matching_nodes: Vec<_> = document
        .select(x)
        .map_err(|_| "Failed to include_tags tags")?
        .collect();
      for tag in matching_nodes {
        root.as_node().append(tag.as_node().clone());
      }
    }

    document = new_document;
  }

  while let Ok(x) = document.select_first("head") {
    x.as_node().detach();
  }
  while let Ok(x) = document.select_first("meta") {
    x.as_node().detach();
  }
  while let Ok(x) = document.select_first("noscript") {
    x.as_node().detach();
  }
  while let Ok(x) = document.select_first("style") {
    x.as_node().detach();
  }
  while let Ok(x) = document.select_first("script") {
    x.as_node().detach();
  }

  // OMCE first
  if opts.only_main_content {
    if let Some(signatures) = opts.omce_signatures.as_ref() {
      let mut nodes_to_drop: Vec<NodeRef> = Vec::new();

      let modes = signatures
        .iter()
        .map(|x| Into::<SignatureMode>::into(x.split(':').nth(1).unwrap().to_string()))
        .collect::<HashSet<_>>();

      for mode in modes {
        let matcher = format!(":{}:", Into::<String>::into(mode));
        let signatures = signatures
          .iter()
          .filter(|x| x.contains(&matcher))
          .cloned()
          .collect::<HashSet<_>>();

        for edge in document.traverse() {
          match edge {
            NodeEdge::Start(_) => {}
            NodeEdge::End(node) => {
              if node.as_element().is_none() {
                continue;
              }
              if node.text_contents().trim().is_empty() {
                continue;
              }

              let signature = get_node_signature(&node, mode);
              if signatures.contains(&signature) {
                nodes_to_drop.push(node);
              }
            }
          }
        }
      }

      for node in nodes_to_drop {
        node.detach();
      }
    }
  }

  for x in opts.exclude_tags.iter() {
    while let Ok(x) = document.select_first(x) {
      x.as_node().detach();
    }
  }

  if opts.only_main_content {
    for x in EXCLUDE_NON_MAIN_TAGS.iter() {
      let x: Vec<_> = document
        .select(x)
        .map_err(|_| "Failed to select tags")?
        .collect();
      for tag in x {
        if !FORCE_INCLUDE_MAIN_TAGS.iter().any(|x| {
          tag
            .as_node()
            .select(x)
            .is_ok_and(|mut x| x.next().is_some())
        }) {
          tag.as_node().detach();
        }
      }
    }
  }

  let srcset_images: Vec<_> = document
    .select("img[srcset]")
    .map_err(|_| "Failed to select srcset images")?
    .collect();
  for img in srcset_images {
    let mut sizes: Vec<ImageSource> = img
      .attributes
      .borrow()
      .get("srcset")
      .ok_or("Failed to get srcset")?
      .split(',')
      .filter_map(|x| {
        let tok: Vec<&str> = x.trim().split(' ').collect();
        let last_token = tok[tok.len() - 1];
        let (last_token, last_token_used) = if tok.len() > 1
          && !last_token.is_empty()
          && (last_token.ends_with('x') || last_token.ends_with('w'))
        {
          (last_token, true)
        } else {
          ("1x", false)
        };

        if let Some((last_index, _)) = last_token.char_indices().last() {
          if let Ok(parsed_size) = last_token[..last_index].parse() {
            Some(ImageSource {
              url: if last_token_used {
                tok[0..tok.len() - 1].join(" ")
              } else {
                tok.join(" ")
              },
              size: parsed_size,
              is_x: last_token.ends_with('x'),
            })
          } else {
            None
          }
        } else {
          None
        }
      })
      .collect();

    if sizes.iter().all(|x| x.is_x) {
      if let Some(src) = img.attributes.borrow().get("src").map(|x| x.to_string()) {
        sizes.push(ImageSource {
          url: src,
          size: 1.0,
          is_x: true,
        });
      }
    }

    sizes.sort_by(|a, b| {
      b.size
        .partial_cmp(&a.size)
        .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(biggest) = sizes.first() {
      img
        .attributes
        .borrow_mut()
        .insert("src", biggest.url.clone());
    }
  }

  let src_images: Vec<_> = document
    .select("img[src]")
    .map_err(|_| "Failed to select src images")?
    .collect();
  for img in src_images {
    let old = img
      .attributes
      .borrow()
      .get("src")
      .map(|x| x.to_string())
      .ok_or("Failed to get src")?;
    if let Ok(new) = url.join(&old) {
      img.attributes.borrow_mut().insert("src", new.to_string());
    }
  }

  let href_anchors: Vec<_> = document
    .select("a[href]")
    .map_err(|_| "Failed to select href anchors")?
    .collect();
  for anchor in href_anchors {
    let old = anchor
      .attributes
      .borrow()
      .get("href")
      .map(|x| x.to_string())
      .ok_or("Failed to get href")?;
    if let Ok(new) = url.join(&old) {
      anchor
        .attributes
        .borrow_mut()
        .insert("href", new.to_string());
    }
  }

  Ok(document.to_string())
}

/// Transform and clean HTML content based on provided options.
#[cfg(feature = "native")]
pub async fn transform_html(opts: TransformHtmlOptions) -> Result<String, String> {
  let res = task::spawn_blocking(move || _transform_html_inner(opts))
    .await
    .map_err(|e| format!("transform_html join error: {e}"))?;

  res.map_err(to_err)
}

/// Transform and clean HTML content based on provided options (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn transform_html(opts: TransformHtmlOptions) -> Result<String, String> {
  _transform_html_inner(opts).map_err(to_err)
}

fn _get_inner_json(html: &str) -> Result<String, ()> {
  Ok(parse_html().one(html).select_first("body")?.text_contents())
}

/// Extract inner text content from HTML body.
#[cfg(feature = "native")]
pub async fn get_inner_json(html: String) -> Result<String, String> {
  let res = task::spawn_blocking(move || _get_inner_json(&html))
    .await
    .map_err(|e| format!("get_inner_json join error: {e}"))?;

  res.map_err(|_| to_err("Failed to get inner JSON"))
}

/// Extract inner text content from HTML body (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn get_inner_json(html: String) -> Result<String, String> {
  _get_inner_json(&html).map_err(|_| to_err("Failed to get inner JSON"))
}

#[derive(Deserialize, Serialize)]
pub struct AttributeSelector {
  pub selector: String,
  pub attribute: String,
}

#[derive(Deserialize, Serialize)]
pub struct ExtractAttributesOptions {
  pub selectors: Vec<AttributeSelector>,
}

#[derive(Serialize)]
pub struct ExtractedAttributeResult {
  pub selector: String,
  pub attribute: String,
  pub values: Vec<String>,
}

fn _extract_attributes(
  html: &str,
  options: &ExtractAttributesOptions,
) -> Result<Vec<ExtractedAttributeResult>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let mut results = Vec::new();

  for selector_config in &options.selectors {
    let mut values = Vec::new();

    let elements: Vec<_> = match document.select(&selector_config.selector).map_err(|_| {
      format!(
        "Failed to select with selector: {}",
        selector_config.selector
      )
    }) {
      Ok(x) => x.collect(),
      Err(_) => Vec::new(), // invalid selector => empty list
    };

    for element in elements {
      if let Some(attr_value) = element
        .attributes
        .borrow()
        .get(selector_config.attribute.as_str())
      {
        values.push(attr_value.to_string());
        continue;
      }

      if !selector_config.attribute.starts_with("data-") {
        let data_attr = format!("data-{}", selector_config.attribute);
        if let Some(attr_value) = element.attributes.borrow().get(data_attr.as_str()) {
          values.push(attr_value.to_string());
        }
      }
    }

    results.push(ExtractedAttributeResult {
      selector: selector_config.selector.clone(),
      attribute: selector_config.attribute.clone(),
      values,
    });
  }

  Ok(results)
}

/// Extract specified attributes from HTML elements matching selectors.
#[cfg(feature = "native")]
pub async fn extract_attributes(
  html: String,
  options: ExtractAttributesOptions,
) -> Result<Vec<ExtractedAttributeResult>, String> {
  let res = task::spawn_blocking(move || _extract_attributes(&html, &options))
    .await
    .map_err(|e| format!("extract_attributes join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract specified attributes from HTML elements matching selectors (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_attributes(
  html: String,
  options: ExtractAttributesOptions,
) -> Result<Vec<ExtractedAttributeResult>, String> {
  _extract_attributes(&html, &options).map_err(to_err)
}

fn _extract_images(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut images = HashSet::<String>::new();

  let resolve_image_url = |src: &str| -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if src.starts_with("data:") || src.starts_with("blob:") {
      return Ok(src.to_string());
    }
    if src.starts_with("http://") || src.starts_with("https://") {
      return Ok(src.to_string());
    }
    if src.starts_with("//") {
      let resolved = base_url.join(src)?;
      return Ok(resolved.to_string());
    }
    let resolved = base_href_url.join(src)?;
    Ok(resolved.to_string())
  };

  // <img>
  let img_elements: Vec<_> = match document
    .select("img")
    .map_err(|_| "Failed to select img tags")
  {
    Ok(x) => x.collect(),
    Err(e) => return Err(e.into()),
  };

  for img in img_elements {
    let attrs = img.attributes.borrow();

    if let Some(src) = attrs.get("src") {
      if let Ok(resolved) = resolve_image_url(src) {
        images.insert(resolved);
      }
    }

    if let Some(data_src) = attrs.get("data-src") {
      if let Ok(resolved) = resolve_image_url(data_src) {
        images.insert(resolved);
      }
    }

    if let Some(srcset) = attrs.get("srcset") {
      for part in srcset.split(',') {
        if let Some(url) = part.split_whitespace().next() {
          if !url.is_empty() {
            if let Ok(resolved) = resolve_image_url(url) {
              images.insert(resolved);
            }
          }
        }
      }
    }
  }

  // <picture><source>
  let source_elements: Vec<_> = match document
    .select("picture source")
    .map_err(|_| "Failed to select picture source")
  {
    Ok(x) => x.collect(),
    Err(_) => Vec::new(),
  };

  for source in source_elements {
    if let Some(srcset) = source.attributes.borrow().get("srcset") {
      for part in srcset.split(',') {
        if let Some(url) = part.split_whitespace().next() {
          if !url.is_empty() {
            if let Ok(resolved) = resolve_image_url(url) {
              images.insert(resolved);
            }
          }
        }
      }
    }
  }

  // OG/Twitter images
  let meta_selectors = [
    "meta[property=\"og:image\"]",
    "meta[property=\"og:image:url\"]",
    "meta[property=\"og:image:secure_url\"]",
    "meta[name=\"twitter:image\"]",
    "meta[name=\"twitter:image:src\"]",
    "meta[itemprop=\"image\"]",
  ];

  for selector in &meta_selectors {
    if let Ok(elements) = document.select(selector) {
      for element in elements {
        if let Some(content) = element.attributes.borrow().get("content") {
          if let Ok(resolved) = resolve_image_url(content) {
            images.insert(resolved);
          }
        }
      }
    }
  }

  // icons
  let link_selectors = [
    "link[rel*=\"icon\"]",
    "link[rel*=\"apple-touch-icon\"]",
    "link[rel*=\"image_src\"]",
  ];

  for selector in &link_selectors {
    if let Ok(elements) = document.select(selector) {
      for element in elements {
        if let Some(href) = element.attributes.borrow().get("href") {
          if let Ok(resolved) = resolve_image_url(href) {
            images.insert(resolved);
          }
        }
      }
    }
  }

  // <video poster="">
  if let Ok(video_elements) = document.select("video[poster]") {
    for video in video_elements {
      if let Some(poster) = video.attributes.borrow().get("poster") {
        if let Ok(resolved) = resolve_image_url(poster) {
          images.insert(resolved);
        }
      }
    }
  }

  // <... style="background: url(...)"> or <... style="background-image: url(...)">
  if let Ok(elements) = document.select("[style*=\"background\"]") {
    for element in elements {
      if let Some(style) = element.attributes.borrow().get("style") {
        for cap in URL_REGEX.captures_iter(style) {
          if let Some(url_match) = cap.get(1) {
            let url = url_match.as_str().trim();
            if !url.is_empty() {
              if let Ok(resolved) = resolve_image_url(url) {
                images.insert(resolved);
              }
            }
          }
        }
      }
    }
  }

  // background-image: url(...) or background: url(...) inside <style> blocks
  if let Ok(style_elements) = document.select("style") {
    for style in style_elements {
      let text = style.as_node().text_contents();
      for cap in BACKGROUND_IMAGE_URL_REGEX.captures_iter(&text) {
        if let Some(url_match) = cap.get(1) {
          let url = url_match.as_str().trim();
          if !url.is_empty() {
            if let Ok(resolved) = resolve_image_url(url) {
              images.insert(resolved);
            }
          }
        }
      }
    }
  }

  let filtered_images: Vec<String> = images
    .into_iter()
    .filter(|url| !url.to_lowercase().starts_with("javascript:"))
    .filter(|url| !url.is_empty())
    .filter(|url| url.starts_with("data:") || url.starts_with("blob:") || Url::parse(url).is_ok())
    .collect();

  Ok(filtered_images)
}

/// Extract all image URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_images(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_images(&html, &base_url))
    .await
    .map_err(|e| format!("extract_images join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all image URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_images(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_images(&html, &base_url).map_err(to_err)
}

fn resolve_url(
  src: &str,
  base_url: &Url,
  base_href_url: &Url,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
  if src.starts_with("data:") || src.starts_with("blob:") {
    return Ok(src.to_string());
  }
  if src.starts_with("http://") || src.starts_with("https://") {
    return Ok(src.to_string());
  }
  if src.starts_with("//") {
    let resolved = base_url.join(src)?;
    return Ok(resolved.to_string());
  }
  let resolved = base_href_url.join(src)?;
  Ok(resolved.to_string())
}

fn _extract_css(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <link rel="stylesheet" href="...">
  if let Ok(elements) = document.select("link[rel=\"stylesheet\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <link type="text/css" href="...">
  if let Ok(elements) = document.select("link[type=\"text/css\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // @import inside <style> tags
  let import_regex = LazyLock::new(|| {
    Regex::new(r#"@import\s+(?:url\s*\(\s*['"]?([^'")]+)['"]?\s*\)|['"]([^'"]+)['"])"#)
      .expect("CSS import regex")
  });

  if let Ok(style_elements) = document.select("style") {
    for style in style_elements {
      let text = style.as_node().text_contents();
      for cap in import_regex.captures_iter(&text) {
        let url_str = cap.get(1).or_else(|| cap.get(2));
        if let Some(url_match) = url_str {
          let u = url_match.as_str().trim();
          if !u.is_empty() {
            if let Ok(resolved) = resolve_url(u, &base_url, &base_href_url) {
              urls.insert(resolved);
            }
          }
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all CSS stylesheet URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_css(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_css(&html, &base_url))
    .await
    .map_err(|e| format!("extract_css join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all CSS stylesheet URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_css(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_css(&html, &base_url).map_err(to_err)
}

fn _extract_js(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <script src="...">
  if let Ok(elements) = document.select("script[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <link rel="preload" as="script" href="..."> or <link rel="modulepreload" href="...">
  if let Ok(elements) = document.select("link[rel=\"preload\"][as=\"script\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  if let Ok(elements) = document.select("link[rel=\"modulepreload\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all JavaScript URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_js(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_js(&html, &base_url))
    .await
    .map_err(|e| format!("extract_js join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all JavaScript URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_js(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_js(&html, &base_url).map_err(to_err)
}

static FONT_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"url\s*\(\s*['"]?([^'")]+\.(?:woff2?|ttf|otf|eot)(?:\?[^'")]*)?)\s*['"]?\s*\)"#)
    .expect("FONT_URL_REGEX is a valid static regex pattern")
});

fn _extract_fonts(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <link> with font preloads: <link rel="preload" as="font" href="...">
  if let Ok(elements) = document.select("link[rel=\"preload\"][as=\"font\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <link rel="prefetch" as="font" href="...">
  if let Ok(elements) = document.select("link[rel=\"prefetch\"][as=\"font\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // @font-face { src: url(...) } inside <style> tags
  if let Ok(style_elements) = document.select("style") {
    for style in style_elements {
      let text = style.as_node().text_contents();
      for cap in FONT_URL_REGEX.captures_iter(&text) {
        if let Some(url_match) = cap.get(1) {
          let u = url_match.as_str().trim();
          if !u.is_empty() {
            if let Ok(resolved) = resolve_url(u, &base_url, &base_href_url) {
              urls.insert(resolved);
            }
          }
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all font URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_fonts(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_fonts(&html, &base_url))
    .await
    .map_err(|e| format!("extract_fonts join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all font URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_fonts(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_fonts(&html, &base_url).map_err(to_err)
}

fn _extract_videos(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <video src="...">
  if let Ok(elements) = document.select("video[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <video><source src="...">
  if let Ok(elements) = document.select("video source[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <video data-src="..."> (lazy-loaded)
  if let Ok(elements) = document.select("video[data-src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("data-src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <link rel="preload" as="video" href="...">
  if let Ok(elements) = document.select("link[rel=\"preload\"][as=\"video\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // OG video meta tags
  let video_meta_selectors = [
    "meta[property=\"og:video\"]",
    "meta[property=\"og:video:url\"]",
    "meta[property=\"og:video:secure_url\"]",
  ];

  for selector in &video_meta_selectors {
    if let Ok(elements) = document.select(selector) {
      for element in elements {
        if let Some(content) = element.attributes.borrow().get("content") {
          if let Ok(resolved) = resolve_url(content, &base_url, &base_href_url) {
            urls.insert(resolved);
          }
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all video URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_videos(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_videos(&html, &base_url))
    .await
    .map_err(|e| format!("extract_videos join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all video URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_videos(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_videos(&html, &base_url).map_err(to_err)
}

fn _extract_audio(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <audio src="...">
  if let Ok(elements) = document.select("audio[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <audio><source src="...">
  if let Ok(elements) = document.select("audio source[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <audio data-src="..."> (lazy-loaded)
  if let Ok(elements) = document.select("audio[data-src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("data-src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <link rel="preload" as="audio" href="...">
  if let Ok(elements) = document.select("link[rel=\"preload\"][as=\"audio\"]") {
    for element in elements {
      if let Some(href) = element.attributes.borrow().get("href") {
        if let Ok(resolved) = resolve_url(href, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // OG audio meta tags
  if let Ok(elements) = document.select("meta[property=\"og:audio\"]") {
    for element in elements {
      if let Some(content) = element.attributes.borrow().get("content") {
        if let Ok(resolved) = resolve_url(content, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all audio URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_audio(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_audio(&html, &base_url))
    .await
    .map_err(|e| format!("extract_audio join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all audio URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_audio(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_audio(&html, &base_url).map_err(to_err)
}

fn _extract_iframes(
  html: &str,
  base_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
  let document = parse_html().one(html);
  let base_url = Url::parse(base_url)?;
  let base_href = _extract_base_href_from_document(&document, &base_url)?;
  let base_href_url = Url::parse(&base_href)?;
  let mut urls = HashSet::<String>::new();

  // <iframe src="...">
  if let Ok(elements) = document.select("iframe[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <iframe data-src="..."> (lazy-loaded)
  if let Ok(elements) = document.select("iframe[data-src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("data-src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <embed src="...">
  if let Ok(elements) = document.select("embed[src]") {
    for element in elements {
      if let Some(src) = element.attributes.borrow().get("src") {
        if let Ok(resolved) = resolve_url(src, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  // <object data="...">
  if let Ok(elements) = document.select("object[data]") {
    for element in elements {
      if let Some(data) = element.attributes.borrow().get("data") {
        if let Ok(resolved) = resolve_url(data, &base_url, &base_href_url) {
          urls.insert(resolved);
        }
      }
    }
  }

  let filtered: Vec<String> = urls
    .into_iter()
    .filter(|u| !u.is_empty())
    .filter(|u| !u.to_lowercase().starts_with("javascript:"))
    .filter(|u| u.starts_with("data:") || u.starts_with("blob:") || Url::parse(u).is_ok())
    .collect();

  Ok(filtered)
}

/// Extract all iframe/embed/object URLs from HTML document.
#[cfg(feature = "native")]
pub async fn extract_iframes(html: String, base_url: String) -> Result<Vec<String>, String> {
  let res = task::spawn_blocking(move || _extract_iframes(&html, &base_url))
    .await
    .map_err(|e| format!("extract_iframes join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract all iframe/embed/object URLs from HTML document (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_iframes(html: String, base_url: String) -> Result<Vec<String>, String> {
  _extract_iframes(&html, &base_url).map_err(to_err)
}

/// All discovered asset URLs from an HTML document, categorized by type.
#[derive(Serialize, Deserialize, Default)]
pub struct ExtractedAssets {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub images: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub css: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub js: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fonts: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub videos: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub audio: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub iframes: Option<Vec<String>>,
}

fn _extract_assets(
  html: &str,
  base_url: &str,
  formats: &[String],
) -> Result<ExtractedAssets, Box<dyn std::error::Error + Send + Sync>> {
  let format_set: HashSet<&str> = formats.iter().map(|s| s.as_str()).collect();
  let all = format_set.contains("assets");
  let mut result = ExtractedAssets::default();

  if all || format_set.contains("images") {
    result.images = Some(_extract_images(html, base_url)?);
  }
  if all || format_set.contains("css") {
    result.css = Some(_extract_css(html, base_url)?);
  }
  if all || format_set.contains("js") {
    result.js = Some(_extract_js(html, base_url)?);
  }
  if all || format_set.contains("fonts") {
    result.fonts = Some(_extract_fonts(html, base_url)?);
  }
  if all || format_set.contains("videos") {
    result.videos = Some(_extract_videos(html, base_url)?);
  }
  if all || format_set.contains("audio") {
    result.audio = Some(_extract_audio(html, base_url)?);
  }
  if all || format_set.contains("iframes") {
    result.iframes = Some(_extract_iframes(html, base_url)?);
  }

  Ok(result)
}

/// Extract assets from HTML based on requested formats.
/// Pass `["assets"]` to get all asset types, or specific types like `["css", "js", "fonts"]`.
#[cfg(feature = "native")]
pub async fn extract_assets(
  html: String,
  base_url: String,
  formats: Vec<String>,
) -> Result<ExtractedAssets, String> {
  let res = task::spawn_blocking(move || _extract_assets(&html, &base_url, &formats))
    .await
    .map_err(|e| format!("extract_assets join error: {e}"))?;

  res.map_err(to_err)
}

/// Extract assets from HTML based on requested formats (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn extract_assets(
  html: String,
  base_url: String,
  formats: Vec<String>,
) -> Result<ExtractedAssets, String> {
  _extract_assets(&html, &base_url, &formats).map_err(to_err)
}

fn _post_process_markdown(markdown: &str) -> String {
  let mut link_open_count = 0usize;
  let mut out = String::with_capacity(markdown.len());

  for ch in markdown.chars() {
    match ch {
      '[' => {
        link_open_count += 1;
      }
      ']' => {
        link_open_count = link_open_count.saturating_sub(1);
      }
      _ => {}
    }

    let inside_link_content = link_open_count > 0;
    if inside_link_content && ch == '\n' {
      out.push('\\');
      out.push('\n');
    } else {
      out.push(ch);
    }
  }

  remove_skip_to_content_links(&out)
}

/// Process multi-line links in markdown.
#[cfg(feature = "native")]
pub async fn post_process_markdown(markdown: String) -> Result<String, String> {
  let res = task::spawn_blocking(move || _post_process_markdown(&markdown))
    .await
    .map_err(|e| format!("post_process_markdown join error: {e}"))?;

  Ok(res)
}

/// Process multi-line links in markdown (sync version for WASM).
#[cfg(not(feature = "native"))]
pub fn post_process_markdown(markdown: String) -> Result<String, String> {
  Ok(_post_process_markdown(&markdown))
}

fn remove_skip_to_content_links(input: &str) -> String {
  const LABEL: &str = "Skip to Content";
  let bytes = input.as_bytes();
  let len = bytes.len();
  let mut out = String::with_capacity(len);
  let mut i = 0;

  'outer: while i < len {
    if bytes[i] == b'[' {
      let label_start = i + 1;
      let label_end = label_start + LABEL.len();

      if label_end <= len && bytes[label_start..label_end].iter().all(|b| b.is_ascii()) {
        let label_slice = &input[label_start..label_end];

        if label_slice.eq_ignore_ascii_case(LABEL)
          && label_end + 3 <= len
          && bytes[label_end] == b']'
          && bytes[label_end + 1] == b'('
          && bytes[label_end + 2] == b'#'
        {
          let mut j = label_end + 3;

          while j < len {
            let ch = input[j..].chars().next().unwrap();
            if ch == ')' {
              i = j + ch.len_utf8();
              continue 'outer;
            }
            j += ch.len_utf8();
          }
        }
      }
    }

    let ch = input[i..].chars().next().unwrap();
    out.push(ch);
    i += ch.len_utf8();
  }

  out
}
