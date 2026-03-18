# site2static Design Spec

## Overview

`site2static` is a new Rust crate (`crates/site2static`) that exports a running CMS site as static HTML by crawling it over HTTP and copying assets from the local filesystem. It replaces the current Simply Static WordPress plugin dependency with a self-contained solution.

**Primary use case:** `appz build` starts a local CMS dev server (DDEV, Playground, etc.), then `site2static` crawls it to produce a deployable static site in `dist/`.

**Scope:** The crate itself is CMS-agnostic by design — it only needs a URL and a webroot path. The v1 integration is WordPress-only via the existing `StaticExporter` in the blueprint crate. Supporting other CMSes (Statamic, Ghost, CraftCMS) requires adding new runtime implementations in the blueprint crate, which is out of scope for v1.

## Motivation

The current `StaticExporter` delegates to the Simply Static WordPress plugin via WP-CLI. This has several drawbacks:

- Requires installing/managing a third-party WordPress plugin
- WordPress-only — cannot export Statamic, Ghost, CraftCMS, etc.
- Plugin reinstalls on every build even when already active (fixed but symptomatic)
- PHP warnings from third-party plugin code pollute build output
- Export speed limited by Simply Static's PHP implementation

## Architecture

### Crate Structure

```
crates/site2static/
├── Cargo.toml
└── src/
    ├── lib.rs          # Public API: SiteMirror, MirrorConfig, MirrorResult
    ├── mirror.rs       # Core crawl loop (ported from legacy sitescrape/scraper.rs)
    ├── dom.rs          # lol_html URL rewriting (ported from legacy sitescrape/dom.rs)
    ├── css.rs          # CSS url() rewriting (ported from legacy sitescrape/css.rs)
    ├── downloader.rs   # HTTP client with conditional requests (ported from legacy)
    ├── local_file.rs   # Filesystem asset copy + URL-to-path mapping (ported from legacy local_file.rs + url_helper.rs)
    ├── disk.rs         # File comparison utilities for incremental checks (ported from legacy disk.rs)
    ├── metadata.rs     # Incremental crawl cache (ported from legacy)
    ├── sitemap.rs      # Sitemap pre-discovery with recursion (wraps crawl-core)
    ├── url_utils.rs    # URL utility functions (inlined from legacy urlz/ufo crate)
    └── response.rs     # Response types (ported from legacy)
```

### Dependencies

From the workspace:
- `crawl-core` (without `native` feature) — sync link filtering (`filter_links`), sitemap parsing (`process_sitemap`). The `native` feature is omitted so the sync API is compiled, avoiding the need for a tokio runtime in the crossbeam worker threads.

