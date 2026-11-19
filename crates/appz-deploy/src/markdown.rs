//! "Markdown for Agents" at deploy time: every scanned `.html` gets a sibling
//! `.md` that is uploaded as an ordinary blob, so the serve worker can answer
//! `Accept: text/markdown` straight from storage. The HTML → Markdown
//! conversion itself is hosted: [`HttpMarkdownConverter`] calls the API.
//! Conversion problems only ever cost a page its sibling; they never fail a
//! deploy.

use crate::static_cas::{LocalBlob, ScannedFileEntry, ScannedTree, blob_id};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tempfile::TempDir;

const MARKDOWN_MIME: &str = "text/markdown; charset=utf-8";

/// Path of the hosted conversion endpoint, relative to the API base.
pub const MARKDOWN_CONVERT_PATH: &str = "/api/v0/markdown/convert";
/// Optional request header carrying the absolute URL the page is served at.
pub const PAGE_ORIGIN_HEADER: &str = "X-Page-Origin";
/// Pages larger than this are not sent for conversion.
pub const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
/// Parallel conversion requests (each one costs money, so keep it low).
const CONCURRENCY: usize = 4;

/// Why a page got no Markdown sibling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertError {
    /// HTTP status of the hosted converter, when there was one.
    pub status: Option<u16>,
    pub message: String,
}

