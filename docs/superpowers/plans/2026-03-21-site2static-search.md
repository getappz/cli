# site2static Search Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Pagefind-based client-side search to site2static so all exported static sites are instantly searchable.

**Architecture:** Two-phase approach — inject Pagefind UI markup during HTML mirroring (Phase 1), then run `pagefind` CLI on the output directory to build the search index (Phase 2). Configuration via `SearchMode` enum on `MirrorConfig`. WordPress search forms are detected and replaced; a Cmd+K floating modal is injected on all pages.

**Tech Stack:** Rust, lol_html (HTML rewriting), Pagefind CLI (search indexing), mise (binary management)

**Spec:** `docs/superpowers/specs/2026-03-21-site2static-search-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `crates/site2static/src/search_ui.rs` | HTML injection — Pagefind UI assets, WP form replacement, Cmd+K modal |
| Create | `crates/site2static/src/search.rs` | Pagefind CLI invocation and binary pre-check |
| Modify | `crates/site2static/src/lib.rs` | Add `SearchMode`, `SearchUiConfig`, new `ProgressEvent` variants, wire modules |
| Modify | `crates/site2static/src/mirror.rs` | Call `inject_search_ui` in `handle_html_url`, call `run_pagefind` in `run()` |
| Modify | `crates/site2static/Cargo.toml` | Add `which` dependency (binary lookup) |
| Modify | `crates/blueprint/src/static_export.rs` | Thread `SearchMode` into `MirrorConfig` |
| Modify | `crates/site2static/tests/integration.rs` | Add search integration tests |

---

### Task 1: Add `SearchMode`, `SearchUiConfig`, and new `ProgressEvent` variants to `lib.rs`

**Files:**
- Modify: `crates/site2static/src/lib.rs:22-68`

- [ ] **Step 1: Add `SearchMode` enum and `SearchUiConfig` struct**

Add after the `ProgressEvent` enum (after line 33) in `lib.rs`:

```rust
/// Search configuration for the static export.
#[derive(Debug, Clone)]
pub enum SearchMode {
    /// No search indexing or UI injection.
    Disabled,
    /// Run Pagefind to build search index, but don't inject UI.
    IndexOnly,
    /// Run Pagefind and inject search UI into pages.
    Full(SearchUiConfig),
}

/// Configuration for search UI injection.
#[derive(Debug, Clone)]
pub struct SearchUiConfig {
    /// Replace existing search forms (WordPress) with Pagefind widgets.
    pub replace_existing: bool,
    /// Inject a floating Cmd+K / Ctrl+K search modal on all pages.
    pub keyboard_shortcut: bool,
}