External:
- `lol_html` (latest 2.x) — fast streaming HTML rewriting
- `reqwest` (workspace, blocking + cookies features) — HTTP client, matching workspace version to avoid duplicate dependency trees
- `crossbeam` ^0.8 — worker channel queue
- `dashmap` 6.1.0 — concurrent visited set and path map
- `encoding_rs` ^0.8 — charset conversion for international content
- `regex` (workspace) — URL pattern matching
- `url` (workspace) — URL parsing
- `serde`, `serde_json` (workspace) — metadata cache serialization
- `filetime` 0.2 — preserve file modification times for incremental checks
- `pathdiff` ^0.2 — relative path calculation for URL rewriting
- `thiserror` (workspace) — error types
- `tracing` (workspace) — structured logging for progress and diagnostics
- `flate2` — gzip decompression for `.xml.gz` sitemaps (fallback when HTTP layer doesn't decompress)

### URL Utilities

The legacy crate depends on `urlz` (aliased from `crates/ufo`) for functions like `is_same_domain()`, `without_leading_slash()`, `without_trailing_slash()`, and URL normalization. These are small, focused utility functions (~93 lines in the legacy `url_helper.rs` + relevant parts of `ufo`). Rather than adding `urlz` as a workspace dependency, they will be inlined into `url_utils.rs` within the crate. This avoids an external dependency for trivial string operations.

### Public API

```rust
pub struct SiteMirror { /* internal state */ }

pub struct MirrorConfig {
    /// URL of the running site to crawl (e.g. http://localhost:8080)
    pub origin: Url,
    /// Local webroot for filesystem asset copy
    pub webroot: WebRoot,
    /// Output directory for the static export
    pub output: PathBuf,
    /// Number of concurrent workers (default: 8)
    pub workers: usize,
    /// Max crawl depth (None = unlimited, maps to u32::MAX for crawl-core)
    pub depth: Option<u32>,
    /// Force full re-crawl (ignore incremental cache)
    pub force: bool,
    /// URL exclude patterns (regex)
    pub exclude_patterns: Vec<String>,
    /// URL include patterns (regex)
    pub include_patterns: Vec<String>,
}

pub enum WebRoot {
    /// Single directory — URL paths map directly (e.g. WordPress project root)
    Direct(PathBuf),
    /// Multiple search paths — try each in order (e.g. Ghost themes + content)
    Search(Vec<PathBuf>),
}

pub struct MirrorResult {
    pub pages_crawled: u64,
    pub assets_copied: u64,
    pub output_dir: PathBuf,
    pub duration: Duration,
}

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

impl SiteMirror {
    pub fn new(config: MirrorConfig) -> Self;
    /// Consume the mirror and run the export. Single-use by design.
    pub fn run(self) -> Result<MirrorResult, MirrorError>;
}
```

Note: `run(self)` consumes the struct to make single-use intent clear. The metadata cache is loaded in `new()` and saved at the end of `run()`.

## Crawl Pipeline

### Phase 1: Sitemap Discovery (optional)

Before crawling, attempt to fetch `<origin>/sitemap.xml` and `<origin>/robots.txt`.

- If a sitemap exists, parse it via `crawl-core::process_sitemap()`
- `process_sitemap` returns `SitemapProcessingResult` with `instructions` — each instruction has an `action` field:
  - `"process"` — URLs to crawl directly → add to URL queue (note: `process_sitemap` filters out file extensions like `.png`, `.jpg` from "process" URLs — this is acceptable since asset discovery happens via HTML link extraction in Phase 2)
  - `"recurse"` — nested sitemap URLs (`.xml` or `.xml.gz`) → fetch each, call `process_sitemap` again, repeat
- For `.xml.gz` sitemaps: rely on reqwest's built-in `gzip` feature to decompress transparently at the HTTP layer (Content-Encoding: gzip). If a sitemap URL ends in `.xml.gz` but the server does not send Content-Encoding, decompress manually with `flate2` before parsing.
- The `sitemap.rs` wrapper handles this recursion loop (fetch → parse → recurse) and returns a flat `Vec<Url>` to the caller
- If no sitemap exists or fetching fails, skip silently — Phase 2 handles everything via link discovery

### Phase 2: Parallel Crawl

Same proven architecture as legacy `sitescrape` — producer-consumer queue with crossbeam channels:

```
                    ┌─────────────────────┐
                    │  URL Queue (crossbeam│
                    │  unbounded channel)  │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
         ┌────v────┐     ┌────v────┐     ┌────v────┐
         │Worker 1 │     │Worker 2 │     │  ... N  │
         └────┬────┘     └────┬────┘     └────┬────┘
              │
     ┌────────┴────────┐
     │ is_html_url()?  │
     ├─ YES: HTTP GET  │
     │   → conditional │
     │     request     │
     │   → charset     │
     │     detection   │
     │   → lol_html    │
     │     rewrite     │
     │   → extract     │
     │     new URLs    │
     │   → write file  │
     ├─ NO: fs copy    │
     │   from webroot  │
     │   → CSS url()   │
     │     rewrite if  │
     │     .css file   │
     └─────────────────┘

Shared state (lock-free):
  • DashSet<String>          — visited URLs
  • DashMap<String, String>  — URL → filesystem path mapping
  • Arc<Mutex<MetadataCache>> — ETag/Last-Modified cache
  • AtomicU64                — pages_crawled counter
  • AtomicU64                — assets_copied counter
```

**HTML pages** (fetched over HTTP):
1. Conditional request using cached ETag/Last-Modified headers (304 = skip)
2. Charset detection from `<meta>` tags, convert to UTF-8 via `encoding_rs` (note: the legacy `find_charset` uses `unsafe String::from_utf8_unchecked` on raw bytes — this must be replaced with safe `String::from_utf8_lossy` during porting)
3. `lol_html` streaming rewrite: convert absolute same-domain URLs to relative paths
4. Extract links from `<a href>`, `<img src>`, `<script src>`, `<link href>`, `<source src>`, `<video src>`, `<audio src>`, `<iframe src>`, `<embed src>`, `<object data>`, `<picture src>`
5. Filter discovered **page URLs** through `crawl-core::filter_links` (depth, regex, robots.txt) — see note below on asset URL handling
6. Queue new URLs for processing
7. Write rewritten HTML to output directory

**Link filtering and asset URL handling:**

`crawl-core::filter_links` filters out file extensions (`.css`, `.js`, `.png`, `.jpg`, etc.) via the `FILE_TYPE` denial reason — it is designed for web crawlers that only want pages. For `site2static`, which needs to discover and copy all assets, the approach is:

- Use `crawl-core::filter_links` **only for page URLs** (URLs without file extensions, or with `.html`/`.htm` extensions) to get depth limiting, regex patterns, and robots.txt enforcement
- Handle asset URLs (CSS, JS, images, fonts, etc.) with a **separate, simpler filter** in `mirror.rs` that only checks: same-domain, not already visited, and passes exclude/include regex patterns. No depth limiting or robots.txt needed for assets — if an HTML page references them, they should be copied.

This gives us the best of both worlds: structured filtering from `crawl-core` for page discovery, and permissive handling for assets.

**`FilterLinksCall` parameter mapping:**

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `max_depth` | `config.depth.unwrap_or(u32::MAX)` | `None` = unlimited |
| `base_url` | `config.origin` | Same-domain enforcement |
| `initial_url` | `config.origin` | Start from site root |
| `allow_backward_crawling` | `true` | Full-site export needs all paths |
| `regex_on_full_url` | `false` | Match path only |
| `allow_external_content_links` | `false` | Same-origin mirror |
| `allow_subdomains` | `false` | Single-site export |
| `ignore_robots_txt` | `false` | Respect robots.txt by default |

**Asset files** (copied from filesystem):
1. Map URL path to local filesystem path via `WebRoot` configuration
2. If local file not found, fall back to HTTP download
3. Fast incremental check via `disk.rs`: compare size + mtime of source vs existing output file
4. Copy file to output directory, preserving mtime
5. For `.css` files: rewrite `url()` references to relative paths using regex

**URL rewriting (lol_html):**
- Element attributes: `img[src]`, `script[src]`, `source[src]`, `video[src]`, `audio[src]`, `iframe[src]`, `embed[src]`, `object[data]`, `picture[src]`, `link[href]`, `a[href]`
- Inline styles: `*[style]` — extract and rewrite `url()` patterns
- Style blocks: `<style>` text content — rewrite `url()` patterns
- Only rewrites same-domain URLs; external URLs left unchanged
- Converts absolute URLs to relative paths using `pathdiff`

**CSS url() rewriting (regex):**
- Pattern: `url(['"]?<path>['"]?)`
- Rewrites same-domain references to relative paths
- Preserves quote style (single, double, or none)

**HTTP redirect handling:**
- `reqwest`'s blocking client follows redirects by default (up to 10 hops)
- The **final URL** (after redirects) is used for the visited set and path mapping, not the original URL
- Both the original URL and the final URL are added to the visited set to prevent re-crawling via either path
- This handles common CMS redirects: `/about` → `/about/`, HTTP → HTTPS, non-www → www

### Phase 3: Finalize

1. Save metadata cache to `<output>/.site2static-metadata.json`
2. Return `MirrorResult` with stats

## Incremental Crawling

Two mechanisms, matching legacy behavior:

**HTML pages — HTTP conditional requests:**
- On first crawl: store `ETag` and `Last-Modified` response headers in metadata cache
- On subsequent crawls: send `If-None-Match` / `If-Modified-Since` headers
- 304 Not Modified → skip download, keep existing file
- Cache keyed by normalized URL (query strings and fragments stripped)

**Asset files — filesystem comparison (in `disk.rs`):**
- Compare source file (in webroot) with destination file (in output)
- Fast path: size differs → copy
- Same size: treat as unchanged (mtime comparison for extra safety)
- `force: true` → skip all checks, re-copy everything

**Cache file format:**
```json
{
  "https://site.local/": {
    "etag": "W/\"33a64df...\"",
    "last_modified": "Wed, 21 Oct 2023 07:28:00 GMT",
    "file_hash": null
  }
}
```

Note: The cache filename changes from `.sitescrape-metadata.json` to `.site2static-metadata.json`. There is no backward compatibility concern since this is a new crate replacing a completely different export mechanism (Simply Static).

## Logging and Progress

The crate uses `tracing` (workspace) for structured logging:

- `info!` — phase transitions ("Discovering sitemap...", "Crawling 1,151 pages..."), final stats
- `debug!` — individual page fetches, asset copies, cache hits
- `warn!` — non-fatal errors (failed page fetch, missing local file, charset issues)
- `error!` — fatal errors before returning `MirrorError`

The caller (blueprint's `StaticExporter`) controls the subscriber. For `appz build`, the existing UI layer handles rendering tracing output to the terminal. No custom progress bars or spinners in the crate itself — that's the caller's responsibility.

## Integration with Blueprint

### Current Flow (removed)

```
build.rs → StaticExporter
  → install_simply_static()      # WP-CLI plugin install
  → install_appz_plugin()        # WP-CLI plugin install
  → run_build()                  # wp appz build --output-dir=<path>
```

### New Flow

```
build.rs → StaticExporter
  → resolve webroot from runtime
  → SiteMirror::new(config).run()
```

**Changes to `blueprint/src/static_export.rs`:**

```rust
pub fn export(&self, output_dir: Option<&Path>) -> Result<PathBuf, RuntimeError> {
    let host_output = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| self.project_path.join("dist"));

    let origin = self.runtime.site_url(&self.project_path);
    let webroot = self.resolve_webroot()?;

    let config = MirrorConfig {
        origin: Url::parse(&origin).map_err(|e| RuntimeError::CommandFailed {
            command: "site2static".into(),
            message: format!("invalid origin URL: {e}"),
        })?,
        webroot: WebRoot::Direct(webroot),
        output: host_output.clone(),
        workers: 8,
        depth: None, // unlimited
        force: false,
        exclude_patterns: vec![],
        include_patterns: vec![],
    };

    let mirror = SiteMirror::new(config);
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
    // For DDEV: the project root IS the WordPress webroot
    // For Playground: similar — project root contains wp-content/
    // Future: Statamic → project_path/public, Ghost → project_path/content
    Ok(self.project_path.clone())
}
```

**What gets removed:**
- `install_simply_static()` — no longer needed
- `install_appz_plugin()` — no longer needed
- `run_build()` — replaced by `SiteMirror::run()`
- The Simply Static plugin dependency is eliminated

**WordPress plugin status:**
The `packages/wordpress-plugin/` (Appz Static Site Generator) is deprecated after this change. It provided WP-CLI access to Simply Static, which is no longer used. The plugin files can be removed or kept for users who independently use Simply Static outside of `appz build`. This decision is deferred — for now, the plugin is simply no longer installed or invoked by `StaticExporter`.

## Webroot Resolution

The filesystem asset copy maps URL paths to local file paths via the `WebRoot` configuration:

**`WebRoot::Direct(path)`** — URL paths map directly:
1. Strip the origin from the URL to get the path component
2. Join path component with the webroot directory
3. If file exists, copy it; otherwise fall back to HTTP download

**`WebRoot::Search(paths)`** — try each directory in order:
1. Strip the origin from the URL to get the path component
2. Try joining with each search path
3. Use the first match that exists on disk
4. If none found, fall back to HTTP download

**Per-runtime webroot configuration:**

| Runtime | WebRoot | Example |
|---------|---------|---------|
| WordPress / DDEV | `Direct(project_root)` | `/path/to/project` |
| Statamic / Valet | `Direct(project_root/public)` | `/path/to/project/public` |
| Ghost | `Search([content/themes/<theme>, content])` | Multiple search paths |

For v1, only `Direct(PathBuf)` is needed (WordPress/DDEV). `Search` is an extension point for future CMS support.

## Output Format

```
dist/
├── index.html                      # / → index.html
├── about/
│   └── index.html                  # /about/ → about/index.html
├── blog/
│   ├── index.html
│   └── my-post/
│       └── index.html
├── wp-content/
│   ├── themes/theme-name/
│   │   ├── style.css               # url() paths rewritten to relative
│   │   └── assets/
│   │       └── app.js
│   └── uploads/
│       └── 2024/
│           └── photo.jpg
└── .site2static-metadata.json      # Incremental cache (excluded from deploy)
```

**Path mapping rules:**
- Root `/` → `index.html`
- Directory-like URLs `/path/` → `path/index.html`
- Files keep original extension and directory structure
- Fragment identifiers stripped from file paths
- All internal URLs rewritten to relative paths

## Error Handling

**Fatal errors (abort the crawl):**
- Origin URL unreachable (initial connectivity check)
- Output directory not writable

**Non-fatal errors (log and continue):**
- Individual page fetch failures
- Individual asset copy failures
- Charset conversion issues
- URL parse errors on discovered links
- Sitemap fetch/parse failures

This matches the legacy behavior where resilience is preferred over strictness — a single broken page shouldn't fail the entire export.

## Code Provenance

| Module | Source | Changes |
|--------|--------|---------|
| `mirror.rs` | legacy `sitescrape/scraper.rs` (1140 lines) | Replace hand-rolled link filtering with `crawl-core::filter_links` (sync, no `native` feature). Add sitemap pre-discovery. Remove `structopt`/CLI concerns. Remove `appz_common` dependency. |
| `dom.rs` | legacy `sitescrape/dom.rs` (403 lines) | Minimal changes — `lol_html` rewriting logic is correct and fast. Replace `urlz` calls with `url_utils`. |
| `css.rs` | legacy `sitescrape/css.rs` (193 lines) | Minimal changes — regex-based CSS url() rewriting. Replace `urlz` calls with `url_utils`. |
| `downloader.rs` | legacy `sitescrape/downloader.rs` (302 lines) | Keep conditional request support. Remove random user agent rotation (not needed for local dev server). Update to workspace `reqwest` version. |
| `local_file.rs` | legacy `sitescrape/local_file.rs` + `url_helper.rs` (209 lines combined) | Adapt to `WebRoot` enum for path resolution. Merge URL-to-path mapping from `url_helper.rs`. |
| `disk.rs` | legacy `sitescrape/disk.rs` (414 lines) | Port file comparison utilities (`files_differ_fast`), mtime preservation. Remove Elementor-specific bundle copy logic (not needed without Simply Static). |
| `metadata.rs` | legacy `sitescrape/metadata.rs` (202 lines) | Rename cache file to `.site2static-metadata.json`. Otherwise minimal changes. |
| `sitemap.rs` | New (~100 lines) | Wraps `crawl-core::process_sitemap()`. Handles recursion loop: fetch sitemap XML → parse → for "recurse" instructions, fetch child sitemaps and repeat → collect all "process" URLs into flat `Vec<Url>`. |
| `url_utils.rs` | Inlined from legacy `ufo/urlz` crate (~100 lines) | Only the functions actually used: `is_same_domain()`, `without_leading_slash()`, `without_trailing_slash()`, `normalize_url()`. |
| `response.rs` | legacy `sitescrape/response.rs` (60 lines) | Minimal changes. |

**Total estimated size:** ~2,800 lines (down from 3,274 in legacy due to removing CLI/logging boilerplate and using `crawl-core` for page filtering).

## Testing Strategy

**Unit tests:**
- `dom.rs` — URL rewriting correctness: absolute → relative, external URLs preserved, inline style rewriting, `<style>` block rewriting. Port legacy test fixtures (`with-style-handling.html`, `without-style-handling.html`) if available.
- `css.rs` — CSS `url()` rewriting: various quote styles, relative/absolute URLs, `@import` statements
- `url_utils.rs` — domain comparison, path normalization, edge cases
- `local_file.rs` — URL-to-path mapping, `WebRoot::Direct` resolution (v1 only; `WebRoot::Search` tested when implemented)
- `metadata.rs` — cache serialization/deserialization round-trip

**Integration tests:**
- Spin up a local HTTP server (using `tiny_http` or `axum` as a dev-dependency) serving a small static site with known structure
- Run `SiteMirror` against it and verify:
  - All pages discovered and written
  - Assets copied from filesystem (not downloaded)
  - URLs rewritten to relative paths
  - Incremental re-run skips unchanged pages (304)
  - `force: true` re-crawls everything
  - CSS `url()` references rewritten correctly

**Not in scope for v1:** End-to-end tests with a real WordPress/DDEV instance (too slow, environment-dependent). The integration test with a mock HTTP server covers the crawl logic; the blueprint integration is a thin wrapper.

## Performance Characteristics

- **Sitemap pre-discovery:** Know all URLs upfront, skip link-following overhead for known pages
- **Filesystem asset copy:** Zero HTTP overhead for CSS/JS/images/fonts — orders of magnitude faster than downloading
- **Configurable worker count:** 8 concurrent HTTP fetches by default (tunable)
- **Incremental builds:** Only re-fetch changed HTML pages; only re-copy changed assets
- **Streaming HTML rewrite:** `lol_html` processes HTML in a single pass without building a DOM tree
- **Lock-free concurrency:** `DashMap`/`DashSet` for visited URLs and path mapping — no contention between workers

## Future Extensions (not in v1)

- `WebRoot::Search` for Ghost/Statamic multi-directory webroots
- Asset optimization pipeline (minification, image compression) as a post-processing step
- Progress callback/events for UI integration
- Configurable user agent and cookie support for authenticated pages
- New runtime trait implementations in blueprint for Statamic, Ghost, CraftCMS
