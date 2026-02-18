/**
 * Unified HTML processing facade.
 *
 * Delegates to one of three backends in priority order:
 *
 * 1. **WASM** (`wasm-processor.ts`) — compiled firecrawl_rs Rust library
 *    running directly in the Worker via WebAssembly.  Full DOM parsing
 *    (kuchikiki), link filtering (robots.txt, PSL), document conversion.
 *    Fastest for all HTML processing (~0ms).
 *
 * 2. **Engine** (`engine-processor.ts`) — appzcrawl-engine Worker called
 *    via RPC Service Binding.  Same Rust code as WASM but runs in a
 *    separate Worker with its own CPU budget.  Also provides
 *    html-to-markdown (pure Rust htmd, no Go FFI needed).
 *    Used for: html-to-markdown, citations postprocess, and as WASM fallback.
 *
 * 3. **Container** (`native-container.ts`) — Cloudflare Container running
 *    native Rust via Axum.  Handles everything but incurs cold-start cost.
 *    Used for: PDF, search (functions the engine doesn't support yet).
 *
 * ### Switching backends
 *
 * - `USE_CONTAINER_BACKEND=true` → force container for everything.
 * - `DISABLE_WASM_BACKEND=true`  → skip WASM, fall through to Engine/Container.
 * - Default: WASM > Engine > Container (best performance).
 *
 * ### Deprecated
 *
 * HTMLRewriter (`worker-html-processor.ts`) is deprecated. WASM provides
 * identical results with better accuracy (full DOM parsing vs streaming).
 *
 * ### Re-exports
 *
 * This module re-exports every public symbol from `native-container.ts`
 * (types, interfaces, container-only functions) so call-sites only need
 * to change their import path to `./html-processor`.
 */
// ---- Engine implementations (RPC to appzcrawl-engine Worker) --------------
import * as engine from "./engine-processor";
// ---- Container implementations (always available as fallback) -------------
import * as container from "./native-container";
// ---- WASM implementations (compiled firecrawl_rs) -------------------------
import * as wasmNative from "./wasm-processor";
// ---------------------------------------------------------------------------
// Backend selector
// ---------------------------------------------------------------------------
/**
 * Returns `true` when the container backend should be used for everything.
 */
function useContainer(env) {
    const flag = env.USE_CONTAINER_BACKEND;
    return flag === "true" || flag === true;
}
/**
 * Returns `true` when the WASM backend should be used.
 * WASM is preferred when available, unless explicitly disabled.
 */
function useWasm(env) {
    if (!wasmNative.isAvailable())
        return false;
    const flag = env.DISABLE_WASM_BACKEND;
    return flag !== "true" && flag !== true;
}
/**
 * Returns `true` when the Engine (appzcrawl-engine) RPC binding is available.
 */
