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
import type { AppEnv } from "../types";
import * as container from "./native-container";
export type { ExtractAttributesOption, ExtractedAssetsResult, FilterLinksParams, FilterLinksResult, NativeSearchOptions, NativeSearchResponse, TransformHtmlParams, WebSearchResult, } from "./native-container";
export declare function extractLinks(env: AppEnv["Bindings"], html: string | null): Promise<{
    links: string[];
}>;
export declare function extractBaseHref(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    baseHref: string;
}>;
export declare function extractMetadata(env: AppEnv["Bindings"], html: string | null): Promise<{
    metadata: unknown;
}>;
export declare function getInnerJson(env: AppEnv["Bindings"], html: string): Promise<{
    content: string;
}>;
export declare function extractImages(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    images: string[];
}>;
export declare function extractAssets(env: AppEnv["Bindings"], html: string, baseUrl: string, formats?: string[]): Promise<container.ExtractedAssetsResult>;
export declare function postProcessMarkdown(env: AppEnv["Bindings"], markdown: string, options?: {
    baseUrl?: string;
    citations?: boolean;
}): Promise<{
    markdown: string;
}>;
export declare function parseSitemap(env: AppEnv["Bindings"], xml: string): Promise<{
    urls: string[];
    sitemapUrls: string[];
}>;
export declare function extractAttributes(env: AppEnv["Bindings"], html: string, options: {
    selectors: container.ExtractAttributesOption[];
}): Promise<{
    results: unknown;
}>;
export declare function transformHtml(env: AppEnv["Bindings"], params: container.TransformHtmlParams): Promise<{
    html: string;
}>;
export declare function filterLinks(env: AppEnv["Bindings"], params: container.FilterLinksParams): Promise<container.FilterLinksResult>;
export declare function convertDocument(env: AppEnv["Bindings"], params: {
    data: Uint8Array;
    url?: string;
    contentType?: string | null;
}): Promise<{
    html: string;
}>;
export declare function htmlToMarkdown(env: AppEnv["Bindings"], html: string): Promise<{
    markdown: string;
}>;
export declare function nativeSearch(env: AppEnv["Bindings"], options: container.NativeSearchOptions): Promise<container.NativeSearchResponse>;
export declare function convertPdf(env: AppEnv["Bindings"], params: {
    data: Uint8Array;
}): Promise<{
    html: string;
}>;
export declare function getPdfMetadata(env: AppEnv["Bindings"], input: {
    pdfBase64: string;
} | Uint8Array): Promise<{
    num_pages: number;
    title: string | null;
}>;
export declare function nativeHealth(env: AppEnv["Bindings"]): Promise<{
    ok: boolean;
}>;
//# sourceMappingURL=html-processor.d.ts.map