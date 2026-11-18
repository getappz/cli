//! HTML → Markdown for "Markdown for Agents".
//!
//! One converter, two homes: the CLI calls [`convert`] natively at deploy time
//! to write `.md` siblings, and the `appz-html2md` worker runs the same code as
//! WASM (see [`wasm`]) for pages that have no sibling.
//!
//! The engine is mdream's "minimal" preset (clean everything, front matter,
//! isolate `<main>`, Tailwind, drop nav/footer/form). `isolate_main` can
//! truncate landing pages whose real content sits outside `<main>`, so a
//! filter-only variant is computed as a safety net and wins when the minimal
//! output is a small fraction of it.

mod fixes;
#[cfg(target_arch = "wasm32")]
mod wasm;

use mdream::{
    CleanConfig, FilterConfig, FrontmatterConfig, HtmlToMarkdownOptions, IsolateMainConfig,
    PluginConfig, TailwindConfig, html_to_markdown, html_to_text,
};
use url::Url;

/// Largest HTML input accepted (2 MiB).
pub const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;

/// Minimal output below this share of the page's raw-text tokens triggers the
/// filter-only comparison.
const SUSPECT_RATIO: f64 = 0.40;
/// Minimal output below this share of the filter-only tokens loses to it.
const FALLBACK_RATIO: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Minimal,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub markdown: String,
    pub tokens: usize,
    pub mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    NotUtf8,
    Empty,
    TooLarge,
}

/// Token estimate: chars / 4, rounded up.
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count().div_ceil(4)
}

/// Convert `html` (fetched from `origin`) to Markdown.
pub fn convert(html: &[u8], origin: &Url) -> Result<Converted, ConvertError> {
    if html.len() > MAX_HTML_BYTES {
        return Err(ConvertError::TooLarge);
    }
    let html = std::str::from_utf8(html).map_err(|_| ConvertError::NotUtf8)?;

    let minimal = run(html, origin, Variant::Minimal);
    if !has_body(&minimal) {
        return Err(ConvertError::Empty);
    }
    let minimal_tokens = estimate_tokens(&minimal);
    let raw_tokens = estimate_tokens(&html_to_text(html, HtmlToMarkdownOptions::default()));

    let (body, mode) = if (minimal_tokens as f64) < SUSPECT_RATIO * raw_tokens as f64 {
        let filtered = run(html, origin, Variant::FilterOnly);
        let filtered_tokens = estimate_tokens(&filtered);
        if (minimal_tokens as f64) < FALLBACK_RATIO * filtered_tokens as f64 {
            (filtered, Mode::Fallback)
        } else {
            (minimal, Mode::Minimal)
        }
    } else {
        (minimal, Mode::Minimal)
    };

    let markdown = fixes::finish(body, html, origin);
    Ok(Converted {
        tokens: estimate_tokens(&markdown),
        markdown,
        mode,
    })
}

/// The minimal preset alone, without the safety net. Test support.
#[doc(hidden)]
pub fn convert_minimal_only(html: &str, origin: &Url) -> String {
    run(html, origin, Variant::Minimal)
}

#[derive(Clone, Copy)]
enum Variant {
    Minimal,
    FilterOnly,
}

fn run(html: &str, origin: &Url, variant: Variant) -> String {
    let exclude: &[&str] = match variant {
        Variant::Minimal => &["nav", "footer", "form"],
        Variant::FilterOnly => &["nav", "footer", "form", "aside", "header"],
    };
    let options = HtmlToMarkdownOptions {
        origin: Some(origin.as_str().to_owned()),
        clean: Some(CleanConfig::all()),
        plugins: Some(PluginConfig {
            frontmatter: Some(FrontmatterConfig::default()),
            isolate_main: matches!(variant, Variant::Minimal).then_some(IsolateMainConfig),
            tailwind: Some(TailwindConfig),
            filter: Some(FilterConfig::exclude(exclude)),
            ..Default::default()
        }),
        ..Default::default()
    };
    fixes::apply(html_to_markdown(html, options))
}

/// Whether anything remains once the front matter block is removed.
fn has_body(md: &str) -> bool {
    !fixes::strip_front_matter(md).trim().is_empty()
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotUtf8 => "input is not valid UTF-8",
            Self::Empty => "conversion produced no content",
            Self::TooLarge => "input exceeds the size cap",
        })
    }
}

impl std::error::Error for ConvertError {}
