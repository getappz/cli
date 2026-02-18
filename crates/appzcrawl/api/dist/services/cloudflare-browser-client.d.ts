/**
 * Cloudflare Browser Rendering API client.
 *
 * Provides a CloudflareBrowserEngine class with methods for:
 * - content: Fetch rendered HTML
 * - snapshot: HTML + base64 screenshot
 * - markdown: Extract page as Markdown
 * - scrape: Extract elements by CSS selectors (text, html, attributes, dimensions)
 * - json: AI-powered structured extraction (prompt + optional JSON schema)
 *
 * API: https://developers.cloudflare.com/browser-rendering/
 *      https://developers.cloudflare.com/api/resources/browser_rendering/
 */
export interface CloudflareContentResult {
    success: true;
    html: string;
    statusCode: number;
}
export interface CloudflareSnapshotResult {
    success: true;
    html: string;
    statusCode: number;
    /** Base64-encoded PNG (or JPEG). Caller uploads to R2 and constructs URL. */
    screenshotBase64: string;
}
export interface CloudflareMarkdownResult {
    success: true;
    markdown: string;
    statusCode: number;
}
/** Single scraped element: text, html, attributes, dimensions */
export interface CloudflareScrapedElement {
    text?: string;
    html?: string;
    attributes?: Array<{
        name: string;
        value: string;
    }>;
    height?: number;
    width?: number;
    top?: number;
    left?: number;
}
/** Per-selector scrape result */
export interface CloudflareScrapeSelectorResult {
    selector: string;
    results: CloudflareScrapedElement[];
}
export interface CloudflareScrapeResult {
    success: true;
    data: CloudflareScrapeSelectorResult[];
    statusCode: number;
}
export interface CloudflareJsonResult {
    success: true;
    data: Record<string, unknown>;
    statusCode: number;
}
export interface CloudflareBrowserError {
    success: false;
    error: string;
}
export type CloudflareContentOutput = CloudflareContentResult | CloudflareBrowserError;
export type CloudflareSnapshotOutput = CloudflareSnapshotResult | CloudflareBrowserError;
export type CloudflareMarkdownOutput = CloudflareMarkdownResult | CloudflareBrowserError;
export type CloudflareScrapeOutput = CloudflareScrapeResult | CloudflareBrowserError;
export type CloudflareJsonOutput = CloudflareJsonResult | CloudflareBrowserError;
export type GotoWaitUntil = "load" | "domcontentloaded" | "networkidle0" | "networkidle2";
/** Shared options for page load behavior (url/html input, gotoOptions, viewport). */
export interface CloudflarePageLoadOptions {
    waitUntil?: GotoWaitUntil;
    viewport?: {
        width: number;
        height: number;
    };
    timeout?: number;
}
export interface CloudflareContentOptions extends CloudflarePageLoadOptions {
}
export interface CloudflareSnapshotOptions extends CloudflarePageLoadOptions {
    fullPage?: boolean;
    screenshotViewport?: {
        width: number;
        height: number;
    };
}
export interface CloudflareMarkdownOptions extends CloudflarePageLoadOptions {
    /** Regex patterns to reject requests (e.g. exclude CSS). */
    rejectRequestPattern?: string[];
}
export interface CloudflareScrapeElement {
    selector: string;
}
export interface CloudflareScrapeOptions extends CloudflarePageLoadOptions {
    elements: CloudflareScrapeElement[];
}
/** JSON schema for response_format (OpenAI-compatible). */
export interface CloudflareJsonSchema {
    type: "json_schema";
    schema: Record<string, unknown>;
}
/** Custom AI model (BYO API key). */
export interface CloudflareCustomAi {
    model: string;
    authorization: string;
}
export interface CloudflareJsonOptions extends CloudflarePageLoadOptions {
    /** Natural language prompt for extraction. */
    prompt?: string;
    /** JSON schema to structure output. Use with Workers AI; avoid with Anthropic. */
    responseFormat?: CloudflareJsonSchema;
    /** Custom AI model(s); first succeeds, rest are fallbacks. */
    customAi?: CloudflareCustomAi[];
}
/**
 * Cloudflare Browser Rendering Engine.
 * Use accountId and apiToken (from env CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_BROWSER_RENDERING_API_TOKEN).
 */
export declare class CloudflareBrowserEngine {
    readonly accountId: string;
    readonly apiToken: string;
    constructor(accountId: string, apiToken: string);
    /**
     * Fetch rendered HTML from /content.
     * https://developers.cloudflare.com/browser-rendering/rest-api/content-endpoint/
     */
    content(url: string, options?: CloudflareContentOptions): Promise<CloudflareContentOutput>;
    /**
     * Fetch HTML + base64 screenshot from /snapshot.
     * https://developers.cloudflare.com/browser-rendering/rest-api/snapshot/
     */
    snapshot(url: string, options?: CloudflareSnapshotOptions): Promise<CloudflareSnapshotOutput>;
    /**
     * Extract page as Markdown from /markdown.
     * https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/
     */
    markdown(url: string, options?: CloudflareMarkdownOptions): Promise<CloudflareMarkdownOutput>;
    /**
     * Scrape elements by CSS selectors from /scrape.
     * Returns text, html, attributes, dimensions per element.
     * https://developers.cloudflare.com/browser-rendering/rest-api/scrape-endpoint/
     */
    scrape(url: string, options: CloudflareScrapeOptions): Promise<CloudflareScrapeOutput>;
    /**
     * AI-powered structured JSON extraction from /json.
     * Requires prompt and/or responseFormat. Uses Workers AI by default; use customAi for BYO model.
     * https://developers.cloudflare.com/browser-rendering/rest-api/json-endpoint/
     */
    json(url: string, options: CloudflareJsonOptions): Promise<CloudflareJsonOutput>;
}
export declare function cloudflareFetchContent(accountId: string, apiToken: string, url: string, options?: CloudflareContentOptions): Promise<CloudflareContentOutput>;
export declare function cloudflareFetchSnapshot(accountId: string, apiToken: string, url: string, options?: CloudflareSnapshotOptions): Promise<CloudflareSnapshotOutput>;
export declare function isCloudflareBrowserEnabled(env: {
    CLOUDFLARE_ACCOUNT_ID?: string;
    CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
}): boolean;
/**
 * Create a CloudflareBrowserEngine from env bindings.
 * Returns null if credentials are not configured.
 */
export declare function createCloudflareBrowserEngine(env: {
    CLOUDFLARE_ACCOUNT_ID?: string;
    CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
}): CloudflareBrowserEngine | null;
//# sourceMappingURL=cloudflare-browser-client.d.ts.map