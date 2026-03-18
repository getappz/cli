//! site2static — export a running site as static HTML.
//!
//! Crawls a local dev server over HTTP, copies assets from the local
//! filesystem, and rewrites URLs for offline navigation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

mod css;
mod disk;
mod dom;
mod downloader;
mod local_file;
mod metadata;
mod mirror;
mod response;
mod sitemap;
mod url_utils;

/// Progress events emitted during the mirror operation.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Discovering sitemap URLs.
    DiscoveringSitemap,
    /// Sitemap discovery complete.
    SitemapDone { urls_found: usize },
    /// Crawling pages and copying assets.
    Crawling { pages: u64, assets: u64 },
    /// Export complete.
    Done { pages: u64, assets: u64, duration: Duration },
}

/// Local filesystem root for asset copy.
pub enum WebRoot {
    /// Single directory — URL paths map directly.
    Direct(PathBuf),
    /// Multiple search paths — try each in order.
    Search(Vec<PathBuf>),
}

/// Configuration for a static site export.
pub struct MirrorConfig {
    /// URL of the running site (e.g. `http://localhost:8080`).
    pub origin: Url,
    /// Local webroot for filesystem asset copy.
    pub webroot: WebRoot,
    /// Output directory for the static export.
    pub output: PathBuf,
    /// Number of concurrent workers (default: 8).
    pub workers: usize,
    /// Max crawl depth (`None` = unlimited).
    pub depth: Option<u32>,
    /// Force full re-crawl (ignore incremental cache).
    pub force: bool,
    /// URL exclude patterns (regex).
    pub exclude_patterns: Vec<String>,
    /// URL include patterns (regex).
    pub include_patterns: Vec<String>,
    /// Directories to copy entirely from webroot to output. Catches assets that
    /// are dynamically loaded by JavaScript (webpack chunks, lazy CSS/JS) and
    /// can't be discovered via HTML parsing. Paths are relative to the webroot.
    pub copy_dirs: Vec<String>,
    /// Optional progress callback. Called from worker threads.
    pub on_progress: Option<Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
}

/// Result of a completed mirror operation.
pub struct MirrorResult {
    pub pages_crawled: u64,
    pub assets_copied: u64,
    pub output_dir: PathBuf,
    pub duration: Duration,
}

/// Errors that can occur during mirroring.
#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("origin unreachable: {url} — {message}")]
    OriginUnreachable { url: String, message: String },

    #[error("output directory not writable: {0}")]
    OutputNotWritable(PathBuf),

    #[error("HTTP error fetching {url}: {message}")]
    HttpError { url: String, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Static site exporter. Crawls a running site and produces a static copy.
pub struct SiteMirror {
    config: MirrorConfig,
}

impl SiteMirror {
    pub fn new(config: MirrorConfig) -> Self {
        Self { config }
    }

    /// Run the export. Consumes self (single-use).
    pub fn run(self) -> Result<MirrorResult, MirrorError> {
        mirror::run(self.config)
    }
}
