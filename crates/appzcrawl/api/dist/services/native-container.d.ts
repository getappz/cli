/**
 * Client for the appzcrawl native addon running in a Cloudflare Container.
 * Uses getRandom() for stateless load balancing across container instances.
 */
import type { AppEnv } from "../types";
export declare function nativeHealth(env: AppEnv["Bindings"]): Promise<{
    ok: boolean;
}>;
export declare function extractLinks(env: AppEnv["Bindings"], html: string | null): Promise<{
    links: string[];
}>;
export interface FilterLinksParams {
    links: string[];
    limit?: number;
    maxDepth: number;
    baseUrl: string;
    initialUrl: string;
    regexOnFullUrl?: boolean;
    excludes?: string[];
    includes?: string[];
    allowBackwardCrawling?: boolean;
    ignoreRobotsTxt?: boolean;
    robotsTxt?: string;
    allowExternalContentLinks?: boolean;
    allowSubdomains?: boolean;
}
export interface FilterLinksResult {
    links: string[];
    denialReasons: Record<string, string>;
}
/** Filter discovered links using the native Rust crawler (includePaths, excludePaths, robots, depth, etc.). */
export declare function filterLinks(env: AppEnv["Bindings"], params: FilterLinksParams): Promise<FilterLinksResult>;
/** Extract base href from HTML (firecrawl_rs). Used to resolve relative links. */
export declare function extractBaseHref(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    baseHref: string;
}>;
export interface TransformHtmlParams {
    html: string;
    url: string;
    include_tags?: string[];
    exclude_tags?: string[];
    only_main_content?: boolean;
    omce_signatures?: string[];
}
export declare function transformHtml(env: AppEnv["Bindings"], params: TransformHtmlParams): Promise<{
    html: string;
}>;
export declare function getInnerJson(env: AppEnv["Bindings"], html: string): Promise<{
    content: string;
}>;
export declare function extractMetadata(env: AppEnv["Bindings"], html: string | null): Promise<{
    metadata: unknown;
}>;
export interface ExtractAttributesOption {
    selector: string;
    attribute: string;
}
export declare function extractAttributes(env: AppEnv["Bindings"], html: string, options: {
    selectors: ExtractAttributesOption[];
}): Promise<{
    results: unknown;
}>;
export declare function extractImages(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    images: string[];
}>;
/** Native container extract-assets response (css, js, images, fonts, videos, audio, iframes). */
export interface ExtractedAssetsResult {
    images?: string[];
    css?: string[];
    js?: string[];
    fonts?: string[];
    videos?: string[];
    audio?: string[];
    iframes?: string[];
}
export declare function extractAssets(env: AppEnv["Bindings"], html: string, baseUrl: string, formats?: string[]): Promise<ExtractedAssetsResult>;
export declare function postProcessMarkdown(env: AppEnv["Bindings"], markdown: string, options?: {
    baseUrl?: string;
    citations?: boolean;
}): Promise<{
    markdown: string;
}>;
/** Parse sitemap XML via native container (Rust quick-xml). Returns page URLs and child sitemap URLs. */
export declare function parseSitemap(env: AppEnv["Bindings"], xml: string): Promise<{
    urls: string[];
    sitemapUrls: string[];
}>;
export interface NativeSearchOptions {
    query: string;
    num_results?: number;
    tbs?: string;
    filter?: string;
    lang?: string;
    country?: string;
    location?: string;
    timeout_ms?: number;
}
export interface WebSearchResult {
    url: string;
    title: string;
    description: string;
}
export interface NativeSearchResponse {
    web?: WebSearchResult[];
}
/**
 * Perform web search using native Rust implementation.
 * - Uses SearXNG if SEARXNG_ENDPOINT is configured (env var)
 * - Falls back to DuckDuckGo if SearXNG is not available or returns no results
 * - Adapted from firecrawl/apps/api/src/search/v2/index.ts
 */
export declare function nativeSearch(env: AppEnv["Bindings"], options: NativeSearchOptions): Promise<NativeSearchResponse>;
/** Convert HTML to markdown via native container (Go library). */
export declare function htmlToMarkdown(env: AppEnv["Bindings"], html: string): Promise<{
    markdown: string;
}>;
/**
 * Convert Office documents (docx, xlsx, odt, rtf, doc, xls) to HTML.
 * Firecrawl-compatible: same flow as firecrawl document engine.
 */
export declare function convertDocument(env: AppEnv["Bindings"], params: {
    /** Document binary data. */
    data: Uint8Array;
    /** URL for extension-based type detection when contentType is absent. */
    url?: string;
    /** Content-Type header for type detection. */
    contentType?: string | null;
}): Promise<{
    html: string;
}>;
/**
 * Convert PDF to HTML.
 * Firecrawl-compatible: same flow as Firecrawl PDF engine (text → HTML for pipeline).
 */
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
//# sourceMappingURL=native-container.d.ts.map