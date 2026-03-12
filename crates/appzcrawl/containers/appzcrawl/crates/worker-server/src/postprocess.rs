//! Post-process markdown enhancements (crawl4ai-inspired).
//! Applied after firecrawl_rs post_process_markdown.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use url::Url;

/// Fix malformed fenced code blocks (4-space indented ```).
pub fn fix_code_blocks(markdown: &str) -> String {
  markdown.replace("    ```", "```")
}

/// Resolve relative URL against base.
fn resolve_url(base: &str, rel: &str) -> String {
  if rel.starts_with("http://")
    || rel.starts_with("https://")
    || rel.starts_with("mailto:")
    || rel.starts_with("//")
  {
    return rel.to_string();
  }
  if rel.starts_with('/') {
    let base_trimmed = base.trim_end_matches('/');
    return format!("{base_trimmed}{rel}");
  }
  Url::parse(base)
    .ok()
    .and_then(|base_url| base_url.join(rel).ok())
    .map(|u| u.to_string())
    .unwrap_or_else(|| rel.to_string())
}

static LINK_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"!?\[([^\]]*)\]\(([^)]+?)(?:\s+"([^"]*)")?\)"#).expect("link regex"));

/// Convert markdown links to citations format.
pub fn convert_links_to_citations(markdown: &str, base_url: &str) -> String {
  let mut link_map: HashMap<String, (u32, String)> = HashMap::new();
  let mut url_cache: HashMap<String, String> = HashMap::new();
  let mut parts: Vec<String> = Vec::new();
  let mut last_end = 0;
  let mut counter: u32 = 1;

  for cap in LINK_RE.captures_iter(markdown) {
    let full = cap.get(0).unwrap();
    let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
    let mut url = cap.get(2).map(|m| m.as_str()).unwrap_or("");
    let title = cap.get(3).map(|m| m.as_str()).unwrap_or("");

    parts.push(markdown[last_end..full.start()].to_string());

    if !base_url.is_empty()
      && !url.starts_with("http://")
      && !url.starts_with("https://")
      && !url.starts_with("mailto:")
    {
      url = url_cache
        .entry(url.to_string())
        .or_insert_with(|| resolve_url(base_url, url))
        .as_str();
    }

    let entry = link_map.entry(url.to_string()).or_insert_with(|| {
      let mut desc = String::new();
      if !title.is_empty() {
        desc.push_str(title);
      }
      if !text.is_empty() && text != title {
        if !desc.is_empty() {
          desc.push_str(" - ");
        }
        desc.push_str(text);
      }
      let suffix = if desc.is_empty() {
        String::new()
      } else {
        format!(": {desc}")
      };
      let num = counter;
      counter += 1;
      (num, suffix)
    });

    let num = entry.0;
    let replacement = if full.as_str().starts_with('!') {
      format!("![{text}⟨{num}⟩]")
    } else {
      format!("{text}⟨{num}⟩")
    };
    parts.push(replacement);
    last_end = full.end();
  }

  parts.push(markdown[last_end..].to_string());
  let converted = parts.concat();

  if link_map.is_empty() {
    return converted;
  }

  let mut refs = vec!["\n\n## References\n\n".to_string()];
  let mut sorted: Vec<_> = link_map.into_iter().collect();
  sorted.sort_by_key(|(_, (n, _))| *n);
  for (url, (num, desc)) in sorted {
    refs.push(format!("⟨{num}⟩ {url}{desc}\n"));
  }
  converted + &refs.concat()
}
