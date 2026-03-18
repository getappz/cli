use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam::channel::{Receiver, Sender, TryRecvError};
use dashmap::DashSet;
use encoding_rs::Encoding;
use url::Url;

use crate::css::Css;
use crate::disk;
use crate::dom::Dom;
use crate::downloader::Downloader;
use crate::local_file;
use crate::metadata::{self, FileMetadata, MetadataCache};
use crate::response::ResponseData;
use crate::sitemap;
use crate::url_utils;
use crate::{MirrorConfig, MirrorError, MirrorResult, ProgressEvent};

// url_utils is kept for is_same_domain(); other URL functions use ufo_rs directly.

/// Maximum number of consecutive empty recv() before a worker exits.
const MAX_EMPTY_RECEIVES: usize = 5;

/// Sleep duration between empty receives.
const SLEEP_DURATION: Duration = Duration::from_millis(500);

// ---------------------------------------------------------------------------
// URL classification
// ---------------------------------------------------------------------------

/// Returns `true` when the URL looks like an HTML page rather than a static
/// asset. The heuristic matches the legacy scraper: `.html`/`.htm` extension,
/// no extension at all, trailing slash, or empty path.
fn is_html_url(url: &Url) -> bool {
    let path = url.path();
    let lower = path.to_lowercase();

    if lower.ends_with(".html") || lower.ends_with(".htm") {
        return true;
    }
    if path.is_empty() || path == "/" || path.ends_with('/') {
        return true;
    }
    // No file-extension segment after the last `/` → treat as page.
    if let Some(last_segment) = path.rsplit('/').next() {
        if !last_segment.contains('.') {
            return true;
        }
    }
    false
}

/// Returns `true` when the URL path ends with `.css`.
fn is_css_url(url: &Url) -> bool {
    url.path().to_lowercase().ends_with(".css")
}

// ---------------------------------------------------------------------------
// Charset helpers
// ---------------------------------------------------------------------------

/// Detect charset from `<meta>` tag (safe: uses `from_utf8_lossy`).
fn find_charset(data: &[u8], http_charset: Option<String>) -> String {
    let lossy = String::from_utf8_lossy(data);
    let re = regex::Regex::new(r#"<meta[^>]*charset\s*=\s*["']?([^"'\s;>]+)"#).unwrap();
    if let Some(caps) = re.captures(&lossy) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_lowercase();
        }
    }
    http_charset.unwrap_or_else(|| "utf-8".into())
}

/// Convert bytes between encodings.
fn charset_convert(data: &[u8], from: &'static Encoding, to: &'static Encoding) -> Vec<u8> {
    let (decoded, _, _) = from.decode(data);
    let (encoded, _, _) = to.encode(&decoded);
    encoded.into_owned()
}

// ---------------------------------------------------------------------------
// Shared crawl state
// ---------------------------------------------------------------------------

struct CrawlState<'a> {
    config: &'a MirrorConfig,
    downloader: Downloader,
    visited: DashSet<String>,
    metadata_cache: Arc<Mutex<MetadataCache>>,
    pages_crawled: AtomicU64,
    assets_copied: AtomicU64,
    robots_txt: String,
}

