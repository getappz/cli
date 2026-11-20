//! "Markdown for Agents" at deploy time: every scanned `.html` gets a sibling
//! `.md` that is uploaded as an ordinary blob, so the serve worker can answer
//! `Accept: text/markdown` straight from storage. The HTML → Markdown
//! conversion itself is hosted: [`HttpMarkdownConverter`] calls the API.
//! Conversion problems only ever cost a page its sibling; they never fail a
//! deploy.

use crate::static_cas::{LocalBlob, ScannedFileEntry, ScannedTree, blob_id};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const MARKDOWN_MIME: &str = "text/markdown; charset=utf-8";

/// Path of the hosted single-page conversion endpoint, relative to the API base.
pub const MARKDOWN_CONVERT_PATH: &str = "/api/v0/markdown/convert";
/// Path of the hosted batch conversion endpoint, relative to the API base.
pub const MARKDOWN_CONVERT_BATCH_PATH: &str = "/api/v0/markdown/convert-batch";
/// Optional request header carrying the absolute URL the page is served at.
pub const PAGE_ORIGIN_HEADER: &str = "X-Page-Origin";
/// Pages larger than this are not sent for conversion.
pub const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
/// Maximum pages accepted per batch request.
pub const BATCH_MAX_PAGES: usize = 25;
/// Target serialized JSON body cap per batch (~3.5 MB, safely below the 4 MB server limit).
pub const BATCH_TARGET_BYTES: usize = 3_500_000;
/// Parallel conversion requests for single-page fallback / closures.
const CONCURRENCY: usize = 4;
/// Parallel batch requests in flight (server converts 5 pages concurrently per batch).
const BATCH_CONCURRENCY: usize = 2;
/// Longest single `Retry-After` we will sit out; longer asks stop the run.
const MAX_RETRY_AFTER_SECS: u64 = 120;
/// Total time one run may spend waiting out rate limits.
const MAX_TOTAL_RATE_LIMIT_WAIT_SECS: u64 = 300;
/// Serialized-entry headroom above [`BATCH_TARGET_BYTES`] before the server's
/// 4 MB body limit; a lone page past this is skipped locally instead of sent.
const BATCH_HARD_LIMIT_SLACK_BYTES: usize = 500_000;
/// Serialized size of `{"pages":[]}`.
const ENVELOPE_BYTES: usize = 12;

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

/// One page in a [`MarkdownConverter::convert_batch`] request.
#[derive(Debug, Clone, Copy)]
pub struct BatchPage<'a> {
    pub id: &'a str,
    pub html: &'a str,
    pub origin: Option<&'a str>,
}

/// Batching limits advertised by a [`MarkdownConverter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchLimits {
    pub max_pages: usize,
    pub max_body_bytes: usize,
    pub max_in_flight: usize,
}

impl BatchLimits {
    pub const fn single() -> Self {
        Self {
            max_pages: 1,
            max_body_bytes: usize::MAX,
            max_in_flight: CONCURRENCY,
        }
    }
}

type BatchResults = Vec<(String, Outcome)>;
type BatchRetryErr = (ConvertError, Option<u64>);

/// Turns one page into Markdown. `page_url` is where the page is served, when
/// known.
pub trait MarkdownConverter: Sync {
    fn convert(&self, html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError>;

    /// Convert a batch of UTF-8 HTML pages, returning `(id, outcome)` pairs.
    fn convert_batch(&self, pages: &[BatchPage<'_>]) -> Result<BatchResults, ConvertError> {
        Ok(pages
            .iter()
            .map(|p| (p.id.to_owned(), self.convert(p.html.as_bytes(), p.origin)))
            .collect())
    }

    /// Limits used by [`add_markdown_siblings`] when packing and scheduling batches.
    fn batch_limits(&self) -> BatchLimits {
        BatchLimits::single()
    }

    #[doc(hidden)]
    fn convert_batch_with_retry(
        &self,
        pages: &[BatchPage<'_>],
    ) -> Result<BatchResults, BatchRetryErr> {
        self.convert_batch(pages).map_err(|e| (e, None))
    }
}

impl<F> MarkdownConverter for F
where
    F: Fn(&[u8], Option<&str>) -> Result<String, ConvertError> + Sync,
{
    fn convert(&self, html: &[u8], page_url: Option<&str>) -> Result<String, ConvertError> {
        self(html, page_url)
    }
}

#[derive(serde::Serialize)]
struct WireBatchRequest<'a> {
    pages: &'a [WireBatchPage<'a>],
}

#[derive(serde::Serialize)]
struct WireBatchPage<'a> {
    id: &'a str,
    html: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<&'a str>,
}

