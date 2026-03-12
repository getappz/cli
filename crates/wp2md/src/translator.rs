//! HTML-to-Markdown conversion with WordPress-specific custom rules.
//!
//! Ports the turndown.js custom rules from the reference `translator.js`.
//! Uses `htmd::convert()` for the core conversion, with pre-processing
//! and post-processing to handle WordPress-specific patterns:
//! - Remove `<style>` elements
//! - Preserve embedded tweets, CodePen embeds, scripts, iframes
//! - Convert `<pre>` to fenced code blocks with language detection
//! - Paragraph separation, image URL rewriting, etc.

use crate::types::Post;
use regex::Regex;

/// Translate all posts' HTML content to Markdown.
pub fn translate_posts(posts: &mut [Post]) {
    for post in posts.iter_mut() {
        post.content_md = get_post_content(&post.content_html);
    }
}

/// Convert a single post's HTML content to Markdown.
fn get_post_content(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }

    let mut content = html.to_string();

    // ── Pre-processing ─────────────────────────────────────────────

    // Remove <style> elements and their contents
    let style_re = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    content = style_re.replace_all(&content, "").to_string();

    // Protect embedded tweets — wrap in a placeholder
    let tweet_re = Regex::new(
        r#"(?is)(<blockquote[^>]*class=["'][^"']*twitter-tweet[^"']*["'][^>]*>.*?</blockquote>)"#,
    )
    .unwrap();
    let mut tweet_blocks: Vec<String> = Vec::new();
    content = tweet_re
        .replace_all(&content, |caps: &regex::Captures| {
            let idx = tweet_blocks.len();
            tweet_blocks.push(caps[1].to_string());
            format!("<!--HTMD_TWEET_{}-->", idx)
        })
        .to_string();

    // Protect <script> tags
    let script_re = Regex::new(r"(?is)(<script[^>]*>.*?</script>)").unwrap();
    let mut script_blocks: Vec<String> = Vec::new();
    content = script_re
        .replace_all(&content, |caps: &regex::Captures| {
            let idx = script_blocks.len();
            script_blocks.push(caps[1].to_string());
            format!("<!--HTMD_SCRIPT_{}-->", idx)
        })
        .to_string();

    // Protect <iframe> elements
    let iframe_re = Regex::new(r"(?is)(<iframe[^>]*(?:/>|>.*?</iframe>))").unwrap();
    let mut iframe_blocks: Vec<String> = Vec::new();
    content = iframe_re
        .replace_all(&content, |caps: &regex::Captures| {
            let idx = iframe_blocks.len();
            iframe_blocks.push(caps[1].to_string());
            format!("<!--HTMD_IFRAME_{}-->", idx)
        })
        .to_string();

    // Protect <figure> elements that contain <figcaption>
    // Note: Rust regex doesn't support look-ahead; use non-greedy .*? instead
    let figure_re =
        Regex::new(r"(?is)(<figure[^>]*>.*?<figcaption.*?</figure>)").unwrap();
    let mut figure_blocks: Vec<String> = Vec::new();
    content = figure_re
        .replace_all(&content, |caps: &regex::Captures| {
            let idx = figure_blocks.len();
            figure_blocks.push(caps[1].to_string());
            format!("<!--HTMD_FIGURE_{}-->", idx)
        })
        .to_string();

    // Extract code language from WordPress block editor comments
    let wp_code_re =
        Regex::new(r#"<!--\s*wp:code\s*\{"language":"([^"]+)"\}\s*-->\s*<pre"#).unwrap();
    content = wp_code_re
        .replace_all(&content, r#"<pre data-lang="$1""#)
        .to_string();

    // Convert <pre> blocks with language detection to fenced code blocks
    let pre_re = Regex::new(
        r#"(?is)<pre[^>]*(?:class=["']([^"']*)["'])?[^>]*(?:data-lang=["']([^"']*)["'])?[^>]*>(.*?)</pre>"#,
    )
    .unwrap();
    let mut code_blocks: Vec<String> = Vec::new();
    content = pre_re
        .replace_all(&content, |caps: &regex::Captures| {
            let inner = &caps[3];

            // Skip if it contains <code> — htmd will handle these
            if inner.contains("<code") {
                return caps[0].to_string();
            }

            // Detect language from data-lang or class
            let lang = caps
                .get(2)
                .and_then(|m| {
                    let l = m.as_str().trim();
                    if l.is_empty() {
                        None
                    } else {
                        Some(l.to_string())
                    }
                })
                .or_else(|| {
                    caps.get(1)
                        .and_then(|m| detect_language_from_class(m.as_str()))
                })
                .unwrap_or_default();

            let text = strip_html_tags(inner);
            let idx = code_blocks.len();
            code_blocks.push(format!("```{}\n{}\n```", lang, text.trim()));
            format!("<!--HTMD_CODE_{}-->", idx)
        })
        .to_string();

    // Insert empty div between double line breaks to preserve paragraph separation
    content = content.replace("\n\n", "\n<div></div>\n");

    // Rewrite absolute image URLs to relative paths
    content = rewrite_image_urls(&content);

    // Preserve "more" separator
    let more_re = Regex::new(r"<!--\s*more(\s+[^>]*)?\s*-->").unwrap();
    content = more_re
        .replace_all(&content, "HTMD_MORE_SEPARATOR$1")
        .to_string();

    // ── Core conversion ────────────────────────────────────────────

    let mut md = htmd::convert(&content).unwrap_or(content);

    // ── Post-processing ────────────────────────────────────────────

    // Restore "more" separators
    let more_restore_re = Regex::new(r"HTMD_MORE_SEPARATOR(.*)").unwrap();
    md = more_restore_re
        .replace_all(&md, "<!--more$1-->")
        .to_string();

    // Restore protected tweet blocks
    for (i, block) in tweet_blocks.iter().enumerate() {
        md = md.replace(
            &format!("<!--HTMD_TWEET_{}-->", i),
            &format!("\n\n{}\n\n", block),
        );
    }

    // Restore protected script blocks
    for (i, block) in script_blocks.iter().enumerate() {
        md = md.replace(
            &format!("<!--HTMD_SCRIPT_{}-->", i),
            &format!("\n\n{}\n\n", block),
        );
    }

    // Restore protected iframe blocks
    for (i, block) in iframe_blocks.iter().enumerate() {
        md = md.replace(
            &format!("<!--HTMD_IFRAME_{}-->", i),
            &format!("\n\n{}\n\n", block),
        );
    }

    // Restore protected figure blocks
    for (i, block) in figure_blocks.iter().enumerate() {
        md = md.replace(
            &format!("<!--HTMD_FIGURE_{}-->", i),
            &format!("\n\n{}\n\n", block),
        );
    }

    // Restore code blocks
    for (i, block) in code_blocks.iter().enumerate() {
        md = md.replace(
            &format!("<!--HTMD_CODE_{}-->", i),
            &format!("\n\n{}\n\n", block),
        );
    }

    // Clean up extra spaces in list items
    let list_space_re = Regex::new(r"(?m)^(\s*[-*+])\s{2,}").unwrap();
    md = list_space_re.replace_all(&md, "$1 ").to_string();

    // Collapse excessive newlines (3+ -> 2)
    let newline_re = Regex::new(r"\n{3,}").unwrap();
    md = newline_re.replace_all(&md, "\n\n").to_string();

    md.trim().to_string()
}

/// Rewrite absolute image URLs in `<img>` src attributes to relative `images/` paths.
fn rewrite_image_urls(html: &str) -> String {
    let img_re =
        Regex::new(r#"(<img[^>]+src=["'])https?://[^"']*?/([^/"']+)(["'])"#).unwrap();
    img_re
        .replace_all(html, "${1}images/${2}${3}")
        .to_string()
}

/// Detect a programming language from CSS class names.
fn detect_language_from_class(class: &str) -> Option<String> {
    for prefix in &["language-", "lang-", "brush:"] {
        if let Some(idx) = class.find(prefix) {
            let rest = &class[idx + prefix.len()..];
            let lang: String = rest
                .chars()
                .take_while(|c: &char| c.is_alphanumeric() || *c == '-' || *c == '+')
                .collect();
            if !lang.is_empty() {
                return Some(lang);
            }
        }
    }
    None
}

/// Strip HTML tags from a string, leaving only text content.
fn strip_html_tags(html: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    tag_re.replace_all(html, "").to_string()
}
