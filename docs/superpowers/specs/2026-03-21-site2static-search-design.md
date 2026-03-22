# site2static Search Feature — Design Spec

## Summary

Add client-side search to static sites exported by site2static using [Pagefind](https://pagefind.app/). The feature indexes all exported HTML and optionally injects a search UI into pages — both replacing existing WordPress search forms and adding a Cmd+K floating modal.

## Goals

- Every site exported by site2static can have full-text search with zero external services
- WordPress search forms are replaced with working Pagefind-powered search
- A keyboard-accessible floating search modal (Cmd+K / Ctrl+K) is available on all pages
- UI injection is configurable; indexing is always available when enabled

## Non-Goals

- Server-side search or API-based search
- AI-powered search (like pageflare's "Ask AI" tab)
- Custom Pagefind weighting or filtering rules (can be added later)

## Architecture

### Two-Phase Pipeline

Search is implemented as two phases layered onto the existing mirror pipeline:

```
Phase 1 (during HTML mirroring):
  crawl origin
    -> fetch HTML
    -> dom.rs rewrites URLs (existing)
    -> search_ui.rs runs a SEPARATE lol_html rewrite pass (new, if inject_ui enabled)
    -> write to disk
  copy assets

Phase 2 (after mirroring completes, before metadata cache save):
  pre-check: verify pagefind binary is available
  if available: run `pagefind --site <output_dir> --bundle-dir pagefind`
  if not available: fail early with clear error (do not inject UI without indexer)
```

This keeps the existing pipeline intact and adds search as a composable layer.

### Why Two Phases

Pagefind needs final HTML to build its index, but the UI injection needs to happen during HTML rewriting. Running Pagefind as a post-processing step via CLI is simpler and more maintainable than using Pagefind's Rust API directly.

## Configuration

### SearchConfig

Added as an optional field on `MirrorConfig`:

```rust
pub enum SearchMode {
    Disabled,
    IndexOnly,                    // run pagefind, no UI injection
    Full(SearchUiConfig),         // run pagefind + inject UI
}

pub struct SearchUiConfig {
    pub replace_existing: bool,  // default: true — replace WP search forms with pagefind widget
    pub keyboard_shortcut: bool, // default: true — add Cmd+K floating search modal
}
```

This eliminates invalid states (e.g., `enabled: false` with `inject_ui: true`).

When `search` is `None` on `MirrorConfig`, defaults to `SearchMode::Disabled` (backward compatible).

## New Modules

### `search_ui.rs` — UI Injection

Responsible for injecting search markup into HTML pages. Runs as a **separate `lol_html::rewrite_str` pass** on the UTF-8 HTML — after `dom.rs` URL rewriting but before charset re-encoding in `handle_html_url`.

This avoids any changes to `dom.rs` or its handler structure. The function signature:

```rust
pub fn inject_search_ui(html: &str, config: &SearchUiConfig) -> Result<String> {
    // Runs its own lol_html::rewrite_str with search-specific handlers
    // Input: UTF-8 HTML (after dom.rs processing)
    // Output: UTF-8 HTML with search UI injected
}
```

**Insertion point in `mirror.rs::handle_html_url`:** After `dom.find_urls_as_strings()` returns the rewritten HTML (UTF-8) and before `charset_convert` re-encodes it. This ensures injected ASCII/UTF-8 markup survives charset conversion correctly.

**Pagefind asset references (injected into `<head>`):**
- `<link rel="stylesheet" href="/pagefind/pagefind-ui.css">`
- `<script src="/pagefind/pagefind-ui.js"></script>`

**WordPress search form replacement (`replace_existing`):**

Detects WordPress search forms by matching:
- `form.search-form`
- `form[role="search"]`
- `.wp-block-search`

Uses a "first match wins" guard: tracks whether each form element has already been replaced (via a `RefCell<HashSet>` of element pointers or a boolean flag per handler invocation) to avoid double-replacement when an element matches multiple selectors.

Replaces the form's inner content with a Pagefind UI container:
```html
<div id="pagefind-replace-N"></div>
<script>new PagefindUI({ element: "#pagefind-replace-N", showSubResults: true });</script>
```

Uses a counter (N) to handle multiple search forms on a single page.

**Floating search modal (`keyboard_shortcut`):**

Injects before `</body>`:
```html
<div id="pagefind-modal-overlay" data-pagefind-ignore style="display:none">
  <dialog id="pagefind-modal">
    <div id="pagefind-modal-search"></div>
  </dialog>
</div>
<script>
  // Initialize Pagefind UI in modal
  // Cmd+K / Ctrl+K opens modal
  // Escape closes modal
  // Click outside closes modal
</script>
```

Self-contained CSS (inline `<style>` block) — theme-agnostic, uses dialog overlay pattern.

**Content hinting:**
- Adds `data-pagefind-ignore` to all injected search UI elements (prevents indexing the search widget itself)
- Detects main content area by matching the **first** element found from this priority list: `<main>`, `<article>`, `.entry-content`, `#content`. Only one element gets `data-pagefind-body` to avoid fragmenting the index. If none found, no `data-pagefind-body` is set (Pagefind indexes the whole page, which is acceptable).

### `search.rs` — Pagefind Invocation

Runs Pagefind CLI after the mirror phase completes, before metadata cache is saved.

**Insertion point in `mirror.rs::run`:** After `copy_supplemental_globs` and before the finalization block that saves metadata cache. The `pagefind/` directory is generated output, not cached content — it should not participate in incremental cache logic.

```rust
pub fn run_pagefind(output_dir: &Path) -> Result<()> {
    // Pre-check: verify `pagefind` binary exists (which pagefind)
    // Shell out to: pagefind --site <output_dir> --bundle-dir pagefind
    // Returns Ok if pagefind succeeds, Err with stderr on failure
}
```

**Pre-check strategy:** Before Phase 1 (UI injection), verify that `pagefind` is available on PATH. If not found, return an early error — do not inject search UI markup into pages when there's no indexer to generate the assets those UI elements reference. This prevents shipping broken search widgets.

**Error handling:**
- Pagefind binary not found: fail early before any HTML processing, with a clear error message suggesting `mise install` or manual installation
- Pagefind indexing fails (non-zero exit): report error with stderr, fail the search phase but leave the mirrored site intact (HTML already written, just without a working index)
- Both cases emit `ProgressEvent` for the caller to handle

### New ProgressEvent Variants

```rust
pub enum ProgressEvent {
    // ... existing variants ...
    IndexingSearch,              // emitted when pagefind starts
    SearchDone { pages: usize }, // emitted when pagefind completes successfully
}
```

## Pagefind Binary Management

Pagefind binary is managed via mise. The project's mise config will include pagefind as a tool dependency. Users running site2static will need mise to auto-install it, or can install pagefind manually.

The `--bundle-dir pagefind` flag is passed explicitly to guarantee the output path matches the injected asset references (`/pagefind/pagefind-ui.js`, etc.), regardless of Pagefind version defaults.

## Incremental Build Interaction

When running in incremental mode (`force: false`):
- Pages skipped via HTTP 304 retain their previously-written HTML. If search was enabled on the prior run, the UI markup is already present — no issue.
- **First run with search enabled on an existing output directory**: Pages that return 304 will NOT have search UI injected (their cached HTML predates search). Pagefind will still index them (it reads the HTML files regardless), but those pages won't have the search modal/replacement. Mitigation: when `SearchMode` changes from the previous run (detectable via metadata cache), force a full re-crawl. Alternatively, document that users should use `force: true` when first enabling search.

## Output Structure

After export with search enabled:

```
output/
  index.html                    (with search UI injected)
  about/index.html              (with search UI injected)
  ...
  pagefind/                     (generated by pagefind CLI)
    pagefind.js
    pagefind-ui.js
    pagefind-ui.css
    pagefind-highlight.js
    wasm.*.pagefind
    index/
      en_*.pf_index
    fragment/
      ...
```

## Testing Strategy

- Unit tests for `search_ui.rs`: verify HTML injection output for various WordPress theme patterns
- Unit tests for search form detection: various WordPress search form HTML structures, including forms matching multiple selectors (verify no double-replacement)
- Unit test for charset safety: inject into HTML with non-UTF-8 charset meta tag, verify round-trip works
- Integration test: mirror a small test site with search enabled, verify pagefind directory exists and UI markup is present in output HTML
- Test with `SearchMode::IndexOnly`: verify pagefind index is built but no UI markup in HTML
- Test with `replace_existing: false`: verify WordPress search forms are untouched
- Test with `keyboard_shortcut: false`: verify no floating modal injected
- Test pagefind binary not found: verify early error before HTML processing

## Backward Compatibility

- `SearchConfig` is optional on `MirrorConfig` (defaults to `None` = `SearchMode::Disabled`)
- Existing callers are unaffected — no behavior change without opt-in
- No changes to existing HTML rewriting logic in `dom.rs`
- `search_ui.rs` operates as an independent rewrite pass, not integrated into `dom.rs` handlers