#[derive(serde::Deserialize)]
struct WireBatchResponse {
    results: Vec<WireBatchResult>,
}

#[derive(serde::Deserialize)]
struct WireBatchResult {
    id: String,
    status: u16,
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

struct BatchCallError {
    err: ConvertError,
    retry_after_secs: Option<u64>,
}

/// Converter backed by `POST {api_base}/api/v0/markdown/convert-batch` (with
/// fallback to `/api/v0/markdown/convert` for older APIs).
pub struct HttpMarkdownConverter {
    endpoint: String,
    batch_endpoint: String,
    auth_header: Option<String>,
    agent: ureq::Agent,
    batch_unsupported: AtomicBool,
}

impl HttpMarkdownConverter {
    pub fn new(api_base: &str, token: Option<&str>) -> Self {
        let base = api_base.trim_end_matches('/');
        Self {
            endpoint: format!("{base}{MARKDOWN_CONVERT_PATH}"),
            batch_endpoint: format!("{base}{MARKDOWN_CONVERT_BATCH_PATH}"),
            auth_header: token.map(|t| format!("Bearer {t}")),
            agent: ureq::builder()
                .timeout_read(Duration::from_secs(30))
                .timeout_write(Duration::from_secs(30))
                .max_idle_connections_per_host(CONCURRENCY)
                .build(),
            batch_unsupported: AtomicBool::new(false),
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

    #[allow(clippy::result_large_err)]
    fn post_batch(&self, body: &[u8]) -> Result<ureq::Response, ureq::Error> {
        let mut req = self
            .agent
            .post(&self.batch_endpoint)
            .set("Content-Type", "application/json");
        if let Some(auth) = &self.auth_header {
            req = req.set("Authorization", auth);
        }
        req.send_bytes(body)
    }

    fn convert_single_fallback(
        &self,
        pages: &[BatchPage<'_>],
    ) -> Result<Vec<(String, Outcome)>, BatchCallError> {
        // Keep pages converted before a fatal status; the failing page and every
        // remaining one carry that error (the caller then stops later batches).
        let mut out = Vec::with_capacity(pages.len());
        let mut fatal: Option<ConvertError> = None;
        for page in pages {
            let res = match &fatal {
                Some(e) => Err(e.clone()),
                None => self.convert(page.html.as_bytes(), page.origin),
            };
            if fatal.is_none()
                && let Err(e) = &res
                && is_fatal_status(e.status)
            {
                fatal = Some(e.clone());
            }
            out.push((page.id.to_owned(), res));
        }
        Ok(out)
    }

    fn send_batch(
        &self,
        pages: &[BatchPage<'_>],
    ) -> Result<Vec<(String, Outcome)>, BatchCallError> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }
        if self.batch_unsupported.load(Ordering::Relaxed) {
            return self.convert_single_fallback(pages);
        }
        let wire_pages: Vec<WireBatchPage<'_>> = pages
            .iter()
            .map(|p| WireBatchPage {
                id: p.id,
                html: p.html,
                origin: p.origin,
            })
            .collect();
        let body = serde_json::to_vec(&WireBatchRequest { pages: &wire_pages }).map_err(|e| {
            BatchCallError {
                err: ConvertError::new(format!("cannot serialize markdown batch: {e}")),
                retry_after_secs: None,
            }
        })?;

        let mut result = self.post_batch(&body);
        if matches!(result, Err(ureq::Error::Status(503, _))) {
            result = self.post_batch(&body);
        }
        match result {
            Ok(resp) if resp.status() == 200 => {
                let parsed: WireBatchResponse = resp.into_json().map_err(|e| BatchCallError {
                    err: ConvertError::new(format!("unreadable converter response: {e}")),
                    retry_after_secs: None,
                })?;
                let mut by_id = HashMap::with_capacity(parsed.results.len());
                for item in parsed.results {
                    let outcome = match item.status {
                        200 => match item.markdown {
                            Some(md) if !md.trim().is_empty() => Ok(md),
                            _ => Err(ConvertError::new("converter returned no content")),
                        },
                        code => Err(status_error(code, item.error.unwrap_or_default())),
                    };
                    by_id.insert(item.id, outcome);
                }
                Ok(pages
                    .iter()
                    .map(|p| {
                        let outcome = by_id.remove(p.id).unwrap_or_else(|| {
                            Err(ConvertError::new("converter omitted page result"))
                        });
                        (p.id.to_owned(), outcome)
                    })
                    .collect())
            }
            Ok(resp) if matches!(resp.status(), 404 | 405) => {
                self.batch_unsupported.store(true, Ordering::Relaxed);
                self.convert_single_fallback(pages)
            }
            Ok(resp) => {
                let retry_after_secs = parse_retry_after(resp.header("retry-after"));
                Err(BatchCallError {
                    err: status_error(resp.status(), String::new()),
                    retry_after_secs,
                })
            }
            Err(ureq::Error::Status(404 | 405, _)) => {
                self.batch_unsupported.store(true, Ordering::Relaxed);
                self.convert_single_fallback(pages)
            }
            Err(ureq::Error::Status(code, resp)) => {
                let retry_after_secs = if code == 429 {
                    parse_retry_after(resp.header("retry-after"))
                } else {
                    None
                };
                let detail = resp
                    .into_json::<serde_json::Value>()
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                    .unwrap_or_default();
                Err(BatchCallError {
                    err: status_error(code, detail),
                    retry_after_secs,
                })
            }
            Err(e) => Err(BatchCallError {
                err: ConvertError::new(format!("converter unreachable: {e}")),
                retry_after_secs: None,
            }),
        }
    }
}

fn is_fatal_status(status: Option<u16>) -> bool {
    matches!(status, Some(401 | 403 | 429))
}

/// `Retry-After` as seconds: delta-seconds, or an HTTP-date (RFC 9110) measured
/// from now. Not clamped; the caller decides whether the wait is acceptable.
fn parse_retry_after(raw: Option<&str>) -> Option<u64> {
    let v = raw?.trim();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(secs);
    }
    let when = chrono::DateTime::parse_from_rfc2822(v).ok()?;
    let delta = when.signed_duration_since(chrono::Utc::now()).num_seconds();
    Some(delta.max(0) as u64)
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

    fn convert_batch(&self, pages: &[BatchPage<'_>]) -> Result<BatchResults, ConvertError> {
        self.send_batch(pages).map_err(|e| e.err)
    }

    fn batch_limits(&self) -> BatchLimits {
        BatchLimits {
            max_pages: BATCH_MAX_PAGES,
            max_body_bytes: BATCH_TARGET_BYTES,
            max_in_flight: BATCH_CONCURRENCY,
        }
    }

    fn convert_batch_with_retry(
        &self,
        pages: &[BatchPage<'_>],
    ) -> Result<BatchResults, BatchRetryErr> {
        self.send_batch(pages)
            .map_err(|e| (e.err, e.retry_after_secs))
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

    if pages.is_empty() {
        return stage_siblings(tree, dir, std::iter::empty());
    }

    let limits = converter.batch_limits();
    let results = if limits.max_pages <= 1 {
        convert_single_pages(output_dir, &pages, origin, converter, limits.max_in_flight)
    } else {
        convert_batched_pages(output_dir, &pages, origin, converter, limits)
    };

    let converted = pages
        .into_iter()
        .zip(results)
        .map(|((_, md_path), outcome)| (md_path, outcome));
    stage_siblings(tree, dir, converted)
}

fn convert_single_pages(
    output_dir: &Path,
    pages: &[(String, String)],
    origin: Option<&str>,
    converter: &dyn MarkdownConverter,
    concurrency: usize,
) -> Vec<Outcome> {
    // Bounded concurrency; results keep page order so the output (and the
    // deploy payload) is deterministic. Only identical converter inputs share work.
    let results: Vec<Mutex<Option<Outcome>>> = pages.iter().map(|_| Mutex::new(None)).collect();
    let by_content = Mutex::new(HashMap::<_, Arc<OnceLock<Outcome>>>::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..concurrency.max(1).min(pages.len()) {
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
                                let url = origin.and_then(|o| page_url(o, html_path));
                                let cell = by_content
                                    .lock()
                                    .unwrap()
                                    .entry((blob_id(&html), url.clone()))
                                    .or_default()
                                    .clone();
                                cell.get_or_init(|| converter.convert(&html, url.as_deref()))
                                    .clone()
                            }
                        };
                    *results[i].lock().unwrap() = Some(outcome);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|r| r.into_inner().unwrap().unwrap())
        .collect()
}

/// Metadata only: the HTML is re-read when its batch is built, so memory stays
/// bounded by the in-flight batches rather than the whole site.
struct UniqueBatchPage {
    path: String,
    origin: Option<String>,
    entry_bytes: usize,
}

fn serialized_entry_bytes(html: &str, origin: Option<&str>) -> usize {
    // Use the max 2-digit batch id ("p24") so packed size never underestimates.
    serde_json::to_string(&WireBatchPage {
        id: "p24",
        html,
        origin,
    })
    .map(|s| s.len())
    .unwrap_or(usize::MAX)
}

fn pack_batches(
    uids: &[usize],
    unique: &[UniqueBatchPage],
    limits: BatchLimits,
) -> Vec<Vec<usize>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = ENVELOPE_BYTES;
    for &uid in uids {
        let entry_bytes = unique[uid].entry_bytes;
        let sep = usize::from(!current.is_empty());
        if !current.is_empty()
            && (current.len() >= limits.max_pages
                || current_bytes
                    .saturating_add(sep)
                    .saturating_add(entry_bytes)
                    > limits.max_body_bytes)
        {
            batches.push(std::mem::take(&mut current));
            current_bytes = ENVELOPE_BYTES;
        }
        let sep = usize::from(!current.is_empty());
        current.push(uid);
        current_bytes = current_bytes
            .saturating_add(sep)
            .saturating_add(entry_bytes);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

#[derive(Default)]
struct BatchRunState {
    fatal_error: Mutex<Option<ConvertError>>,
    /// (total seconds spent waiting, end of the current rate-limit window)
    cooldown: Mutex<(u64, Option<Instant>)>,
}

impl BatchRunState {
    fn fatal(&self) -> Option<ConvertError> {
        self.fatal_error.lock().unwrap().clone()
    }

    fn set_fatal(&self, err: ConvertError) {
        let mut guard = self.fatal_error.lock().unwrap();
        if guard.is_none() {
            *guard = Some(err);
        }
    }

    fn wait_cooldown_if_any(&self) {
        let until = self.cooldown.lock().unwrap().1;
        if let Some(target) = until {
            let now = Instant::now();
            if target > now {
                std::thread::sleep(target - now);
            }
        }
    }

    fn schedule_rate_limit_wait(&self, secs: u64) -> bool {
        let target = {
            let mut guard = self.cooldown.lock().unwrap();
            let now = Instant::now();
            if let Some(existing) = guard.1
                && existing > now
            {
                existing
            } else if secs <= MAX_RETRY_AFTER_SECS
                && guard.0.saturating_add(secs) <= MAX_TOTAL_RATE_LIMIT_WAIT_SECS
            {
                // A new window: allowed while the run's total wait budget lasts.
                guard.0 += secs;
                let next = now + Duration::from_secs(secs);
                guard.1 = Some(next);
                next
            } else {
                return false;
            }
        };
        let now = Instant::now();
        if target > now {
            std::thread::sleep(target - now);
        }
        true
    }
}

fn convert_batched_pages(
    output_dir: &Path,
    pages: &[(String, String)],
    origin: Option<&str>,
    converter: &dyn MarkdownConverter,
    limits: BatchLimits,
) -> Vec<Outcome> {
    let mut local_outcomes: Vec<Option<Outcome>> = vec![None; pages.len()];
    let mut page_to_uid: Vec<Option<usize>> = vec![None; pages.len()];
    let mut unique: Vec<UniqueBatchPage> = Vec::new();
    let mut dedupe: HashMap<(String, Option<String>), usize> = HashMap::new();
    let max_single_request = limits
        .max_body_bytes
        .saturating_add(BATCH_HARD_LIMIT_SLACK_BYTES);

    for (i, (html_path, _)) in pages.iter().enumerate() {
        match std::fs::read(output_dir.join(html_path.trim_start_matches('/'))) {
            Err(e) => {
                local_outcomes[i] = Some(Err(ConvertError::new(format!(
                    "cannot read {html_path}: {e}"
                ))));
            }
            Ok(bytes) if bytes.len() > MAX_HTML_BYTES => {
                local_outcomes[i] = Some(Err(ConvertError::new(format!(
                    "{html_path} is over the {MAX_HTML_BYTES} byte limit"
                ))));
            }
            Ok(bytes) => match String::from_utf8(bytes) {
                Err(_) => {
                    local_outcomes[i] = Some(Err(ConvertError::new(format!(
                        "{html_path} is not valid UTF-8"
                    ))));
                }
                Ok(html) => {
                    let url = origin.and_then(|o| page_url(o, html_path));
                    let key = (blob_id(html.as_bytes()), url.clone());
                    if let Some(&uid) = dedupe.get(&key) {
                        page_to_uid[i] = Some(uid);
                        continue;
                    }
                    let entry_bytes = serialized_entry_bytes(&html, url.as_deref());
                    if entry_bytes.saturating_add(ENVELOPE_BYTES) > max_single_request {
                        local_outcomes[i] = Some(Err(ConvertError::new(format!(
                            "{html_path} is too large to convert once JSON-encoded"
                        ))));
                        continue;
                    }
                    let uid = unique.len();
                    unique.push(UniqueBatchPage {
                        path: html_path.clone(),
                        origin: url,
                        entry_bytes,
                    });
                    dedupe.insert(key, uid);
                    page_to_uid[i] = Some(uid);
                }
            },
        }
    }

    let unique_outcomes: Vec<Mutex<Option<Outcome>>> =
        (0..unique.len()).map(|_| Mutex::new(None)).collect();
    let state = BatchRunState::default();
    if !unique.is_empty() {
        let all_uids: Vec<usize> = (0..unique.len()).collect();
        let batches = pack_batches(&all_uids, &unique, limits);
        let retry_503 = run_batch_wave(
            output_dir,
            &batches,
            &unique,
            converter,
            limits.max_in_flight,
            &state,
            &unique_outcomes,
            true,
        );
        if !retry_503.is_empty() && state.fatal().is_none() {
            let retry_batches = pack_batches(&retry_503, &unique, limits);
            let _ = run_batch_wave(
                output_dir,
                &retry_batches,
                &unique,
                converter,
                limits.max_in_flight,
                &state,
                &unique_outcomes,
                false,
            );
        }
    }

    let final_outcomes: Vec<Option<Outcome>> = unique_outcomes
        .into_iter()
        .map(|slot| slot.into_inner().unwrap())
        .collect();
    let fallback_err = state.fatal();
    local_outcomes
        .into_iter()
        .zip(page_to_uid)
        .map(|(local, uid)| {
            local.unwrap_or_else(|| {
                final_outcomes[uid.unwrap()].clone().unwrap_or_else(|| {
                    Err(fallback_err
                        .clone()
                        .unwrap_or_else(|| ConvertError::new("converter returned no result")))
                })
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)] // one internal call site; a context struct would only add indirection
fn run_batch_wave(
    output_dir: &Path,
    batches: &[Vec<usize>],
    unique: &[UniqueBatchPage],
    converter: &dyn MarkdownConverter,
    max_in_flight: usize,
    state: &BatchRunState,
    unique_outcomes: &[Mutex<Option<Outcome>>],
    allow_503_retry: bool,
) -> Vec<usize> {
    let retry_503 = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..max_in_flight.max(1).min(batches.len()) {
            s.spawn(|| {
                loop {
                    let bi = next.fetch_add(1, Ordering::Relaxed);
                    let Some(batch) = batches.get(bi) else {
                        break;
                    };
                    if let Some(err) = state.fatal() {
                        for &uid in batch {
                            *unique_outcomes[uid].lock().unwrap() = Some(Err(err.clone()));
                        }
                        continue;
                    }
                    state.wait_cooldown_if_any();
                    if let Some(err) = state.fatal() {
                        for &uid in batch {
                            *unique_outcomes[uid].lock().unwrap() = Some(Err(err.clone()));
                        }
                        continue;
                    }

                    let mut loaded: Vec<(usize, String)> = Vec::with_capacity(batch.len());
                    for &uid in batch {
                        let path = &unique[uid].path;
                        match std::fs::read_to_string(output_dir.join(path.trim_start_matches('/')))
                        {
                            Ok(html) => loaded.push((uid, html)),
                            Err(e) => {
                                *unique_outcomes[uid].lock().unwrap() = Some(Err(
                                    ConvertError::new(format!("cannot read {path}: {e}")),
                                ));
                            }
                        }
                    }
                    if loaded.is_empty() {
                        continue;
                    }

                    let ids: Vec<String> = (0..loaded.len()).map(|idx| format!("p{idx}")).collect();
                    let wire: Vec<BatchPage<'_>> = loaded
                        .iter()
                        .zip(&ids)
                        .map(|((uid, html), id)| BatchPage {
                            id: id.as_str(),
                            html: html.as_str(),
                            origin: unique[*uid].origin.as_deref(),
                        })
                        .collect();

                    let mut retried_429 = false;
                    let response = loop {
                        match converter.convert_batch_with_retry(&wire) {
                            Ok(res) => break Ok(res),
                            Err((err, Some(secs)))
                                if err.status == Some(429)
                                    && !retried_429
                                    && state.schedule_rate_limit_wait(secs) =>
                            {
                                retried_429 = true;
                                // Another batch may have gone fatal while we slept.
                                if let Some(e) = state.fatal() {
                                    break Err(e);
                                }
                            }
                            Err((err, _)) => break Err(err),
                        }
                    };

                    match response {
                        Ok(items) => {
                            let mut by_id: HashMap<String, Outcome> = items.into_iter().collect();
                            for ((uid, _), id) in loaded.iter().zip(&ids) {
                                let uid = *uid;
                                let outcome = by_id.remove(id).unwrap_or_else(|| {
                                    Err(ConvertError::new("converter omitted page result"))
                                });
                                // A per-item 429 only costs that page; 401/403 mean the
                                // whole run is unauthorized.
                                if let Err(e) = &outcome
                                    && matches!(e.status, Some(401 | 403))
                                {
                                    state.set_fatal(e.clone());
                                }
                                if allow_503_retry
                                    && matches!(&outcome, Err(e) if e.status == Some(503))
                                {
                                    retry_503.lock().unwrap().push(uid);
                                } else {
                                    *unique_outcomes[uid].lock().unwrap() = Some(outcome);
                                }
                            }
                        }
                        Err(err) => {
                            if is_fatal_status(err.status) {
                                state.set_fatal(err.clone());
                            }
                            for (uid, _) in &loaded {
                                let uid = *uid;
                                *unique_outcomes[uid].lock().unwrap() = Some(Err(err.clone()));
                            }
                        }
                    }
                }
            });
        }
    });
    retry_503.into_inner().unwrap()
}

fn stage_siblings(
    tree: &mut ScannedTree,
    dir: TempDir,
    converted: impl Iterator<Item = (String, Outcome)>,
) -> Result<SiblingGuard, String> {
    let mut staged = Vec::new();
    let mut skipped = 0;
    let mut first_error = None;
    for (md_path, result) in converted {
        let markdown = match result {
            Ok(markdown) => markdown,
            Err(e) => {
                skipped += 1;
                first_error.get_or_insert(e.message);
                continue;
            }
        };
        let bytes = markdown.into_bytes();
        let hash = blob_id(&bytes);
        let blob_path = dir.path().join(hash.trim_start_matches("b3:"));
        if !blob_path.is_file() {
            std::fs::write(&blob_path, &bytes)
                .map_err(|e| format!("cannot write {}: {e}", blob_path.display()))?;
        }
        staged.push((
            blob_path,
            ScannedFileEntry {
                path: md_path,
                hash,
                size: bytes.len(),
                r#type: MARKDOWN_MIME.to_owned(),
                immutable: None,
            },
        ));
    }
    // A failed write drops the scratch directory. Publish entries only once
    // every blob is ready so callers can still deploy the original tree.
    let generated = staged.len();
    for (path, file) in staged {
        tree.total_bytes += file.size;
        tree.blobs_by_hash
            .entry(file.hash.clone())
            .or_insert_with_key(|hash| LocalBlob {
                hash: hash.clone(),
                path,
            });
        tree.files.push(file);
    }
    Ok(SiblingGuard {
        _dir: dir,
        generated,
        skipped,
        first_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::static_cas::scan_output_dir;

    #[test]
    fn failed_scratch_write_preserves_the_upload_tree() {
        let root = TempDir::new().unwrap();
        std::fs::write(root.path().join("index.html"), "original").unwrap();
        let mut tree = scan_output_dir(root.path()).unwrap();
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(
            dir.path()
                .join(blob_id(b"second").trim_start_matches("b3:")),
        )
        .unwrap();
        let converted = [
            ("/a.md".into(), Ok("first".into())),
            ("/b.md".into(), Ok("second".into())),
        ];
        assert!(stage_siblings(&mut tree, dir, converted.into_iter()).is_err());
        assert_eq!(tree.files.len(), 1);
        assert_eq!(tree.files[0].path, "/index.html");
        assert_eq!(tree.blobs_by_hash.len(), 1);
        assert_eq!(tree.total_bytes, "original".len());
    }
}
