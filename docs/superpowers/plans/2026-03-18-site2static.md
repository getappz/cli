# site2static Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a CMS-agnostic static site exporter that crawls a local dev server and copies assets from the filesystem, replacing the Simply Static WordPress plugin dependency.

**Architecture:** Port the legacy `sitescrape` crate into a new `crates/site2static` workspace member. Use crossbeam workers for parallel HTML crawling, `lol_html` for streaming URL rewriting, filesystem copy for assets, and `crawl-core` (sync mode) for link filtering and sitemap parsing. Integrate into blueprint's `StaticExporter` as a drop-in replacement.

**Tech Stack:** Rust, lol_html, reqwest (blocking), crossbeam, DashMap/DashSet, crawl-core, encoding_rs, regex, flate2

**Spec:** `docs/superpowers/specs/2026-03-18-site2static-design.md`

**Legacy source:** `/home/avihs/workspace/appz-cli-legacy/crates/sitescrape/`

---

## File Structure

### New files (crates/site2static/)

| File | Responsibility | Ported from |
|------|---------------|-------------|
| `Cargo.toml` | Crate manifest with workspace deps | New |
| `src/lib.rs` | Public API: `SiteMirror`, `MirrorConfig`, `MirrorResult`, `MirrorError`, `WebRoot` | New |
| `src/url_utils.rs` | `is_same_domain()`, `without_leading_slash()`, `without_trailing_slash()`, `normalize_url()` | legacy `ufo/src/lib.rs` + `ufo/src/url_cache.rs` + `ufo/src/utils.rs` |
| `src/response.rs` | `ResponseData`, `Response` types | legacy `sitescrape/src/response.rs` |
| `src/metadata.rs` | `MetadataCache`, `FileMetadata`, load/save | legacy `sitescrape/src/metadata.rs` |
| `src/disk.rs` | `save_file()`, `files_differ_fast()`, mtime preservation | legacy `sitescrape/src/disk.rs` |
| `src/local_file.rs` | `url_to_path()`, `resolve_local_path()`, `read_local_file()` with `WebRoot` | legacy `sitescrape/src/local_file.rs` + `url_helper.rs` |
| `src/downloader.rs` | HTTP client with conditional GET (ETag/Last-Modified) | legacy `sitescrape/src/downloader.rs` |
| `src/css.rs` | CSS `url()` extraction and rewriting | legacy `sitescrape/src/css.rs` |
| `src/dom.rs` | `lol_html` HTML URL extraction and rewriting | legacy `sitescrape/src/dom.rs` |
| `src/sitemap.rs` | Sitemap discovery with recursive fetching | New (wraps `crawl-core`) |
| `src/mirror.rs` | Core crawl loop: worker pool, URL queue, HTML/asset dispatch | legacy `sitescrape/src/scraper.rs` |

### Modified files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Add `crates/site2static` to workspace members |
| `crates/blueprint/Cargo.toml` | Add `site2static` dependency, add `url` dependency |
| `crates/blueprint/src/static_export.rs` | Replace Simply Static plugin calls with `SiteMirror` |

---

## Task 1: Scaffold crate and public API types

**Files:**
- Create: `crates/site2static/Cargo.toml`
- Create: `crates/site2static/src/lib.rs`
- Modify: `Cargo.toml` (workspace root, members list)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "site2static"
version = "0.1.0"
edition = "2021"
description = "Static site exporter — crawl a local dev server and mirror it as static HTML"
publish = false