impl<'a> CrawlState<'a> {
    fn emit_progress(&self, event: ProgressEvent) {
        if let Some(cb) = &self.config.on_progress {
            cb(event);
        }
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn run(config: MirrorConfig) -> Result<MirrorResult, MirrorError> {
    let start = Instant::now();

    // Ensure output directory exists.
    std::fs::create_dir_all(&config.output)?;

    let downloader = Downloader::new(3);

    // Load incremental metadata cache (unless force mode).
    let metadata_cache = if config.force {
        MetadataCache::new()
    } else {
        metadata::load_metadata(&config.output)
    };
    let metadata_cache = Arc::new(Mutex::new(metadata_cache));

    // ------------------------------------------------------------------
    // Phase 1 — Sitemap + robots.txt discovery
    // ------------------------------------------------------------------
    if let Some(cb) = &config.on_progress {
        cb(ProgressEvent::DiscoveringSitemap);
    }
    let robots_txt = fetch_robots_txt(&config.origin, &downloader);
    let sitemap_urls = sitemap::discover_urls(&config.origin, &downloader);

    if let Some(cb) = &config.on_progress {
        cb(ProgressEvent::SitemapDone { urls_found: sitemap_urls.len() });
    }

    tracing::info!(
        "Phase 1 complete: {} sitemap URLs, robots.txt {} bytes",
        sitemap_urls.len(),
        robots_txt.len(),
    );

    // ------------------------------------------------------------------
    // Phase 2 — Parallel crawl
    // ------------------------------------------------------------------
    let (tx, rx) = crossbeam::channel::unbounded::<(Url, i32)>();

    let state = CrawlState {
        config: &config,
        downloader,
        visited: DashSet::new(),
        metadata_cache: metadata_cache.clone(),
        pages_crawled: AtomicU64::new(0),
        assets_copied: AtomicU64::new(0),
        robots_txt,
    };

    // Seed the queue with the origin URL.
    let _ = tx.send((config.origin.clone(), 0));
    state.visited.insert(config.origin.to_string());

    // Seed sitemap-discovered URLs.
    for url in &sitemap_urls {
        if state.visited.insert(url.to_string()) {
            let _ = tx.send((url.clone(), 0));
        }
    }

    // Spawn worker threads.
    let worker_count = config.workers.max(1);
    let state_ref = &state;
    let result = crossbeam::thread::scope(|scope| {
        for _ in 0..worker_count {
            let tx_clone = tx.clone();
            let rx_clone = rx.clone();
            scope.spawn(move |_| {
                process_queue_loop(state_ref, &tx_clone, rx_clone);
            });
        }
        // Drop the sender held by the main thread so the channel closes when
        // all workers are done.
        drop(tx);
        drop(rx);
    });

    if let Err(e) = result {
        tracing::error!("Worker thread panicked: {:?}", e);
    }

    // ------------------------------------------------------------------
    // Phase 2b — Copy supplemental directories (JS-dynamically-loaded assets)
    // ------------------------------------------------------------------
    let extra_assets = copy_supplemental_globs(&config);

    // ------------------------------------------------------------------
    // Phase 3 — Finalize
    // ------------------------------------------------------------------
    let pages = state.pages_crawled.load(Ordering::Relaxed);
    let assets = state.assets_copied.load(Ordering::Relaxed) + extra_assets;

    // Save metadata cache (only for incremental mode).
    if !config.force {
        if let Ok(cache) = metadata_cache.lock() {
            if !cache.entries.is_empty() {
                metadata::save_metadata(&config.output, &cache);
            }
        }
    }

    let duration = start.elapsed();

    if let Some(cb) = &config.on_progress {
        cb(ProgressEvent::Done { pages, assets, duration });
    }

    Ok(MirrorResult {
        pages_crawled: pages,
        assets_copied: assets,
        output_dir: config.output.clone(),
        duration,
    })
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

fn process_queue_loop(state: &CrawlState, tx: &Sender<(Url, i32)>, rx: Receiver<(Url, i32)>) {
    let mut empty_count: usize = 0;

    while empty_count < MAX_EMPTY_RECEIVES {
        match rx.try_recv() {
            Ok((url, depth)) => {
                empty_count = 0;
                handle_url(state, tx, &url, depth);
            }
            Err(TryRecvError::Empty) => {
                empty_count += 1;
                std::thread::sleep(SLEEP_DURATION);
            }
            Err(TryRecvError::Disconnected) => return,
        }
    }
}

// ---------------------------------------------------------------------------
// URL dispatch
// ---------------------------------------------------------------------------

fn handle_url(state: &CrawlState, tx: &Sender<(Url, i32)>, url: &Url, depth: i32) {
    if is_html_url(url) {
        handle_html_url(state, tx, url, depth);
    } else {
        handle_asset_url(state, tx, url);
    }
}

// ---------------------------------------------------------------------------
// HTML page handling
// ---------------------------------------------------------------------------

fn handle_html_url(state: &CrawlState, tx: &Sender<(Url, i32)>, url: &Url, depth: i32) {
    // Conditional GET for incremental builds.
    let (cached_etag, cached_lm) = get_cached_metadata(state, url);

    let response = if !state.config.force && (cached_etag.is_some() || cached_lm.is_some()) {
        match state.downloader.get_conditional(
            url,
            cached_etag.as_deref(),
            cached_lm.as_deref(),
        ) {
            Ok(resp) => {
                if resp.not_modified {
                    tracing::debug!("304 Not Modified: {}", url);
                    // Still need to update metadata in cache.
                    update_metadata(state, url, resp.etag.as_deref(), resp.last_modified.as_deref());
                    return;
                }
                resp
            }
            Err(e) => {
                tracing::warn!("HTTP error for {}: {}", url, e);
                return;
            }
        }
    } else {
        match state.downloader.get(url) {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!("HTTP error for {}: {}", url, e);
                return;
            }
        }
    };

    // Handle redirects — mark both original and final URL as visited.
    if let Some(ref final_url) = response.final_url {
        if final_url.as_str() != url.as_str() {
            state.visited.insert(final_url.to_string());
        }
    }

    let raw_data = match &response.data {
        ResponseData::Html(d) => d.clone(),
        ResponseData::Css(d) => d.clone(),
        ResponseData::Other(d) => d.clone(),
    };

    // Charset detection and conversion to UTF-8.
    let charset_label = find_charset(&raw_data, response.charset.clone());
    let needs_conversion = charset_label != "utf-8";
    let charset_enc = Encoding::for_label(charset_label.as_bytes()).unwrap_or(encoding_rs::UTF_8);

    let utf8_data = if needs_conversion {
        charset_convert(&raw_data, charset_enc, encoding_rs::UTF_8)
    } else {
        raw_data.clone()
    };

    let html_str = String::from_utf8_lossy(&utf8_data);

    // DOM rewriting — extract URLs and rewrite same-domain references.
    let mut dom = Dom::new(&html_str);
    let base_url = response.final_url.as_ref().unwrap_or(url);
    let discovered = dom.find_urls_as_strings(base_url);

    // Split discovered URLs into pages and assets, filter, and enqueue.
    let (page_links, asset_links): (Vec<String>, Vec<String>) =
        discovered.into_iter().partition(|link| {
            // Resolve relative URL to determine type.
            match resolve_link(base_url, link) {
                Some(resolved) => is_html_url(&resolved),
                None => false,
            }
        });

    // Filter page URLs through crawl_core::filter_links.
    enqueue_page_links(state, tx, url, depth, &page_links);

    // Enqueue asset URLs with simple same-domain + not-visited check.
    enqueue_asset_links(state, tx, base_url, &asset_links);

    // Serialize the rewritten HTML.
    let rewritten = dom.serialize();
    let output_bytes = if needs_conversion {
        charset_convert(rewritten.as_bytes(), encoding_rs::UTF_8, charset_enc)
    } else {
        rewritten.into_bytes()
    };

    // Write to output.
    let file_path = local_file::url_to_path(url);
    disk::save_file(&file_path, &output_bytes, &state.config.output, None);

    // Save metadata for incremental crawling.
    update_metadata(state, url, response.etag.as_deref(), response.last_modified.as_deref());
    let pages = state.pages_crawled.fetch_add(1, Ordering::Relaxed) + 1;
    let assets = state.assets_copied.load(Ordering::Relaxed);
    state.emit_progress(ProgressEvent::Crawling { pages, assets });

    tracing::debug!("Crawled HTML: {}", url);
}

// ---------------------------------------------------------------------------
// Asset handling
// ---------------------------------------------------------------------------

fn handle_asset_url(state: &CrawlState, tx: &Sender<(Url, i32)>, url: &Url) {
    let file_path = local_file::url_to_path(url);

    // Incremental skip: check if file differs from source.
    if !state.config.force {
        let rel = ufo_rs::without_leading_slash(url.path());
        if let Some(source_path) = local_file::resolve_local_path(&state.config.webroot, &rel) {
            let dest = state.config.output.join(&file_path);
            match disk::files_differ_fast(&source_path, &dest) {
                Ok(Some(false)) => {
                    tracing::debug!("Skipping unchanged asset: {}", url);
                    let assets = state.assets_copied.fetch_add(1, Ordering::Relaxed) + 1;
                    let pages = state.pages_crawled.load(Ordering::Relaxed);
                    state.emit_progress(ProgressEvent::Crawling { pages, assets });
                    return;
                }
                _ => {} // Differ or uncertain → proceed.
            }
        }
    }

    // Try local filesystem first.
    let data = match local_file::read_local_file(&state.config.webroot, url) {
        Ok(bytes) => bytes,
        Err(_) => {
            // Fall back to HTTP download.
            match state.downloader.get(url) {
                Ok(resp) => match resp.data {
                    ResponseData::Html(d) | ResponseData::Css(d) | ResponseData::Other(d) => d,
                },
                Err(e) => {
                    tracing::warn!("Could not fetch asset {}: {}", url, e);
                    return;
                }
            }
        }
    };

    // For CSS files, rewrite url() references and discover sub-assets.
    let output_data = if is_css_url(url) {
        let css_str = String::from_utf8_lossy(&data);
        let mut css = Css::new(&css_str);
        let css_urls = css.find_urls_as_strings(url);

        // Enqueue discovered CSS sub-assets.
        enqueue_asset_links(state, tx, url, &css_urls);

        css.serialize().into_bytes()
    } else {
        data
    };

    // Preserve source mtime when copying from local.
    let rel = ufo_rs::without_leading_slash(url.path());
    let source_path = local_file::resolve_local_path(&state.config.webroot, &rel);

    disk::save_file(
        &file_path,
        &output_data,
        &state.config.output,
        source_path.as_deref(),
    );

    let assets = state.assets_copied.fetch_add(1, Ordering::Relaxed) + 1;
    let pages = state.pages_crawled.load(Ordering::Relaxed);
    state.emit_progress(ProgressEvent::Crawling { pages, assets });
    tracing::debug!("Copied asset: {}", url);
}

// ---------------------------------------------------------------------------
// Link filtering and enqueueing
// ---------------------------------------------------------------------------

/// Filter page links through crawl_core and enqueue accepted ones.
fn enqueue_page_links(
    state: &CrawlState,
    tx: &Sender<(Url, i32)>,
    current_url: &Url,
    depth: i32,
    links: &[String],
) {
    if links.is_empty() {
        return;
    }

    let max_depth = state.config.depth.unwrap_or(u32::MAX);

    // Resolve all relative links to absolute before filtering.
    let resolved: Vec<String> = links
        .iter()
        .filter_map(|l| resolve_link(current_url, l).map(|u| u.to_string()))
        .collect();

    if resolved.is_empty() {
        return;
    }

    let call = crawl_core::FilterLinksCall {
        links: resolved,
        limit: None,
        max_depth,
        base_url: state.config.origin.to_string(),
        initial_url: state.config.origin.to_string(),
        regex_on_full_url: true,
        excludes: state.config.exclude_patterns.clone(),
        includes: state.config.include_patterns.clone(),
        allow_backward_crawling: true,
        ignore_robots_txt: state.robots_txt.is_empty(),
        robots_txt: state.robots_txt.clone(),
        allow_external_content_links: false,
        allow_subdomains: false,
    };

    match crawl_core::filter_links(call) {
        Ok(result) => {
            for link_str in &result.links {
                if let Ok(link_url) = Url::parse(link_str) {
                    if state.visited.insert(link_url.to_string()) {
                        let _ = tx.send((link_url, depth + 1));
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("filter_links error: {}", e);
        }
    }
}

/// Enqueue asset links with simple same-domain + not-visited filter.
fn enqueue_asset_links(
    state: &CrawlState,
    tx: &Sender<(Url, i32)>,
    base_url: &Url,
    links: &[String],
) {
    for link in links {
        let resolved = match resolve_link(base_url, link) {
            Some(u) => u,
            None => continue,
        };

        if !url_utils::is_same_domain(resolved.as_str(), state.config.origin.as_str()) {
            continue;
        }

        if state.visited.insert(resolved.to_string()) {
            let _ = tx.send((resolved, 0));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a possibly-relative link against a base URL.
fn resolve_link(base: &Url, link: &str) -> Option<Url> {
    let normalized = ufo_rs::normalize_url(link);
    match Url::parse(&normalized) {
        Ok(u) => Some(u),
        Err(_) => base.join(&normalized).ok(),
    }
}

/// Copy files matching glob patterns from webroot to output. Catches assets
/// dynamically loaded by JavaScript that can't be discovered via HTML parsing.
/// Returns the number of files copied.
fn copy_supplemental_globs(config: &MirrorConfig) -> u64 {
    if config.copy_globs.is_empty() {
        return 0;
    }

    let webroot = match &config.webroot {
        crate::WebRoot::Direct(p) => p.clone(),
        crate::WebRoot::Search(paths) => match paths.first() {
            Some(p) => p.clone(),
            None => return 0,
        },
    };

    let mut copied = 0u64;
    for pattern in &config.copy_globs {
        let full_pattern = webroot.join(pattern).display().to_string();
        let matches = match glob::glob(&full_pattern) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Invalid glob pattern '{}': {}", pattern, e);
                continue;
            }
        };

        for entry in matches.flatten() {
            if !entry.is_file() {
                continue;
            }
            // Compute relative path from webroot
            let rel = match entry.strip_prefix(&webroot) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let dst = config.output.join(rel);

            // Skip unchanged files unless force mode
            if !config.force {
                match disk::files_differ_fast(&entry, &dst) {
                    Ok(Some(false)) => continue,
                    _ => {}
                }
            }

            // Ensure parent dir exists
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::copy(&entry, &dst) {
                tracing::warn!("Failed to copy {}: {}", rel.display(), e);
                continue;
            }
            // Preserve mtime
            if let Ok(meta) = std::fs::metadata(&entry) {
                if let Ok(mtime) = meta.modified() {
                    let ft = filetime::FileTime::from_system_time(mtime);
                    let _ = filetime::set_file_times(&dst, ft, ft);
                }
            }
            copied += 1;
        }
    }

    if copied > 0 {
        tracing::info!("Copied {} supplemental asset files", copied);
    }
    copied
}

/// Fetch `/robots.txt` from the origin. Returns empty string on failure.
fn fetch_robots_txt(origin: &Url, downloader: &Downloader) -> String {
    let robots_url = match origin.join("/robots.txt") {
        Ok(u) => u,
        Err(_) => return String::new(),
    };
    match downloader.get(&robots_url) {
        Ok(resp) => {
            let bytes = match resp.data {
                ResponseData::Html(b) | ResponseData::Css(b) | ResponseData::Other(b) => b,
            };
            String::from_utf8(bytes).unwrap_or_default()
        }
        Err(_) => String::new(),
    }
}

/// Retrieve cached ETag / Last-Modified for a URL.
fn get_cached_metadata(state: &CrawlState, url: &Url) -> (Option<String>, Option<String>) {
    if let Ok(cache) = state.metadata_cache.lock() {
        if let Some(meta) = cache.get(url.as_str()) {
            return (meta.etag.clone(), meta.last_modified.clone());
        }
    }
    (None, None)
}

/// Store / update metadata for a URL.
fn update_metadata(state: &CrawlState, url: &Url, etag: Option<&str>, last_modified: Option<&str>) {
    if let Ok(mut cache) = state.metadata_cache.lock() {
        cache.set(
            url.to_string(),
            FileMetadata {
                etag: etag.map(String::from),
                last_modified: last_modified.map(String::from),
                file_hash: None,
            },
        );
    }
}
