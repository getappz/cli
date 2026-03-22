# TODO

## Static Site Export Issues

### Font files not loading on deployed static site
- **Symptom**: 56x "Failed to decode downloaded font" / "OTS parsing error: OS/2: missing required table" in browser console
- **Cause**: Font files referenced in CSS `@font-face` are likely not being copied to the static output. The browser finds a 404 HTML page where it expects a `.woff2` file and tries to parse it as a font.
- **Possible roots**:
  - External CDN fonts: CSS `url()` rewriter rewrites paths to relative, but `enqueue_asset_links` only copies same-domain assets — external font files never get copied
  - Query-string font URLs (`fonts.php?family=...`) may not be matched by the CSS URL regex in `css.rs`
- **Files**: `crates/site2static/src/css.rs`, `crates/site2static/src/mirror.rs` (enqueue_asset_links)

### WordPress REST API calls 404 on static site
- **Symptom**: `GET /sureforms/v1/refresh-nonces 404` and "Failed to refresh form nonces" errors from `api-fetch.min.js` and `formSubmit.js`
- **Cause**: JavaScript files are copied verbatim during export with no processing. Scripts that make REST API calls (SureForms, wp-api-fetch) still execute on the static site and hit non-existent endpoints.
- **Fix options**:
  - WordPress plugin: add `wp_dequeue_script('wp-api-fetch')` and SureForms script dequeue in `apply_cli_performance_tweaks()` before export
  - site2static: add exclude patterns for known API-dependent scripts
  - site2static: add JS content filtering to neutralize `fetch()` calls to `/wp-json/` endpoints
- **Files**: `packages/wordpress-plugin/appz-static-site-generator.php`, `crates/site2static/src/dom.rs`, `crates/blueprint/src/static_export.rs`