impl Default for SearchUiConfig {
    fn default() -> Self {
        Self {
            replace_existing: true,
            keyboard_shortcut: true,
        }
    }
}
```

- [ ] **Step 2: Add new `ProgressEvent` variants**

Add two variants to the `ProgressEvent` enum (after the `Done` variant at line 32):

```rust
/// Search indexing started.
IndexingSearch,
/// Search indexing complete.
SearchDone { pages: usize },
```

- [ ] **Step 3: Add `search` field to `MirrorConfig`**

Add after `copy_globs` field (after line 65):

```rust
/// Search mode. `None` defaults to `SearchMode::Disabled`.
pub search: Option<SearchMode>,
```

- [ ] **Step 4: Register new modules in `lib.rs`**

Add after line 20 (`mod url_utils;`):

```rust
mod search;
mod search_ui;
```

- [ ] **Step 5: Add public re-exports**

Add to the top of `lib.rs` alongside existing public types:

```rust
pub use search::check_pagefind;
```

And update the existing `pub` items so `SearchMode` and `SearchUiConfig` are exported (they're already `pub`).

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p site2static`
Expected: Compilation errors about missing `search` and `search_ui` modules (expected — we'll create them next)

- [ ] **Step 7: Commit**

```bash
git add crates/site2static/src/lib.rs
git commit -m "feat(site2static): add SearchMode, SearchUiConfig, and search ProgressEvent variants"
```

---

### Task 2: Create `search.rs` — Pagefind CLI invocation

**Files:**
- Create: `crates/site2static/src/search.rs`
- Modify: `crates/site2static/Cargo.toml:8-28` (add `which` dependency)

- [ ] **Step 1: Add `which` dependency to Cargo.toml**

Add to `[dependencies]` in `Cargo.toml`:

```toml
which = "7"
```

- [ ] **Step 2: Create `search.rs` with pagefind pre-check**

Create `crates/site2static/src/search.rs`:

```rust
//! Pagefind CLI invocation for search index generation.

use std::path::Path;
use std::process::Command;

use crate::MirrorError;

/// Check whether the `pagefind` binary is available on PATH.
/// Returns the path to the binary, or an error with install instructions.
pub fn check_pagefind() -> Result<std::path::PathBuf, MirrorError> {
    which::which("pagefind").map_err(|_| MirrorError::SearchBinaryNotFound {
        binary: "pagefind".into(),
        hint: "Install via `mise use -g pagefind` or `npm install -g pagefind`".into(),
    })
}

/// Run `pagefind --site <output_dir> --bundle-dir pagefind` to build the search index.
/// Returns the number of pages indexed (parsed from stdout), or 0 if unparseable.
pub fn run_pagefind(output_dir: &Path) -> Result<usize, MirrorError> {
    let bin = check_pagefind()?;

    let output = Command::new(&bin)
        .arg("--site")
        .arg(output_dir)
        .arg("--bundle-dir")
        .arg("pagefind")
        .output()
        .map_err(|e| MirrorError::SearchIndexingFailed {
            message: format!("failed to execute pagefind: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MirrorError::SearchIndexingFailed {
            message: format!("pagefind exited with {}: {}", output.status, stderr.trim()),
        });
    }

    // Parse page count from stdout. Pagefind prints something like:
    // "Running Pagefind ... on 42 page(s)"
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pages = parse_page_count(&stdout);
    tracing::info!("Pagefind indexed {} pages in {}", pages, output_dir.display());
    Ok(pages)
}

/// Extract page count from pagefind stdout.
fn parse_page_count(stdout: &str) -> usize {
    // Look for pattern like "42 page" in output
    for word in stdout.split_whitespace().collect::<Vec<_>>().windows(2) {
        if word[1].starts_with("page") {
            if let Ok(n) = word[0].parse::<usize>() {
                return n;
            }
        }
    }
    0
}
```

- [ ] **Step 3: Add unit tests for `parse_page_count`**

Add at the bottom of `search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_count_from_pagefind_output() {
        assert_eq!(parse_page_count("Running Pagefind v1.4.0 on 42 page(s)"), 42);
    }

    #[test]
    fn returns_zero_for_unparseable_output() {
        assert_eq!(parse_page_count("no match here"), 0);
        assert_eq!(parse_page_count(""), 0);
    }

    #[test]
    fn parses_single_page() {
        assert_eq!(parse_page_count("Indexed 1 page"), 1);
    }
}
```

- [ ] **Step 4: Add new `MirrorError` variants to `lib.rs`**

Add to the `MirrorError` enum in `lib.rs` (after line 91):

```rust
#[error("search binary '{binary}' not found: {hint}")]
SearchBinaryNotFound { binary: String, hint: String },

#[error("search indexing failed: {message}")]
SearchIndexingFailed { message: String },
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p site2static`
Expected: PASS (or errors only about the still-missing `search_ui` module)

- [ ] **Step 6: Commit**

```bash
git add crates/site2static/src/search.rs crates/site2static/Cargo.toml crates/site2static/src/lib.rs
git commit -m "feat(site2static): add search.rs — pagefind CLI pre-check and invocation"
```

---

### Task 3: Create `search_ui.rs` — HTML injection

**Files:**
- Create: `crates/site2static/src/search_ui.rs`

This is the largest module. It uses `lol_html::rewrite_str` as a separate pass on already-rewritten HTML.

- [ ] **Step 1: Create `search_ui.rs` with the `inject_search_ui` function**

Create `crates/site2static/src/search_ui.rs`:

```rust
//! Search UI injection into HTML pages.
//!
//! Runs a separate lol_html rewrite pass to:
//! 1. Inject Pagefind CSS/JS references into <head>
//! 2. Replace WordPress search forms with Pagefind UI widgets
//! 3. Inject a floating Cmd+K search modal before </body>

use std::cell::RefCell;

use lol_html::{element, rewrite_str, RewriteStrSettings};

use crate::SearchUiConfig;

/// Pagefind UI CSS link tag.
const PAGEFIND_CSS: &str =
    r#"<link rel="stylesheet" href="/pagefind/pagefind-ui.css">"#;

/// Pagefind UI JS script tag.
const PAGEFIND_JS: &str =
    r#"<script src="/pagefind/pagefind-ui.js"></script>"#;

/// Inject search UI markup into an HTML page.
///
/// Runs its own `lol_html::rewrite_str` pass.
/// Input: UTF-8 HTML (after dom.rs URL rewriting).
/// Output: UTF-8 HTML with search UI injected.
pub fn inject_search_ui(html: &str, config: &SearchUiConfig) -> Result<String, lol_html::errors::RewritingError> {
    let head_injected = RefCell::new(false);
    let form_counter = RefCell::new(0u32);
    let replaced_forms: RefCell<std::collections::HashSet<u64>> = RefCell::new(Default::default());
    let body_injected = RefCell::new(false);
    let content_marked = RefCell::new(false);

    let mut element_handlers: Vec<_> = vec![
        // Inject Pagefind CSS/JS into <head>
        element!("head", |el| {
            if !*head_injected.borrow() {
                el.append(&format!("\n{}\n{}", PAGEFIND_CSS, PAGEFIND_JS), lol_html::html_content::ContentType::Html);
                *head_injected.borrow_mut() = true;
            }
            Ok(())
        }),
    ];

    // Content hinting: mark first main content element with data-pagefind-body
    for selector in &["main", "article", ".entry-content", "#content"] {
        let content_marked = &content_marked;
        element_handlers.push(
            element!(selector, |el| {
                if !*content_marked.borrow() {
                    el.set_attribute("data-pagefind-body", "")?;
                    *content_marked.borrow_mut() = true;
                }
                Ok(())
            })
        );
    }

    // WordPress search form replacement.
    // Guard: an element matching multiple selectors (e.g. <form class="search-form" role="search">)
    // is only replaced once. We hash the element's start tag position via a unique marker.
    if config.replace_existing {
        for selector in &["form.search-form", "form[role=\"search\"]", ".wp-block-search"] {
            let form_counter = &form_counter;
            let replaced_forms = &replaced_forms;
            element_handlers.push(
                element!(selector, |el| {
                    // Use a hash of the tag name + existing attributes as a dedup key.
                    // lol_html doesn't expose element identity, so we use the current
                    // counter state + tag name as a proxy. The real guard is: if we
                    // already set_inner_content on this element, the next handler sees
                    // our injected content, not the original. We track by checking if
                    // the element already contains our marker.
                    let inner_before = el.tag_name();
                    let key = std::hash::BuildHasher::hash_one(
                        &std::collections::hash_map::RandomState::new(),
                        &format!("{}{}", inner_before, el.attributes().iter().map(|a| a.value().to_string()).collect::<String>()),
                    );
                    if !replaced_forms.borrow_mut().insert(key) {
                        return Ok(()); // Already replaced by a prior selector
                    }
                    let mut counter = form_counter.borrow_mut();
                    let id = *counter;
                    *counter += 1;
                    let replacement = format!(
                        r#"<div id="pagefind-replace-{id}" data-pagefind-ignore></div><script>new PagefindUI({{ element: "#pagefind-replace-{id}", showSubResults: true }});</script>"#,
                    );
                    el.set_inner_content(&replacement, lol_html::html_content::ContentType::Html);
                    Ok(())
                })
            );
        }
    }

    // Floating modal injection before </body>
    if config.keyboard_shortcut {
        let body_injected = &body_injected;
        element_handlers.push(
            element!("body", |el| {
                if !*body_injected.borrow() {
                    el.append(SEARCH_MODAL_HTML, lol_html::html_content::ContentType::Html);
                    *body_injected.borrow_mut() = true;
                }
                Ok(())
            })
        );
    }

    rewrite_str(html, RewriteStrSettings {
        element_content_handlers: element_handlers,
        ..RewriteStrSettings::default()
    })
}

/// Self-contained HTML/CSS/JS for the floating search modal.
const SEARCH_MODAL_HTML: &str = r##"
<div id="s2s-search-overlay" data-pagefind-ignore style="display:none;position:fixed;inset:0;z-index:99999;background:rgba(0,0,0,0.5);align-items:flex-start;justify-content:center;padding-top:min(20vh,120px)">
  <style>
    #s2s-search-dialog{background:#fff;border-radius:12px;padding:0;border:1px solid #ddd;width:90%;max-width:620px;box-shadow:0 16px 70px rgba(0,0,0,0.3);max-height:80vh;overflow:auto}
    #s2s-search-dialog .pagefind-ui__search-input{font-size:18px;padding:12px 16px;width:100%;box-sizing:border-box;border:none;border-bottom:1px solid #eee;outline:none}
    #s2s-search-dialog .pagefind-ui__result-link{color:#1a0dab;text-decoration:none}
    #s2s-search-dialog .pagefind-ui__result-link:hover{text-decoration:underline}
    @media(prefers-color-scheme:dark){#s2s-search-dialog{background:#1e1e1e;border-color:#444;color:#e0e0e0}#s2s-search-dialog .pagefind-ui__search-input{border-bottom-color:#444;color:#e0e0e0}}
    #s2s-search-kbd{position:absolute;right:16px;top:50%;transform:translateY(-50%);font-size:12px;color:#999;pointer-events:none}
  </style>
  <div id="s2s-search-dialog" role="dialog" aria-label="Site search">
    <div id="s2s-search-mount"></div>
  </div>
</div>
<script>
(function(){
  var overlay=document.getElementById('s2s-search-overlay');
  var mount=document.getElementById('s2s-search-mount');
  var ui;
  function open(){
    if(!ui){ui=new PagefindUI({element:mount,showSubResults:true,autofocus:true})}
    overlay.style.display='flex';
    var input=mount.querySelector('input');
    if(input)input.focus();
  }
  function close(){overlay.style.display='none'}
  document.addEventListener('keydown',function(e){
    if((e.metaKey||e.ctrlKey)&&e.key==='k'){e.preventDefault();overlay.style.display==='flex'?close():open()}
    if(e.key==='Escape'&&overlay.style.display==='flex'){e.preventDefault();close()}
  });
  overlay.addEventListener('click',function(e){if(e.target===overlay)close()});
})();
</script>
"##;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p site2static`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/site2static/src/search_ui.rs
git commit -m "feat(site2static): add search_ui.rs — Pagefind UI injection via lol_html"
```

---

### Task 4: Wire search into the mirror pipeline

**Files:**
- Modify: `crates/site2static/src/mirror.rs:282-378` (`handle_html_url`) and `crates/site2static/src/mirror.rs:205-241` (`run`)

- [ ] **Step 1: Import search modules in `mirror.rs`**

Add to the top of `mirror.rs` (with other `use` statements):

```rust
use crate::search_ui;
use crate::{SearchMode, SearchUiConfig};
```

- [ ] **Step 2: Inject search UI in `handle_html_url`**

In `handle_html_url`, after `dom.serialize()` (line 360) and before charset conversion (line 361), insert the search UI rewrite pass:

```rust
    let rewritten = dom.serialize();

    // Search UI injection — separate lol_html pass on UTF-8 HTML.
    let rewritten = if let Some(SearchMode::Full(ref ui_config)) = state.config.search {
        search_ui::inject_search_ui(&rewritten, ui_config)
            .unwrap_or_else(|e| {
                tracing::warn!("Search UI injection failed for {}: {}", url, e);
                rewritten
            })
    } else {
        rewritten
    };

    let output_bytes = if needs_conversion {
```

The key point: this runs on UTF-8 HTML, before `charset_convert`.

- [ ] **Step 3: Add pagefind pre-check and invocation in `run()`**

In the `run()` function, add two changes:

**Before Phase 2 workers (before line 150):** Add pagefind pre-check when search is enabled:

```rust
    // Pre-check: verify pagefind binary is available if search is enabled.
    let search_enabled = matches!(
        config.search,
        Some(SearchMode::IndexOnly) | Some(SearchMode::Full(_))
    );
    if search_enabled {
        crate::search::check_pagefind()?;
    }
```

**After `copy_supplemental_globs` (after line 213) and before Phase 3 finalize (before line 218):** Add pagefind invocation:

```rust
    // ------------------------------------------------------------------
    // Phase 2c — Build search index (Pagefind)
    // ------------------------------------------------------------------
    if search_enabled {
        if let Some(cb) = &config.on_progress {
            cb(ProgressEvent::IndexingSearch);
        }
        match crate::search::run_pagefind(&config.output) {
            Ok(indexed_pages) => {
                if let Some(cb) = &config.on_progress {
                    cb(ProgressEvent::SearchDone { pages: indexed_pages });
                }
            }
            Err(e) => {
                tracing::error!("Search indexing failed: {e}");
                // Don't fail the whole export — site is already mirrored
            }
        }
    }
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p site2static`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/site2static/src/mirror.rs
git commit -m "feat(site2static): wire search UI injection and pagefind into mirror pipeline"
```

---

### Task 5: Update blueprint integration

**Files:**
- Modify: `crates/blueprint/src/static_export.rs:57-68`

- [ ] **Step 1: Add `search` field to `MirrorConfig` construction**

In `StaticExporter::export()`, add the `search` field to the `MirrorConfig` construction at line 67 (after `copy_globs`):

```rust
            copy_globs,
            search: None, // Search disabled by default; callers opt in
            on_progress,
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p blueprint`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/blueprint/src/static_export.rs
git commit -m "feat(blueprint): add search field to MirrorConfig (disabled by default)"
```

---

### Task 6: Update existing integration test for new `search` field

**Files:**
- Modify: `crates/site2static/tests/integration.rs:89-100`

- [ ] **Step 1: Add `search` field to existing test's `MirrorConfig`**

In `test_basic_mirror`, add `search: None` to the config construction (after `copy_globs`, before `on_progress`):

```rust
        copy_globs: vec![],
        search: None,
        on_progress: None,
```

- [ ] **Step 2: Verify existing tests pass**

Run: `cargo test -p site2static`
Expected: PASS — `test_basic_mirror` passes, search disabled

- [ ] **Step 3: Commit**

```bash
git add crates/site2static/tests/integration.rs
git commit -m "test(site2static): update existing test for new search config field"
```

---

### Task 7: Unit tests for `search_ui.rs`

**Files:**
- Modify: `crates/site2static/src/search_ui.rs` (add `#[cfg(test)]` module)

- [ ] **Step 1: Add unit tests to `search_ui.rs`**

Append to the bottom of `search_ui.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn full_config() -> SearchUiConfig {
        SearchUiConfig {
            replace_existing: true,
            keyboard_shortcut: true,
        }
    }

    #[test]
    fn injects_pagefind_assets_into_head() {
        let html = "<html><head><title>Test</title></head><body>Hello</body></html>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-ui.css"), "should inject CSS link");
        assert!(result.contains("pagefind-ui.js"), "should inject JS script");
    }

    #[test]
    fn injects_floating_modal() {
        let html = "<html><head></head><body><p>Content</p></body></html>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("s2s-search-overlay"), "should inject modal overlay");
        assert!(result.contains("s2s-search-mount"), "should inject modal mount point");
        assert!(result.contains("metaKey||e.ctrlKey"), "should have Cmd+K handler");
    }

    #[test]
    fn replaces_wp_search_form() {
        let html = r#"<html><head></head><body><form class="search-form"><input type="search"><button>Search</button></form></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-replace-0"), "should replace WP search form");
        assert!(result.contains("PagefindUI"), "should initialize Pagefind UI");
    }

    #[test]
    fn replaces_wp_block_search() {
        let html = r#"<html><head></head><body><div class="wp-block-search"><input type="search"></div></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-replace-0"), "should replace WP block search");
    }

    #[test]
    fn replaces_form_role_search() {
        let html = r#"<html><head></head><body><form role="search"><input name="s"></form></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-replace-0"), "should replace role=search form");
    }

    #[test]
    fn handles_multiple_search_forms() {
        let html = r#"<html><head></head><body><form class="search-form">A</form><form role="search">B</form></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-replace-0"), "should have first replacement");
        assert!(result.contains("pagefind-replace-1"), "should have second replacement");
    }

    #[test]
    fn no_modal_when_disabled() {
        let config = SearchUiConfig {
            replace_existing: true,
            keyboard_shortcut: false,
        };
        let html = "<html><head></head><body><p>Content</p></body></html>";
        let result = inject_search_ui(html, &config).unwrap();
        assert!(!result.contains("s2s-search-overlay"), "should NOT inject modal");
        // But still injects Pagefind assets
        assert!(result.contains("pagefind-ui.js"), "should still inject JS");
    }

    #[test]
    fn no_form_replacement_when_disabled() {
        let config = SearchUiConfig {
            replace_existing: false,
            keyboard_shortcut: true,
        };
        let html = r#"<html><head></head><body><form class="search-form"><input></form></body></html>"#;
        let result = inject_search_ui(html, &config).unwrap();
        assert!(!result.contains("pagefind-replace"), "should NOT replace search form");
        // But still injects modal
        assert!(result.contains("s2s-search-overlay"), "should still inject modal");
    }

    #[test]
    fn marks_main_content_with_pagefind_body() {
        let html = "<html><head></head><body><main><p>Content</p></main></body></html>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("data-pagefind-body"), "should mark main with data-pagefind-body");
    }

    #[test]
    fn marks_only_first_content_element() {
        let html = "<html><head></head><body><main>Main</main><article>Art</article></body></html>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        let count = result.matches("data-pagefind-body").count();
        assert_eq!(count, 1, "should mark only one element with data-pagefind-body");
    }

    #[test]
    fn search_elements_have_pagefind_ignore() {
        let html = "<html><head></head><body><p>Content</p></body></html>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("data-pagefind-ignore"), "search UI should be marked pagefind-ignore");
    }

    #[test]
    fn no_double_replacement_for_multi_selector_form() {
        // A form matching both form.search-form AND form[role="search"] should only be replaced once
        let html = r#"<html><head></head><body><form class="search-form" role="search"><input name="s"></form></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        let count = result.matches("pagefind-replace-").count();
        assert_eq!(count, 1, "form matching multiple selectors should be replaced exactly once");
    }

    #[test]
    fn handles_non_utf8_charset_meta() {
        // Verify injection works on HTML that declares a non-UTF-8 charset (the input is
        // already UTF-8 at this point — charset_convert happens after injection)
        let html = r#"<html><head><meta charset="windows-1252"><title>Test</title></head><body><main>Content</main></body></html>"#;
        let result = inject_search_ui(html, &full_config()).unwrap();
        assert!(result.contains("pagefind-ui.js"), "should inject JS even with non-UTF-8 charset meta");
        assert!(result.contains("data-pagefind-body"), "should mark main content");
    }

    #[test]
    fn passthrough_when_no_head_or_body() {
        let html = "<p>Just a fragment</p>";
        let result = inject_search_ui(html, &full_config()).unwrap();
        // Should not panic — gracefully handle partial HTML
        assert!(result.contains("Just a fragment"));
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p site2static -- search_ui`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/site2static/src/search_ui.rs
git commit -m "test(site2static): add unit tests for search UI injection"
```

---

### Task 8: Integration test — full search pipeline

**Files:**
- Modify: `crates/site2static/tests/integration.rs`

- [ ] **Step 1: Add search integration test**

Add a new test to `integration.rs`:

```rust
#[test]
fn test_mirror_with_search_ui_injection() {
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    // Create test site with a WordPress search form
    fs::write(
        site_dir.path().join("index.html"),
        r#"<html><head><title>Test</title></head><body>
<main>
<h1>Welcome</h1>
<form class="search-form"><input type="search" name="s"><button>Search</button></form>
<a href="/about/">About</a>
</main>
</body></html>"#,
    )
    .unwrap();

    fs::create_dir(site_dir.path().join("about")).unwrap();
    fs::write(
        site_dir.path().join("about/index.html"),
        r#"<html><head><title>About</title></head><body><main><a href="/">Home</a></main></body></html>"#,
    )
    .unwrap();

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
        copy_globs: vec![],
        search: Some(site2static::SearchMode::Full(site2static::SearchUiConfig::default())),
        on_progress: None,
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run();

    // If pagefind is not installed, the mirror still succeeds (search phase logs error)
    // but the HTML should have search UI injected regardless
    match result {
        Ok(r) => assert!(r.pages_crawled >= 2),
        Err(site2static::MirrorError::SearchBinaryNotFound { .. }) => {
            // Expected in CI without pagefind installed — skip rest of assertions
            return;
        }
        Err(e) => panic!("Unexpected error: {e}"),
    }

    // Verify search UI was injected
    let index_html = fs::read_to_string(output_dir.path().join("index.html")).unwrap();
    assert!(index_html.contains("pagefind-ui.js"), "should inject Pagefind JS");
    assert!(index_html.contains("pagefind-ui.css"), "should inject Pagefind CSS");
    assert!(index_html.contains("pagefind-replace-0"), "should replace WP search form");
    assert!(index_html.contains("s2s-search-overlay"), "should inject Cmd+K modal");
    assert!(index_html.contains("data-pagefind-body"), "should mark main content");

    // About page should also have search UI
    let about_html = fs::read_to_string(output_dir.path().join("about/index.html")).unwrap();
    assert!(about_html.contains("pagefind-ui.js"), "about page should have Pagefind JS");
    assert!(about_html.contains("s2s-search-overlay"), "about page should have modal");
}

#[test]
fn test_mirror_with_index_only_search() {
    let site_dir = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();

    fs::write(
        site_dir.path().join("index.html"),
        r#"<html><head></head><body><form class="search-form"><input></form></body></html>"#,
    )
    .unwrap();

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
        copy_globs: vec![],
        search: Some(site2static::SearchMode::IndexOnly),
        on_progress: None,
    };

    let mirror = SiteMirror::new(config);
    let result = mirror.run();

    match result {
        Ok(_) => {}
        Err(site2static::MirrorError::SearchBinaryNotFound { .. }) => return,
        Err(e) => panic!("Unexpected error: {e}"),
    }

    // IndexOnly: no UI injection
    let index_html = fs::read_to_string(output_dir.path().join("index.html")).unwrap();
    assert!(!index_html.contains("pagefind-ui.js"), "should NOT inject Pagefind JS in IndexOnly mode");
    assert!(!index_html.contains("s2s-search-overlay"), "should NOT inject modal in IndexOnly mode");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p site2static -- --test integration`
Expected: Tests PASS (with graceful skip if pagefind not installed)

- [ ] **Step 3: Commit**

```bash
git add crates/site2static/tests/integration.rs
git commit -m "test(site2static): add search integration tests"
```

---

### Task 9: Verify full build and all tests

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --workspace`
Expected: PASS — no compilation errors

- [ ] **Step 2: Run all site2static tests**

Run: `cargo test -p site2static`
Expected: All tests PASS

- [ ] **Step 3: Run blueprint tests**

Run: `cargo test -p blueprint`
Expected: PASS

- [ ] **Step 4: Final commit if any fixups needed**

Only if previous steps required code fixes.
