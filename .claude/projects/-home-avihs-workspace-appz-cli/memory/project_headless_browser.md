---
name: headless-browser-enhancement
description: Future enhancement for site2static — use chromiumoxide for JS-loaded asset discovery instead of copy_globs
type: project
---

site2static v2 enhancement: use headless browser (chromiumoxide crate) for asset discovery.

**Why:** Current approach uses `copy_globs` patterns to catch JS-dynamically-loaded assets (Elementor webpack chunks, lazy CSS). This is fragile and plugin-specific. A headless browser naturally discovers all assets by executing JS.

**How to apply:** When working on site2static asset discovery improvements, prototype a hybrid approach:
1. Launch headless Chrome via `chromiumoxide` (async, DevTools Protocol)
2. Visit each page, listen for `Network.requestWillBeSent` events → collect full URL set
3. Pass URL set to existing filesystem copier + URL rewriter
4. This eliminates `copy_globs` entirely

**Trade-offs:** Adds ~400MB Chromium dependency, higher memory usage (~200-500MB per instance), but solves the dynamic asset problem completely for any CMS/plugin.

**Crate options evaluated (March 2026):**
- `chromiumoxide` — recommended (async, DevTools Protocol, active)
- `headless_chrome` — alternative (sync, Puppeteer-like)
- `fantoccini` — WebDriver-based (overkill)
- `playwright-rust` — less maintained