[dependencies]
crawl-core = { path = "../crawl-core", default-features = false }
crossbeam = "0.8"
dashmap = "6.1.0"
encoding_rs = "0.8"
filetime = "0.2"
flate2 = "1.0"
lol_html = "2"
pathdiff = "0.2"
regex = { workspace = true }
reqwest = { workspace = true, features = ["blocking", "cookies"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = "0.1"
url = { workspace = true }

[dev-dependencies]
tempfile = "3"
tiny_http = "0.12"
```

- [ ] **Step 2: Create src/lib.rs with public API types**

```rust
//! site2static — export a running site as static HTML.
//!
//! Crawls a local dev server over HTTP, copies assets from the local
//! filesystem, and rewrites URLs for offline navigation.

use std::path::PathBuf;
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
```

- [ ] **Step 3: Add crate to workspace members**

In `Cargo.toml` (workspace root), add `"crates/site2static"` to the `members` array.

- [ ] **Step 4: Create stub modules so it compiles**

Create empty stub files for all internal modules: `src/css.rs`, `src/disk.rs`, `src/dom.rs`, `src/downloader.rs`, `src/local_file.rs`, `src/metadata.rs`, `src/mirror.rs`, `src/response.rs`, `src/sitemap.rs`, `src/url_utils.rs`.

The `mirror.rs` stub needs a `run` function:

```rust
use crate::{MirrorConfig, MirrorResult, MirrorError};

pub fn run(_config: MirrorConfig) -> Result<MirrorResult, MirrorError> {
    todo!()
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p site2static`
Expected: compiles with no errors (warnings about unused modules are OK)

- [ ] **Step 6: Commit**

```bash
git add crates/site2static/ Cargo.toml
git commit -m "feat(site2static): scaffold crate with public API types"
```

---

## Task 2: Port url_utils

**Files:**
- Create: `crates/site2static/src/url_utils.rs`

Port the URL utility functions used by the scraper from legacy `ufo/src/lib.rs`, `ufo/src/utils.rs`, and `ufo/src/url_cache.rs`. Only the functions actually called by other modules.

- [ ] **Step 1: Write tests**

```rust
// crates/site2static/src/url_utils.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_without_leading_slash() {
        assert_eq!(without_leading_slash("/foo"), "foo");
        assert_eq!(without_leading_slash("foo"), "foo");
        assert_eq!(without_leading_slash(""), "");
    }

    #[test]
    fn test_without_trailing_slash() {
        assert_eq!(without_trailing_slash("/foo/"), "/foo");
        assert_eq!(without_trailing_slash("/foo"), "/foo");
        assert_eq!(without_trailing_slash("/path/?q=1", ), "/path?q=1");
    }

    #[test]
    fn test_is_same_domain() {
        assert!(is_same_domain("https://example.com/a", "https://example.com/b"));
        assert!(is_same_domain("http://example.com", "https://example.com"));
        assert!(is_same_domain("//example.com/x", "https://example.com/y"));
        assert!(!is_same_domain("https://a.com", "https://b.com"));
    }

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("///example.com/"), "https://example.com/");
        assert_eq!(normalize_url("//example.com/"), "https://example.com/");
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static url_utils`
Expected: FAIL — functions not defined

- [ ] **Step 3: Implement url_utils.rs**

Port from legacy `ufo` crate — only the 4 functions we need:

```rust
//! URL utility functions inlined from legacy urlz/ufo crate.

use url::Url;

/// Remove leading slash from a path.
pub fn without_leading_slash(input: &str) -> &str {
    input.strip_prefix('/').unwrap_or(input)
}

/// Remove trailing slash, respecting query strings and fragments.
pub fn without_trailing_slash(input: &str) -> String {
    let end_pos = input
        .find('?')
        .or_else(|| input.find('#'))
        .unwrap_or(input.len());
    let (path, suffix) = input.split_at(end_pos);
    let trimmed = path.trim_end_matches('/');
    format!("{}{}", trimmed, suffix)
}

/// Check if two URLs share the same host (case-insensitive, ignores port/scheme).
pub fn is_same_domain(a: &str, b: &str) -> bool {
    let na = ensure_https(a);
    let nb = ensure_https(b);
    match (Url::parse(&na), Url::parse(&nb)) {
        (Ok(ua), Ok(ub)) => match (ua.host_str(), ub.host_str()) {
            (Some(ha), Some(hb)) => ha.eq_ignore_ascii_case(hb),
            _ => false,
        },
        _ => false,
    }
}

/// Normalize scheme-relative and triple-slash URLs.
pub fn normalize_url(input: &str) -> String {
    if input.starts_with("///") {
        input.replacen("///", "https://", 1)
    } else if input.starts_with("//") {
        input.replacen("//", "https://", 1)
    } else {
        input.to_string()
    }
}

fn ensure_https(input: &str) -> String {
    if input.starts_with("//") {
        format!("https:{}", input)
    } else {
        input.to_string()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p site2static url_utils`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/url_utils.rs
git commit -m "feat(site2static): port url_utils from legacy ufo crate"
```

---

## Task 3: Port response and metadata modules

**Files:**
- Create: `crates/site2static/src/response.rs`
- Create: `crates/site2static/src/metadata.rs`

- [ ] **Step 1: Write metadata round-trip test**

```rust
// At end of metadata.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_round_trip() {
        let dir = TempDir::new().unwrap();
        let mut cache = MetadataCache::new();
        cache.set("https://example.com/".into(), FileMetadata {
            etag: Some("W/\"abc\"".into()),
            last_modified: Some("Wed, 21 Oct 2023".into()),
            file_hash: None,
        });
        save_metadata(&dir.path().to_path_buf(), &cache);
        let loaded = load_metadata(&dir.path().to_path_buf());
        let entry = loaded.get("https://example.com/").unwrap();
        assert_eq!(entry.etag.as_deref(), Some("W/\"abc\""));
        assert_eq!(entry.last_modified.as_deref(), Some("Wed, 21 Oct 2023"));
    }

    #[test]
    fn test_metadata_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let cache = load_metadata(&dir.path().to_path_buf());
        assert!(cache.entries.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static metadata`
Expected: FAIL

- [ ] **Step 3: Implement response.rs**

Port from legacy `sitescrape/src/response.rs` — direct copy with no changes needed:

```rust
/// Content type classification for responses.
pub enum ResponseData {
    Html(Vec<u8>),
    Css(Vec<u8>),
    Other(Vec<u8>),
}

/// HTTP response wrapper with metadata for incremental crawling.
pub struct Response {
    pub data: ResponseData,
    pub filename: Option<String>,
    pub charset: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
    /// The final URL after redirects (for visited-set deduplication).
    pub final_url: Option<url::Url>,
}

impl Response {
    pub fn new(data: ResponseData, filename: Option<String>, charset: Option<String>) -> Self {
        Self { data, filename, charset, etag: None, last_modified: None, not_modified: false, final_url: None }
    }

    pub fn with_metadata(
        data: ResponseData, filename: Option<String>, charset: Option<String>,
        etag: Option<String>, last_modified: Option<String>, final_url: Option<url::Url>,
    ) -> Self {
        Self { data, filename, charset, etag, last_modified, not_modified: false, final_url }
    }

    pub fn not_modified(etag: Option<String>, last_modified: Option<String>) -> Self {
        Self {
            data: ResponseData::Other(Vec::new()),
            filename: None, charset: None,
            etag, last_modified, not_modified: true, final_url: None,
        }
    }
}
```

- [ ] **Step 4: Implement metadata.rs**

Port from legacy `sitescrape/src/metadata.rs`. Key changes:
- Rename cache file to `.site2static-metadata.json`
- Replace `crate::{info, warn}` macros with `tracing::{info, warn}`
- Change `load_metadata`/`save_metadata` to take `&Path` instead of `&Option<PathBuf>`
- Remove `find_appz_dir` walk-up logic (output dir is always known)
- Remove `check_index_exists` (not needed)

```rust
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

const METADATA_FILENAME: &str = ".site2static-metadata.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub file_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataCache {
    #[serde(flatten)]
    pub entries: HashMap<String, FileMetadata>,
}

impl Default for MetadataCache {
    fn default() -> Self { Self::new() }
}

impl MetadataCache {
    pub fn new() -> Self { Self { entries: HashMap::new() } }
    pub fn get(&self, url: &str) -> Option<&FileMetadata> { self.entries.get(url) }
    pub fn set(&mut self, url: String, metadata: FileMetadata) { self.entries.insert(url, metadata); }
}

pub fn load_metadata(output_dir: &Path) -> MetadataCache {
    let path = output_dir.join(METADATA_FILENAME);
    if !path.exists() { return MetadataCache::new(); }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse metadata {}: {}", path.display(), e);
            MetadataCache::new()
        }),
        Err(e) => {
            tracing::warn!("Failed to read metadata {}: {}", path.display(), e);
            MetadataCache::new()
        }
    }
}

pub fn save_metadata(output_dir: &Path, cache: &MetadataCache) {
    let path = output_dir.join(METADATA_FILENAME);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(cache) {
        Ok(content) => {
            if let Err(e) = fs::write(&path, content) {
                tracing::warn!("Failed to write metadata {}: {}", path.display(), e);
            }
        }
        Err(e) => tracing::warn!("Failed to serialize metadata: {}", e),
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p site2static metadata`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add crates/site2static/src/response.rs crates/site2static/src/metadata.rs
git commit -m "feat(site2static): port response and metadata modules"
```

---

## Task 4: Port disk utilities

**Files:**
- Create: `crates/site2static/src/disk.rs`

Port from legacy `sitescrape/src/disk.rs`. Remove Elementor-specific code, `appz_common` hash dependency, and symlink logic. Keep `save_file`, `files_differ_fast`, and mtime preservation.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_save_file_creates_dirs() {
        let dir = TempDir::new().unwrap();
        save_file("sub/dir/file.txt", b"hello", dir.path(), None);
        let content = fs::read_to_string(dir.path().join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_files_differ_fast_dest_missing() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, "hello").unwrap();
        assert_eq!(files_differ_fast(&src, &dst).unwrap(), Some(true));
    }

    #[test]
    fn test_files_differ_fast_same_size_same_mtime() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        fs::write(&src, "hello").unwrap();
        // Copy preserving mtime
        save_file("dst.txt", b"hello", dir.path(), Some(src.as_path()));
        assert_eq!(files_differ_fast(&src, &dst).unwrap(), Some(false));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static disk`
Expected: FAIL

- [ ] **Step 3: Implement disk.rs**

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use filetime::{set_file_times, FileTime};

/// Preserve modification time from source to destination.
fn preserve_mtime(source: &Path, dest: &Path) {
    if let Ok(meta) = fs::metadata(source) {
        if let Ok(mtime) = meta.modified() {
            let ft = FileTime::from_system_time(mtime);
            let _ = set_file_times(dest, ft, ft);
        }
    }
}

/// Save content to a file, creating parent dirs. Optionally preserve mtime from source.
pub fn save_file(file_name: &str, content: &[u8], output_dir: &Path, source_path: Option<&Path>) {
    let path = output_dir.join(file_name);
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                tracing::warn!("Couldn't create dir {}: {}", parent.display(), e);
                return;
            }
        }
    }
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    let mut file = match fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => { tracing::warn!("Couldn't create {}: {}", path.display(), e); return; }
    };
    if let Err(e) = file.write_all(content) {
        tracing::warn!("Couldn't write {}: {}", path.display(), e);
        return;
    }
    if let Some(source) = source_path {
        preserve_mtime(source, &path);
    }
}

