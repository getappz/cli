//! Post-conversion repairs for what mdream leaks or leaves out: empty data-URI
//! images, heading/sup tags inside link text, and a front matter block in
//! Cloudflare's layout (`title`, `description`, `image`) with JSON-LD appended.

use mdream::{HtmlToMarkdownOptions, html_to_markdown};

/// Repairs on mdream's raw Markdown (front matter untouched).
pub fn apply(md: String) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    for line in md.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
        } else if in_fence {
            out.push_str(line);
        } else {
            let line = strip_data_images(line);
            let line = strip_link_text_leaks(&line);
            out.push_str(&drop_empty_links(&line));
        }
    }
    out
}

/// `![alt](data:...)` and `![alt]()` (what mdream emits for a data URI) are
/// base64 noise for an agent: remove the whole image.
fn strip_data_images(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("![") {
        let after = &rest[start + 2..];
        let Some(close) = after.find("](") else { break };
        let url_start = start + 2 + close + 2;
        let url_part = &rest[url_start..];
        let Some(end) = matching_paren(url_part) else { break };
        let url = &url_part[..end];
        if url.is_empty() || url.starts_with("data:") {
            out.push_str(&rest[..start]);
        } else {
            out.push_str(&rest[..url_start + end + 1]);
        }
        rest = &rest[url_start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Byte index of the `)` closing an already-open `(`, honouring nesting.
fn matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// `[](url)` left behind once its only content was a dropped image.
fn drop_empty_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("[](") {
        let Some(end) = matching_paren(&rest[i + 3..]) else { break };
        out.push_str(&rest[..i]);
        rest = &rest[i + 3 + end + 1..];
    }
    out.push_str(rest);
    out
}

const LEAK_TAGS: [&str; 8] = [
    "<h1>", "</h1>", "<sup>", "</sup>", "<h2>", "</h2>", "<h3>", "</h3>",
];

/// Remove literal heading / `<sup>` tags (and a leading `# `) from `[text](...)`.
fn strip_link_text_leaks(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find("](") else { break };
        out.push_str(&rest[..open + 1]);
        let mut text = after[..close].to_owned();
        for tag in LEAK_TAGS {
            text = text.replace(tag, "");
        }
        out.push_str(text.trim_start_matches('#').trim_start());
        rest = &after[close..];
    }
    out.push_str(rest);
    out
}

/// The Markdown without its leading `---` front matter block, if any.
pub fn strip_front_matter(md: &str) -> &str {
    match md
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
    {
        Some((_, body)) => body,
        None => md,
    }
}

/// Swap mdream's front matter for `title` / `description` / `image` built from
/// the page's meta tags (`name=` beats `og:`), then append JSON-LD blocks.
pub fn finish(md: String, html: &str, origin: &str) -> String {
    let title = front_matter_title(&md).map(str::to_owned);
    let body = strip_front_matter(&md).trim_start_matches('\n');
    let metas = meta_tags(html);
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| metas.iter().find(|(n, _)| n == k).map(|(_, v)| v.trim()))
            .filter(|v| !v.is_empty())
    };

    let mut fm = String::new();
    if let Some(t) = &title {
        fm.push_str(&format!("title: {t}\n"));
    }
    if let Some(d) = pick(&["description", "og:description"]) {
        fm.push_str(&format!("description: {}\n", yaml_quote(d)));
    }
    if let Some(i) = pick(&["image", "twitter:image", "og:image"]) {
        let abs = resolve(i, origin);
        fm.push_str(&format!("image: {}\n", yaml_quote(&abs)));
    }

    let mut out = String::with_capacity(md.len() + fm.len() + 16);
    if !fm.is_empty() {
        out.push_str("---\n");
        out.push_str(&fm);
        out.push_str("---\n\n");
    }
    out.push_str(body.trim_end());
    out.push('\n');
    for block in json_ld_blocks(html) {
        out.push_str("\n```json\n");
        out.push_str(&block);
        out.push_str("\n```\n");
    }
    out
}

/// Resolve `href` against `origin` with mdream's own resolver, so image URLs
/// match the links in the body (and the wasm build needs no URL crate).
fn resolve(href: &str, origin: &str) -> String {
    let probe = format!("<a href=\"{}\">x</a>", href.replace('"', "&quot;"));
    let md = html_to_markdown(&probe, HtmlToMarkdownOptions::default().with_origin(origin));
    md.strip_prefix("[x](")
        .and_then(|r| r.trim_end().strip_suffix(')'))
        .map_or_else(|| href.to_owned(), str::to_owned)
}

fn front_matter_title(md: &str) -> Option<&str> {
    let head = md.strip_prefix("---\n")?.split_once("\n---\n")?.0;
    head.lines().find_map(|l| l.strip_prefix("title: "))
}

fn yaml_quote(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{}\"", flat.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `(name-or-property lowercased, content)` of every `<meta>` before `</head>`.
fn meta_tags(html: &str) -> Vec<(String, String)> {
    let lower = html.to_ascii_lowercase();
    let end = lower.find("</head").unwrap_or(lower.len());
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..end].find("<meta") {
        let start = pos + i + 5;
        let Some(len) = lower[start..end].find('>') else { break };
        let attrs = parse_attrs(&html[start..start + len]);
        pos = start + len;
        let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        if let (Some(key), Some(content)) = (get("name").or_else(|| get("property")), get("content"))
        {
            out.push((key.to_ascii_lowercase(), decode_entities(&content)));
        }
    }
    out
}

/// Attributes of a tag body: `name="v" other='v' bare=v`.
fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let ns = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        let name = s[ns..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let value = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                let v = &s[vs..i];
                i = (i + 1).min(b.len());
                v
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                &s[vs..i]
            };
            if !name.is_empty() {
                out.push((name, value.to_owned()));
            }
        } else if !name.is_empty() {
            out.push((name, String::new()));
        }
    }
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Contents of every `<script type="application/ld+json">`.
fn json_ld_blocks(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<script") {
        let tag_start = pos + i + 7;
        let Some(gt) = lower[tag_start..].find('>') else { break };
        let content_start = tag_start + gt + 1;
        let Some(end) = lower[content_start..].find("</script") else { break };
        pos = content_start + end;
        if lower[tag_start..tag_start + gt].contains("ld+json") {
            let body = html[content_start..content_start + end].trim();
            if body.starts_with('{') || body.starts_with('[') {
                out.push(body.to_owned());
            }
        }
    }
    out
}