impl ConvertError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            status: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Turns one page into Markdown. `page_url` is where the page is served, when
/// known.
pub trait MarkdownConverter: Sync {
    fn convert(&self, html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError>;
}

impl<F> MarkdownConverter for F
where
    F: Fn(&[u8], Option<&str>) -> Result<String, ConvertError> + Sync,
{
    fn convert(&self, html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError> {
        self(html, page_url)
    }
}

/// Converter backed by `POST {api_base}/api/v0/markdown/convert`: raw HTML in
/// the body, `{markdown, tokens, mode}` JSON back. Every non-200 is an error;
/// 4xx is never retried, a 5xx is retried once.
pub struct HttpMarkdownConverter {
    endpoint: String,
    auth_header: Option<String>,
    agent: ureq::Agent,
}

impl HttpMarkdownConverter {
    pub fn new(api_base: &str, token: Option<&str>) -> Self {
        Self {
            endpoint: format!("{}{MARKDOWN_CONVERT_PATH}", api_base.trim_end_matches('/')),
            auth_header: token.map(|t| format!("Bearer {t}")),
            agent: ureq::builder()
                .timeout_read(Duration::from_secs(30))
                .timeout_write(Duration::from_secs(30))
                .max_idle_connections_per_host(CONCURRENCY)
                .build(),
        }
    }

    #[allow(clippy::result_large_err)] // ureq::Error is large; this stays private
    fn post(&self, html: &[u8], page_url: Option<&str>) -> Result<ureq::Response, ureq::Error> {
        let mut req = self
            .agent
            .post(&self.endpoint)
            .set("Content-Type", "text/html; charset=utf-8");
        if let Some(auth) = &self.auth_header {
            req = req.set("Authorization", auth);
        }
        if let Some(url) = page_url {
            req = req.set(PAGE_ORIGIN_HEADER, url);
        }
        req.send_bytes(html)
    }
}

impl MarkdownConverter for HttpMarkdownConverter {
    fn convert(&self, html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError> {
        let mut result = self.post(html, page_url);
        if matches!(result, Err(ureq::Error::Status(code, _)) if code >= 500) {
            result = self.post(html, page_url);
        }
        match result {
            Ok(resp) if resp.status() == 200 => {
                let body: serde_json::Value = resp.into_json().map_err(|e| {
                    ConvertError::new(format!("unreadable converter response: {e}"))
                })?;
                match body.get("markdown").and_then(|m| m.as_str()) {
                    Some(md) if !md.trim().is_empty() => Ok(md.to_owned()),
                    _ => Err(ConvertError::new("converter returned no content")),
                }
            }
            Ok(resp) => Err(status_error(resp.status(), String::new())),
            Err(ureq::Error::Status(code, resp)) => {
                let detail = resp
                    .into_json::<serde_json::Value>()
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                    .unwrap_or_default();
                Err(status_error(code, detail))
            }
            Err(e) => Err(ConvertError::new(format!("converter unreachable: {e}"))),
        }
    }
}

fn status_error(code: u16, detail: String) -> ConvertError {
    let message = match code {
        401 => "not signed in to the markdown converter (HTTP 401)".to_owned(),
        403 => {
            "the markdown feature needs to be enabled for your organization (HTTP 403)".to_owned()
        }
        413 => "page too large to convert (HTTP 413)".to_owned(),
        429 => "markdown converter rate limit reached (HTTP 429)".to_owned(),
        _ if detail.is_empty() => format!("markdown converter returned HTTP {code}"),
        _ => format!("markdown converter returned HTTP {code}: {detail}"),
    };
    ConvertError {
        status: Some(code),
        message,
    }
}

/// Owns the generated `.md` files; keep it alive until uploads finish, since
/// the [`LocalBlob`] paths in the tree point into it.
#[derive(Debug)]
pub struct SiblingGuard {
    _dir: TempDir,
    /// How many siblings were generated.
    pub generated: usize,
    /// Pages that got no sibling because conversion failed.
    pub skipped: usize,
    /// The first conversion error, in page order.
    pub first_error: Option<String>,
}

impl SiblingGuard {
    /// One deploy-level warning covering every skipped page, if any.
    pub fn warning(&self) -> Option<String> {
        let first = self.first_error.as_deref()?;
        let pages = if self.skipped == 1 { "page" } else { "pages" };
        Some(format!(
            "Markdown for Agents: no .md sibling for {} {pages}; first error: {first}",
            self.skipped
        ))
    }
}

/// `/about/index.html` → `/about/index.md`, `/about.html` → `/about.md`;
/// anything that is not `.html` has no sibling. Mirrors `markdownSiblingPath`
/// on the serve side — change both together.
pub fn markdown_sibling_path(html_path: &str) -> Option<String> {
    html_path
        .strip_suffix(".html")
        .map(|stem| format!("{stem}.md"))
}

/// The URL a page is served at (`/about/index.html` → `{origin}/about/`).
fn page_url(origin: &str, html_path: &str) -> Option<String> {
    let served = match html_path.strip_suffix("/index.html") {
        Some(dir) => format!("{dir}/"),
        None => html_path.strip_suffix(".html")?.to_owned(),
    };
    Some(format!("{}{served}", origin.trim_end_matches('/')))
}

type Outcome = Result<String, ConvertError>;

/// Convert each `.html` in `tree` and add the result as a `.md` entry. Pages
/// that fail (unreadable, over [`MAX_HTML_BYTES`], converter error) are
/// skipped and counted in the guard, see [`SiblingGuard::warning`]; an
/// existing file at the sibling path is never replaced. `origin` is the
/// site's base URL, if known. Only a local scratch-directory failure is an
/// `Err`.
pub fn add_markdown_siblings(
    output_dir: &Path,
    tree: &mut ScannedTree,
    origin: Option<&str>,
    converter: &dyn MarkdownConverter,
) -> Result<SiblingGuard, String> {
    let dir = TempDir::new().map_err(|e| format!("cannot create markdown scratch dir: {e}"))?;
    let existing: HashSet<String> = tree.files.iter().map(|f| f.path.clone()).collect();

    let mut pages: Vec<(String, String)> = tree
        .files
        .iter()
        .filter_map(|f| {
            let sibling = markdown_sibling_path(&f.path)?;
            (!existing.contains(&sibling)).then(|| (f.path.clone(), sibling))
        })
        .collect();
    pages.sort();

    // Bounded concurrency; results keep page order so the output (and the
    // deploy payload) is deterministic. Identical HTML is converted once: the
    // first thread to see a content hash does the work, the rest wait for and
    // reuse its result.
    let results: Vec<Mutex<Option<Outcome>>> = pages.iter().map(|_| Mutex::new(None)).collect();
    let by_content: Mutex<HashMap<String, Arc<OnceLock<Outcome>>>> = Mutex::new(HashMap::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..CONCURRENCY.min(pages.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((html_path, _)) = pages.get(i) else {
                        break;
                    };
                    let outcome =
                        match std::fs::read(output_dir.join(html_path.trim_start_matches('/'))) {
                            Err(e) => {
                                Err(ConvertError::new(format!("cannot read {html_path}: {e}")))
                            }
                            Ok(html) if html.len() > MAX_HTML_BYTES => Err(ConvertError::new(
                                format!("{html_path} is over the {MAX_HTML_BYTES} byte limit"),
                            )),
                            Ok(html) => {
                                let cell = by_content
                                    .lock()
                                    .unwrap()
                                    .entry(blob_id(&html))
                                    .or_default()
                                    .clone();
                                let url = origin.and_then(|o| page_url(o, html_path));
                                cell.get_or_init(|| converter.convert(&html, url.as_deref()))
                                    .clone()
                            }
                        };
                    *results[i].lock().unwrap() = Some(outcome);
                }
            });
        }
    });

    let mut generated = 0;
    let mut skipped = 0;
    let mut first_error = None;
    for ((_, md_path), result) in pages.into_iter().zip(results) {
        let markdown = match result.into_inner().unwrap() {
            Some(Ok(markdown)) => markdown,
            Some(Err(e)) => {
                skipped += 1;
                first_error.get_or_insert(e.message);
                continue;
            }
            None => continue,
        };
        let bytes = markdown.into_bytes();
        let hash = blob_id(&bytes);
        let blob_path = dir.path().join(hash.trim_start_matches("b3:"));
        if !blob_path.exists() {
            std::fs::write(&blob_path, &bytes)
                .map_err(|e| format!("cannot write {}: {e}", blob_path.display()))?;
        }
        tree.total_bytes += bytes.len();
        tree.blobs_by_hash
            .entry(hash.clone())
            .or_insert_with(|| LocalBlob {
                hash: hash.clone(),
                path: blob_path,
            });
        tree.files.push(ScannedFileEntry {
            path: md_path,
            hash,
            size: bytes.len(),
            r#type: MARKDOWN_MIME.to_owned(),
            immutable: None,
        });
        generated += 1;
    }
    Ok(SiblingGuard {
        _dir: dir,
        generated,
        skipped,
        first_error,
    })
}