function useEngine(env) {
    return engine.isAvailable(env);
}
// =========================================================================
// Functions — Priority: WASM > Engine > Container
// =========================================================================
export async function extractLinks(env, html) {
    if (useContainer(env))
        return container.extractLinks(env, html);
    if (useWasm(env))
        return wasmNative.extractLinks(html);
    if (useEngine(env))
        return engine.extractLinks(env, html);
    return container.extractLinks(env, html);
}
export async function extractBaseHref(env, html, baseUrl) {
    if (useContainer(env))
        return container.extractBaseHref(env, html, baseUrl);
    if (useWasm(env))
        return wasmNative.extractBaseHref(html, baseUrl);
    if (useEngine(env))
        return engine.extractBaseHref(env, html, baseUrl);
    return container.extractBaseHref(env, html, baseUrl);
}
export async function extractMetadata(env, html) {
    if (useContainer(env))
        return container.extractMetadata(env, html);
    if (useWasm(env))
        return wasmNative.extractMetadata(html);
    if (useEngine(env))
        return engine.extractMetadata(env, html);
    return container.extractMetadata(env, html);
}
export async function getInnerJson(env, html) {
    if (useContainer(env))
        return container.getInnerJson(env, html);
    if (useWasm(env))
        return wasmNative.getInnerJson(html);
    if (useEngine(env))
        return engine.getInnerJson(env, html);
    return container.getInnerJson(env, html);
}
export async function extractImages(env, html, baseUrl) {
    if (useContainer(env))
        return container.extractImages(env, html, baseUrl);
    if (useWasm(env))
        return wasmNative.extractImages(html, baseUrl);
    if (useEngine(env))
        return engine.extractImages(env, html, baseUrl);
    return container.extractImages(env, html, baseUrl);
}
export async function extractAssets(env, html, baseUrl, formats) {
    if (useContainer(env))
        return container.extractAssets(env, html, baseUrl, formats);
    if (useWasm(env))
        return wasmNative.extractAssets(html, baseUrl, formats);
    if (useEngine(env))
        return engine.extractAssets(env, html, baseUrl, formats);
    return container.extractAssets(env, html, baseUrl, formats);
}
export async function postProcessMarkdown(env, markdown, options) {
    if (useContainer(env))
        return container.postProcessMarkdown(env, markdown, options);
    // WASM handles core postprocess. Engine handles citations too (has postprocess module).
    if (options?.citations && useEngine(env))
        return engine.postProcessMarkdown(env, markdown, options);
    if (options?.citations)
        return container.postProcessMarkdown(env, markdown, options);
    if (useWasm(env))
        return wasmNative.postProcessMarkdown(markdown, options);
    if (useEngine(env))
        return engine.postProcessMarkdown(env, markdown, options);
    return container.postProcessMarkdown(env, markdown, options);
}
export async function parseSitemap(env, xml) {
    if (useContainer(env))
        return container.parseSitemap(env, xml);
    if (useWasm(env))
        return wasmNative.parseSitemap(xml);
    if (useEngine(env))
        return engine.parseSitemap(env, xml);
    return container.parseSitemap(env, xml);
}
// =========================================================================
// Functions with WASM + Engine + Container (no HTMLRewriter)
// =========================================================================
export async function extractAttributes(env, html, options) {
    if (useContainer(env))
        return container.extractAttributes(env, html, options);
    if (useWasm(env))
        return wasmNative.extractAttributes(html, options);
    if (useEngine(env))
        return engine.extractAttributes(env, html, options);
    return container.extractAttributes(env, html, options);
}
export async function transformHtml(env, params) {
    if (useContainer(env))
        return container.transformHtml(env, params);
    if (useWasm(env))
        return wasmNative.transformHtml(params);
    if (useEngine(env))
        return engine.transformHtml(env, params);
    return container.transformHtml(env, params);
}
export async function filterLinks(env, params) {
    if (useContainer(env))
        return container.filterLinks(env, params);
    if (useWasm(env))
        return wasmNative.filterLinks(params);
    if (useEngine(env))
        return engine.filterLinks(env, params);
    return container.filterLinks(env, params);
}
export async function convertDocument(env, params) {
    if (useContainer(env))
        return container.convertDocument(env, params);
    if (useWasm(env)) {
        try {
            return wasmNative.convertDocumentToHtml(params);
        }
        catch {
            // Fall through to engine/container
        }
    }
    if (useEngine(env)) {
        try {
            return await engine.convertDocument(env, params);
        }
        catch {
            // Fall through to container
        }
    }
    return container.convertDocument(env, params);
}
// =========================================================================
// html-to-markdown — Engine (pure Rust htmd) > Container (Go FFI)
// =========================================================================
export async function htmlToMarkdown(env, html) {
    // Engine provides html-to-markdown via htmd (pure Rust) — no Container needed!
    if (useEngine(env))
        return engine.htmlToMarkdown(env, html);
    // Fall back to Container (Go FFI)
    return container.htmlToMarkdown(env, html);
}
// =========================================================================
// Container-only functions (PDF, search, health)
// =========================================================================
export async function nativeSearch(env, options) {
    // Engine uses Workers fetch for DDG search — faster than Container's reqwest
    if (useEngine(env))
        return engine.nativeSearch(env, options);
    return container.nativeSearch(env, options);
}
export async function convertPdf(env, params) {
    // Container-only: lopdf PDF extraction
    return container.convertPdf(env, params);
}
export async function getPdfMetadata(env, input) {
    // Container-only: lopdf PDF metadata
    return container.getPdfMetadata(env, input);
}
export async function nativeHealth(env) {
    return container.nativeHealth(env);
}
//# sourceMappingURL=html-processor.js.map