/// Fast file comparison using size and mtime (no hashing).
/// Returns `Some(true)` if different, `Some(false)` if unchanged, `None` if uncertain.
pub fn files_differ_fast(source: &Path, dest: &Path) -> Result<Option<bool>, std::io::Error> {
    if !dest.exists() {
        return Ok(Some(true));
    }
    let src_meta = source.symlink_metadata()?;
    let dst_meta = dest.symlink_metadata()?;
    if src_meta.len() != dst_meta.len() {
        return Ok(Some(true));
    }
    let src_mtime = src_meta.modified().unwrap_or(std::time::SystemTime::now());
    let dst_mtime = dst_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if src_mtime == dst_mtime {
        return Ok(Some(false));
    }
    Ok(None) // Same size, different mtime — uncertain
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p site2static disk`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/disk.rs
git commit -m "feat(site2static): port disk utilities (save_file, files_differ_fast)"
```

---

## Task 5: Port local_file (with WebRoot support)

**Files:**
- Create: `crates/site2static/src/local_file.rs`

Merge legacy `local_file.rs` (filesystem reading) and `url_helper.rs` (URL-to-path mapping). Add `WebRoot` support.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_url_to_path_root() {
        let url = Url::parse("http://localhost:8080/").unwrap();
        assert_eq!(url_to_path(&url), "index.html");
    }

    #[test]
    fn test_url_to_path_directory() {
        let url = Url::parse("http://localhost:8080/about/").unwrap();
        assert_eq!(url_to_path(&url), "about/index.html");
    }

    #[test]
    fn test_url_to_path_no_extension() {
        let url = Url::parse("http://localhost:8080/about").unwrap();
        assert_eq!(url_to_path(&url), "about/index.html");
    }

    #[test]
    fn test_url_to_path_asset() {
        let url = Url::parse("http://localhost:8080/assets/app.js").unwrap();
        assert_eq!(url_to_path(&url), "assets/app.js");
    }

    #[test]
    fn test_url_to_path_html_file() {
        let url = Url::parse("http://localhost:8080/page.html").unwrap();
        assert_eq!(url_to_path(&url), "page.html");
    }

    #[test]
    fn test_resolve_direct_webroot() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets/style.css"), "body{}").unwrap();

        let webroot = crate::WebRoot::Direct(dir.path().to_path_buf());
        let result = resolve_local_path(&webroot, "assets/style.css");
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_resolve_direct_webroot_missing() {
        let dir = TempDir::new().unwrap();
        let webroot = crate::WebRoot::Direct(dir.path().to_path_buf());
        let result = resolve_local_path(&webroot, "missing.css");
        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static local_file`
Expected: FAIL

- [ ] **Step 3: Implement local_file.rs**

```rust
use std::fs;
use std::path::PathBuf;
use url::Url;
use crate::url_utils;
use crate::WebRoot;

/// Convert a URL to a relative filesystem path for the output directory.
pub fn url_to_path(url: &Url) -> String {
    let url_path = url_utils::without_leading_slash(url.path());
    if url_path.is_empty() {
        return "index.html".into();
    }
    let trimmed = url_utils::without_trailing_slash(url_path);
    let lower = trimmed.to_lowercase();
    let is_html_like = !lower.contains('.') || lower.ends_with(".html") || lower.ends_with(".htm");
    if is_html_like {
        if trimmed.ends_with(".html") || trimmed.ends_with(".htm") {
            trimmed
        } else {
            format!("{}/index.html", trimmed)
        }
    } else {
        trimmed
    }
}

/// Resolve a relative path against the WebRoot. Returns `Some(path)` if the file exists locally.
pub fn resolve_local_path(webroot: &WebRoot, relative_path: &str) -> Option<PathBuf> {
    match webroot {
        WebRoot::Direct(root) => {
            let full = root.join(relative_path);
            if full.exists() { Some(full) } else { None }
        }
        WebRoot::Search(roots) => {
            for root in roots {
                let full = root.join(relative_path);
                if full.exists() { return Some(full); }
            }
            None
        }
    }
}

/// Read a file from the local filesystem based on URL and WebRoot.
pub fn read_local_file(webroot: &WebRoot, url: &Url) -> Result<Vec<u8>, std::io::Error> {
    let relative = url_utils::without_leading_slash(url.path());
    match resolve_local_path(webroot, relative) {
        Some(path) => fs::read(&path),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found locally for URL: {}", url),
        )),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p site2static local_file`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/local_file.rs
git commit -m "feat(site2static): port local_file with WebRoot support"
```

---

## Task 6: Port downloader (HTTP client)

**Files:**
- Create: `crates/site2static/src/downloader.rs`

Port from legacy. Simplify: remove auth, random user agent, cookie support (not needed for local dev server). Keep conditional GET.

- [ ] **Step 1: Write test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_html_content_type() {
        assert!(is_html("text/html; charset=utf-8"));
        assert!(!is_html("text/css"));
        assert!(!is_html("application/javascript"));
    }

    #[test]
    fn test_is_css_content_type() {
        assert!(is_css("text/css"));
        assert!(is_css("text/css; charset=utf-8"));
        assert!(!is_css("text/html"));
    }
}
```

- [ ] **Step 2: Implement downloader.rs**

```rust
use regex::Regex;
use reqwest::StatusCode;
use std::sync::LazyLock;
use url::Url;
use crate::response::{Response, ResponseData};

static DATA_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^.*(\b[a-z]+/[a-z\-+\.]+).*$"#).unwrap());
static CHARSET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^.*charset\s*=\s*["']?([^"'\s;]+).*$"#).unwrap());

fn is_html(content_type: &str) -> bool { content_type.contains("text/html") }
fn is_css(content_type: &str) -> bool { content_type.contains("text/css") }

pub struct Downloader {
    client: reqwest::blocking::Client,
    tries: usize,
}

impl Downloader {
    pub fn new(tries: usize) -> Self {
        let client = reqwest::blocking::ClientBuilder::new()
            .cookie_store(true)
            .user_agent("site2static/0.1")
            .danger_accept_invalid_certs(true) // local dev server
            .build()
            .expect("failed to build HTTP client");
        Self { client, tries }
    }

    pub fn get(&self, url: &Url) -> Result<Response, reqwest::Error> {
        self.get_conditional(url, None, None)
    }

    pub fn get_conditional(
        &self, url: &Url, etag: Option<&str>, last_modified: Option<&str>,
    ) -> Result<Response, reqwest::Error> {
        let mut last_err = None;
        for _ in 0..self.tries {
            match self.make_request(url, etag, last_modified) {
                Ok(resp) => return Ok(resp),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap())
    }

    fn make_request(
        &self, url: &Url, etag: Option<&str>, last_modified: Option<&str>,
    ) -> Result<Response, reqwest::Error> {
        let mut req = self.client.get(url.clone());
        if let Some(v) = etag { req = req.header("If-None-Match", v); }
        else if let Some(v) = last_modified { req = req.header("If-Modified-Since", v); }

        let resp = req.send()?;

        if resp.status() == StatusCode::NOT_MODIFIED {
            let resp_etag = resp.headers().get("ETag")
                .and_then(|v| v.to_str().ok()).map(String::from);
            let resp_lm = resp.headers().get("Last-Modified")
                .and_then(|v| v.to_str().ok()).map(String::from);
            return Ok(Response::not_modified(
                resp_etag.or_else(|| etag.map(String::from)),
                resp_lm.or_else(|| last_modified.map(String::from)),
            ));
        }

        let headers = resp.headers().clone();
        let etag_val = headers.get("ETag").and_then(|v| v.to_str().ok()).map(String::from);
        let lm_val = headers.get("Last-Modified").and_then(|v| v.to_str().ok()).map(String::from);

        let (data_type, charset) = match headers.get("content-type") {
            Some(ct) => {
                let ct_str = ct.to_str().unwrap_or("text/html");
                let dt = DATA_TYPE_RE.captures(ct_str)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_lowercase())
                    .unwrap_or_else(|| "text/html".into());
                let cs = CHARSET_RE.captures(ct_str)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_lowercase());
                (dt, cs)
            }
            None => ("text/html".into(), None),
        };

        let filename = if !is_html(&data_type) {
            headers.get("content-disposition")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.find('=').map(|i| s[i+1..].to_string()))
        } else { None };

        // Capture final URL after redirects for visited-set deduplication
        let final_url = Some(resp.url().clone());

        let mut raw = Vec::new();
        let mut resp = resp;
        resp.copy_to(&mut raw)?;

        let response_data = if is_html(&data_type) {
            ResponseData::Html(raw)
        } else if is_css(&data_type) {
            ResponseData::Css(raw)
        } else {
            ResponseData::Other(raw)
        };

        Ok(Response::with_metadata(response_data, filename, charset, etag_val, lm_val, final_url))
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p site2static downloader`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/site2static/src/downloader.rs
git commit -m "feat(site2static): port HTTP downloader with conditional GET"
```

---

## Task 7: Port CSS rewriting

**Files:**
- Create: `crates/site2static/src/css.rs`

Port from legacy `sitescrape/src/css.rs`. Replace `Urlz` with `url::Url` directly.

- [ ] **Step 1: Write tests**

Port tests from legacy css.rs — they cover URL extraction, quote preservation, and same-domain rewriting. See legacy file at `/home/avihs/workspace/appz-cli-legacy/crates/sitescrape/src/css.rs` lines 111-193 for the complete test suite to port.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static css`
Expected: FAIL

- [ ] **Step 3: Implement css.rs**

Port from legacy. Replace all `Urlz::new(x)` calls with `Url::parse(x)`. Use `LazyLock` for regex patterns to avoid recompilation per CSS file:

```rust
use std::sync::LazyLock;
use url::Url;

pub struct Css {
    pub css: String,
}

impl Css {
    pub fn new(css: &str) -> Self { Self { css: css.to_string() } }
    pub fn serialize(&self) -> String { self.css.clone() }

    /// Extract URLs from CSS and rewrite same-domain ones to absolute paths.
    pub fn find_urls_as_strings(&mut self, base_url: &Url) -> Vec<String> {
        let mut urls = Vec::new();
        let base_domain = base_url.domain().unwrap_or_default().to_string();

        let url_patterns = [
            r#"url\s*\(\s*['"]?([^'"]*?)['"]?\s*\)"#,
            r#"@import\s+url\s*\(\s*['"]?([^'"]*?)['"]?\s*\)"#,
            r#"@import\s+'([^']+)'"#,
            r#"@import\s+"([^"]+)""#,
            r#"@import\s+([^\s;]+)"#,
        ];

        for pattern in &url_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                for cap in regex.captures_iter(&self.css) {
                    if let Some(m) = cap.get(1) {
                        let url = m.as_str().trim();
                        if !url.is_empty() && !url.starts_with("data:") {
                            urls.push(url.to_string());
                        }
                    }
                }
                self.css = regex.replace_all(&self.css, |caps: &regex::Captures| {
                    if let Some(m) = caps.get(1) {
                        let original = m.as_str().trim();
                        if !original.is_empty() && !original.starts_with("data:") {
                            let resolved = Url::parse(original)
                                .or_else(|_| base_url.join(original));
                            if let Ok(resolved) = resolved {
                                if resolved.domain() == Some(&*base_domain) {
                                    if let Some(path) = resolved.path().strip_prefix('/') {
                                        let rel = format!("/{}", path);
                                        let full = caps.get(0).unwrap().as_str();
                                        return full.replace(original, &rel);
                                    }
                                }
                            }
                        }
                    }
                    caps.get(0).unwrap().as_str().to_string()
                }).to_string();
            }
        }
        urls
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p site2static css`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/css.rs
git commit -m "feat(site2static): port CSS url() rewriting"
```

---

## Task 8: Port DOM rewriting (lol_html)

**Files:**
- Create: `crates/site2static/src/dom.rs`

Port from legacy `sitescrape/src/dom.rs`. Replace `Urlz` with `Url::parse`. Keep `lol_html` element/text handlers and the hostname regex rewriting.

- [ ] **Step 1: Write tests**

Port all tests from legacy `dom.rs` lines 236-403: `test_dom_urls`, `test_css_url_extraction`, `test_css_url_replacement`, `test_hostname_rewriting`, `test_style_attribute_preserves_quotes`, `test_inline_style_preserves_quotes`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static dom`
Expected: FAIL

- [ ] **Step 3: Implement dom.rs**

Port from legacy. Key changes:
- Replace `Urlz::new(x)` with `Url::parse(x)`
- Replace `once_cell::sync::Lazy` with `std::sync::LazyLock`
- Replace `Arc<DashSet<String>>` with `Arc<dashmap::DashSet<String>>` (explicit import)
- Keep the `lol_html` rewriting logic, CSS_URL_REGEX, regex cache, and `rewrite_html_with_base_host` exactly as in legacy

The full implementation is at `/home/avihs/workspace/appz-cli-legacy/crates/sitescrape/src/dom.rs`. Port it with the substitutions above.

- [ ] **Step 4: Run tests**

Run: `cargo test -p site2static dom`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/dom.rs
git commit -m "feat(site2static): port lol_html DOM rewriting"
```

---

## Task 9: Implement sitemap discovery

**Files:**
- Create: `crates/site2static/src/sitemap.rs`

New module wrapping `crawl-core::process_sitemap()` with recursive fetching.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urlset_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/</loc></url>
            <url><loc>https://example.com/about/</loc></url>
        </urlset>"#;
        let result = crawl_core::process_sitemap(xml.to_string()).unwrap();
        assert!(!result.instructions.is_empty());
        let process_instr = result.instructions.iter()
            .find(|i| i.action == "process").unwrap();
        assert_eq!(process_instr.urls.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p site2static sitemap`
Expected: FAIL

- [ ] **Step 3: Implement sitemap.rs**

```rust
use url::Url;
use crate::downloader::Downloader;

/// Discover all page URLs from sitemaps. Returns empty vec on any failure.
pub fn discover_urls(origin: &Url, downloader: &Downloader) -> Vec<Url> {
    let sitemap_url = origin.join("/sitemap.xml").ok();
    let sitemap_url = match sitemap_url {
        Some(u) => u,
        None => return Vec::new(),
    };

    tracing::info!("Checking sitemap at {}", sitemap_url);
    let xml = match fetch_xml(downloader, &sitemap_url) {
        Some(xml) => xml,
        None => return Vec::new(),
    };

    let mut all_urls = Vec::new();
    process_recursive(downloader, &xml, &mut all_urls);

    tracing::info!("Sitemap discovery found {} page URLs", all_urls.len());
    all_urls
}

fn process_recursive(downloader: &Downloader, xml: &str, urls: &mut Vec<Url>) {
    let result = match crawl_core::process_sitemap(xml.to_string()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse sitemap: {}", e);
            return;
        }
    };

    for instr in &result.instructions {
        match instr.action.as_str() {
            "process" => {
                for url_str in &instr.urls {
                    if let Ok(url) = Url::parse(url_str) {
                        urls.push(url);
                    }
                }
            }
            "recurse" => {
                for url_str in &instr.urls {
                    if let Ok(url) = Url::parse(url_str) {
                        if let Some(child_xml) = fetch_xml(downloader, &url) {
                            process_recursive(downloader, &child_xml, urls);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn fetch_xml(downloader: &Downloader, url: &Url) -> Option<String> {
    match downloader.get(url) {
        Ok(resp) => {
            let bytes = match resp.data {
                crate::response::ResponseData::Html(b)
                | crate::response::ResponseData::Css(b)
                | crate::response::ResponseData::Other(b) => b,
            };
            // Handle gzip if Content-Encoding wasn't handled by reqwest
            if url.path().ends_with(".xml.gz") && bytes.starts_with(&[0x1f, 0x8b]) {
                use flate2::read::GzDecoder;
                use std::io::Read;
                let mut decoder = GzDecoder::new(&bytes[..]);
                let mut xml = String::new();
                decoder.read_to_string(&mut xml).ok()?;
                Some(xml)
            } else {
                String::from_utf8(bytes).ok()
            }
        }
        Err(e) => {
            tracing::debug!("Could not fetch sitemap {}: {}", url, e);
            None
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p site2static sitemap`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/sitemap.rs
git commit -m "feat(site2static): implement sitemap discovery with recursive fetching"
```

---

## Task 10: Implement core mirror loop

**Files:**
- Create: `crates/site2static/src/mirror.rs`

This is the main crawl engine. Port from legacy `sitescrape/src/scraper.rs` with these changes:
- Replace `Args` struct with `MirrorConfig`
- Use `crawl-core::filter_links` for page URL filtering (sync, no native feature)
- Use separate asset filter (same-domain + not visited + regex)
- Add sitemap pre-discovery (Phase 1)
- Replace `urlz::is_same_domain` with `url_utils::is_same_domain`
- Replace `unsafe String::from_utf8_unchecked` with safe `String::from_utf8_lossy` in charset detection
- Replace custom logging macros with `tracing`
- Track both original and final URL in visited set for redirect handling
- Return `MirrorResult` instead of printing stats

- [ ] **Step 1: Implement the mirror module**

This is the largest module (~600 lines). Port the core structure from legacy `scraper.rs`:

```rust
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam::channel::{Receiver, Sender, TryRecvError};
use dashmap::{DashMap, DashSet};
use regex::Regex;
use url::Url;

use crate::{MirrorConfig, MirrorError, MirrorResult, WebRoot};
use crate::css;
use crate::disk;
use crate::dom;
use crate::downloader::Downloader;
use crate::local_file;
use crate::metadata::{self, FileMetadata, MetadataCache};
use crate::response::{Response, ResponseData};
use crate::sitemap;
use crate::url_utils;

const MAX_EMPTY_RECEIVES: usize = 5;
const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(500);

pub fn run(config: MirrorConfig) -> Result<MirrorResult, MirrorError> {
    let start = Instant::now();

    // Validate output dir
    if let Err(e) = std::fs::create_dir_all(&config.output) {
        return Err(MirrorError::OutputNotWritable(config.output.clone()));
    }

    let downloader = Downloader::new(3);

    // Check origin is reachable
    if let Err(e) = downloader.get(&config.origin) {
        return Err(MirrorError::OriginUnreachable {
            url: config.origin.to_string(),
            message: e.to_string(),
        });
    }

    // Load incremental cache
    let metadata_cache = if !config.force {
        Arc::new(Mutex::new(metadata::load_metadata(&config.output)))
    } else {
        Arc::new(Mutex::new(MetadataCache::new()))
    };

    // Compile regex patterns
    let exclude_re = compile_regex_vec(&config.exclude_patterns);
    let include_re = compile_regex_vec(&config.include_patterns);

    let (tx, rx) = crossbeam::channel::unbounded::<(Url, i32)>();
    let visited: Arc<DashSet<String>> = Arc::new(DashSet::new());
    let path_map: Arc<DashMap<String, String>> = Arc::new(DashMap::new());
    let pages_crawled = Arc::new(AtomicU64::new(0));
    let assets_copied = Arc::new(AtomicU64::new(0));

    // Fetch robots.txt for crawl-core filter_links
    let robots_txt = fetch_robots_txt(&config.origin, &downloader);

    // Phase 1: Sitemap discovery
    let sitemap_urls = sitemap::discover_urls(&config.origin, &downloader);
    for url in &sitemap_urls {
        let path = local_file::url_to_path(url);
        if path_map.insert(url.to_string(), path).is_none() {
            let _ = tx.send((url.clone(), 0));
        }
    }

    // Seed with origin if not already queued
    if !visited.contains(config.origin.as_str()) {
        let path = local_file::url_to_path(&config.origin);
        path_map.insert(config.origin.to_string(), path);
        let _ = tx.send((config.origin.clone(), 0));
    }

    // Phase 2: Parallel crawl
    let workers = config.workers.max(1);
    let depth_limit = config.depth.unwrap_or(u32::MAX) as i32;

    crossbeam::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let rx = rx.clone();
            let downloader = &downloader;
            let config = &config;
            let visited = &visited;
            let path_map = &path_map;
            let metadata_cache = &metadata_cache;
            let pages_crawled = &pages_crawled;
            let assets_copied = &assets_copied;
            let exclude_re = &exclude_re;
            let include_re = &include_re;

            let robots_txt = &robots_txt;
            scope.spawn(move |_| {
                worker_loop(
                    &tx, rx, downloader, config, visited, path_map,
                    metadata_cache, pages_crawled, assets_copied,
                    exclude_re, include_re, depth_limit, robots_txt,
                );
            });
        }
    }).expect("worker thread panicked");

    // Phase 3: Finalize — always save metadata (even after forced crawl, so next run can be incremental)
    if let Ok(cache) = metadata_cache.lock() {
        if !cache.entries.is_empty() {
            metadata::save_metadata(&config.output, &cache);
        }
    }

    Ok(MirrorResult {
        pages_crawled: pages_crawled.load(Ordering::Relaxed),
        assets_copied: assets_copied.load(Ordering::Relaxed),
        output_dir: config.output.clone(),
        duration: start.elapsed(),
    })
}

/// Fetch robots.txt from origin, returning empty string if not found.
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

/// Build a FilterLinksCall for page URL filtering via crawl-core.
fn build_filter_call(
    page_urls: Vec<String>,
    config: &MirrorConfig,
    robots_txt: &str,
) -> crawl_core::FilterLinksCall {
    crawl_core::FilterLinksCall {
        links: page_urls,
        limit: None,
        max_depth: config.depth.unwrap_or(u32::MAX),
        base_url: config.origin.to_string(),
        initial_url: config.origin.to_string(),
        regex_on_full_url: false,
        excludes: config.exclude_patterns.clone(),
        includes: config.include_patterns.clone(),
        allow_backward_crawling: true,
        ignore_robots_txt: false,
        robots_txt: robots_txt.to_string(),
        allow_external_content_links: false,
        allow_subdomains: false,
    }
}

/// Simple asset URL filter — same-domain, not visited, passes regex patterns.
/// Used instead of crawl-core::filter_links which rejects file extensions.
fn should_copy_asset(
    url_str: &str,
    origin: &Url,
    visited: &DashSet<String>,
    exclude_re: &[Regex],
    include_re: &[Regex],
) -> bool {
    if visited.contains(url_str) { return false; }
    if !url_utils::is_same_domain(url_str, origin.as_str()) { return false; }
    if exclude_re.iter().any(|re| re.is_match(url_str)) { return false; }
    if !include_re.is_empty() && !include_re.iter().any(|re| re.is_match(url_str)) { return false; }
    true
}

/// Check if a URL points to an HTML page (vs. an asset).
fn is_html_url(url: &Url) -> bool {
    let path = url.path().to_lowercase();
    if path.ends_with(".html") || path.ends_with(".htm") { return true; }
    if path.is_empty() || path == "/" || path.ends_with('/') { return true; }
    if !path.contains('.') { return true; }
    false
}

/// Find charset from HTML meta tags. Safe version (no unsafe).
fn find_charset(data: &[u8], http_charset: Option<String>) -> Option<String> {
    static CHARSET_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(||
        Regex::new(r#"<meta.*charset\s*=\s*["']?([^"'\s;>]+).*>"#).unwrap()
    );
    let data_str = String::from_utf8_lossy(data);
    CHARSET_RE.captures(&data_str)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase())
        .or(http_charset)
}

fn compile_regex_vec(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

// worker_loop, handle_url, handle_html, handle_css — ported from legacy scraper.rs
// with the changes listed in the step description above.
```

The full implementation of `worker_loop`, `handle_url`, `handle_html`, `handle_css`, `is_html_url`, `find_charset`, `should_visit`, and helpers should be ported from legacy `scraper.rs` (at `/home/avihs/workspace/appz-cli-legacy/crates/sitescrape/src/scraper.rs`), applying the changes listed above. Key functions to port:

- `worker_loop` — from `process_queue_loop` (lines 585-633)
- `handle_url` — from `Scraper::handle_url` (lines 350-575)
- `handle_html` — from `Scraper::handle_html` (lines 219-300)
- `handle_css` — from `Scraper::handle_css` (lines 302-348)
- `is_html_url` — from `Scraper::is_html_url` (lines 771-791)
- `find_charset` — from `Scraper::find_charset` (lines 158-174), replace `unsafe` with `String::from_utf8_lossy`
- `should_visit` — from `Scraper::should_visit` (lines 699-723), simplified without `structopt` args
- `normalize_url_for_metadata` — from `Scraper::normalize_url_for_metadata` (lines 848-852)

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p site2static`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add crates/site2static/src/mirror.rs
git commit -m "feat(site2static): implement core mirror crawl loop"
```

---

## Task 11: Integration test

**Files:**
- Create: `crates/site2static/tests/integration.rs`

Spin up a local HTTP server, run `SiteMirror`, verify output.

- [ ] **Step 1: Write integration test**

```rust
use site2static::{MirrorConfig, SiteMirror, WebRoot};
use std::fs;
use std::path::PathBuf;
use std::thread;
use tempfile::TempDir;
use tiny_http::{Response, Server};
use url::Url;

fn serve_site(webroot: &std::path::Path) -> (String, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", server.server_addr().to_ip().unwrap());
    let webroot = webroot.to_path_buf();

    let handle = thread::spawn(move || {
        // Serve up to 50 requests then exit
        for _ in 0..50 {
            let request = match server.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(Some(r)) => r,
                _ => break,
            };
            let path = request.url().to_string();
            let file_path = if path == "/" {
                webroot.join("index.html")
            } else {
                webroot.join(path.trim_start_matches('/'))
            };
            if file_path.exists() {
                let content = fs::read(&file_path).unwrap();
                let ct = if file_path.extension().map(|e| e == "css").unwrap_or(false) {
                    "text/css"
                } else if file_path.extension().map(|e| e == "js").unwrap_or(false) {
                    "application/javascript"
                } else {
                    "text/html"
                };
                let resp = Response::from_data(content).with_header(
                    tiny_http::Header::from_bytes("Content-Type", ct).unwrap()
                );
                let _ = request.respond(resp);
            } else {
                let _ = request.respond(Response::from_string("404").with_status_code(404));
            }
        }
    });

    (addr, handle)
}

#[test]
fn test_basic_mirror() {
    // Set up test site
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    fs::write(site_dir.path().join("index.html"),
        r#"<html><head><link href="/style.css"></head><body><a href="/about/">About</a></body></html>"#
    ).unwrap();
    fs::create_dir(site_dir.path().join("about")).unwrap();
    fs::write(site_dir.path().join("about/index.html"),
        r#"<html><body><a href="/">Home</a></body></html>"#
    ).unwrap();
    fs::write(site_dir.path().join("style.css"), "body { color: red; }").unwrap();

    let (addr, _handle) = serve_site(site_dir.path());

    let config = MirrorConfig {
        origin: Url::parse(&addr).unwrap(),
        webroot: WebRoot::Direct(site_dir.path().to_path_buf()),
        output: output_dir.path().to_path_buf(),
        workers: 2,
        depth: None,
        force: true,
        exclude_patterns: vec![],
        include_patterns: vec![],
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run().unwrap();

    assert!(result.pages_crawled >= 2); // index + about
    assert!(output_dir.path().join("index.html").exists());
    assert!(output_dir.path().join("about/index.html").exists());
    assert!(output_dir.path().join("style.css").exists());
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p site2static --test integration`
Expected: PASS (may need debugging of mirror.rs)

- [ ] **Step 3: Iterate on mirror.rs until test passes**

Fix any issues found during integration testing.

- [ ] **Step 4: Commit**

```bash
git add crates/site2static/tests/integration.rs
git commit -m "test(site2static): add integration test with local HTTP server"
```

---

## Task 12: Integrate into blueprint

**Files:**
- Modify: `crates/blueprint/Cargo.toml`
- Modify: `crates/blueprint/src/static_export.rs`

- [ ] **Step 1: Add site2static dependency to blueprint**

Add to `crates/blueprint/Cargo.toml` under `[dependencies]`:

```toml
site2static = { path = "../site2static" }
url = { workspace = true }
```

- [ ] **Step 2: Replace StaticExporter implementation**

Replace the full content of `crates/blueprint/src/static_export.rs` with the new implementation that uses `SiteMirror` instead of Simply Static:

```rust
//! Static site export for CMS projects.
//!
//! Uses site2static to crawl a running local dev server and produce
//! a static HTML export suitable for deployment.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Default output directory name for static exports.
const DEFAULT_OUTPUT_DIR: &str = "dist";

/// Exports a CMS site as static HTML using site2static.
pub struct StaticExporter {
    project_path: PathBuf,
    runtime: Arc<dyn WordPressRuntime>,
}

impl StaticExporter {
    pub fn new(project_path: PathBuf, runtime: Arc<dyn WordPressRuntime>) -> Self {
        Self { project_path, runtime }
    }

    /// Run the full static export pipeline.
    ///
    /// 1. Resolve the site URL and webroot from the runtime
    /// 2. Run site2static to crawl and mirror the site
    ///
    /// Returns the host-side path to the output directory.
    pub fn export(&self, output_dir: Option<&Path>) -> Result<PathBuf, RuntimeError> {
        let host_output = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_path.join(DEFAULT_OUTPUT_DIR));

        let origin = self.runtime.site_url(&self.project_path);
        let webroot = self.resolve_webroot()?;

        let origin_url = url::Url::parse(&origin).map_err(|e| RuntimeError::CommandFailed {
            command: "site2static".into(),
            message: format!("invalid origin URL: {e}"),
        })?;

        let config = site2static::MirrorConfig {
            origin: origin_url,
            webroot: site2static::WebRoot::Direct(webroot),
            output: host_output.clone(),
            workers: 8,
            depth: None,
            force: false,
            exclude_patterns: vec![],
            include_patterns: vec![],
        };

        let mirror = site2static::SiteMirror::new(config);
        let result = mirror.run().map_err(|e| RuntimeError::CommandFailed {
            command: "site2static".into(),
            message: e.to_string(),
        })?;

        tracing::info!(
            "Exported {} pages, {} assets in {:.1}s",
            result.pages_crawled,
            result.assets_copied,
            result.duration.as_secs_f64()
        );

        Ok(host_output)
    }

    fn resolve_webroot(&self) -> Result<PathBuf, RuntimeError> {
        Ok(self.project_path.clone())
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p blueprint`
Expected: compiles

- [ ] **Step 4: Verify the full workspace compiles**

Run: `cargo check`
Expected: compiles (may have warnings about unused code in the old WordPress plugin, which is fine)

- [ ] **Step 5: Commit**

```bash
git add crates/blueprint/Cargo.toml crates/blueprint/src/static_export.rs
git commit -m "feat(blueprint): replace Simply Static with site2static"
```

---

## Task 13: Run all tests and verify

- [ ] **Step 1: Run site2static tests**

Run: `cargo test -p site2static`
Expected: all PASS

- [ ] **Step 2: Run blueprint tests (if any)**

Run: `cargo test -p blueprint`
Expected: all PASS

- [ ] **Step 3: Run full workspace check**

Run: `cargo check --workspace`
Expected: compiles with no errors

- [ ] **Step 4: Final commit if any fixes needed**

```bash
git add -u
git commit -m "fix(site2static): address test and compilation issues"
```
