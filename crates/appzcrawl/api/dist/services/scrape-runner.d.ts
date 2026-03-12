/**
 * Scrape runner orchestrator.
 * Coordinates: fetch URL → transform HTML → extract metadata & links → format processing.
 * Delegates to scrape-fetcher (fetch), scrape-format-processor (assets/markdown), and cache layer.
 */
import type { ScrapeFormat } from "../contracts/scrape";
import type { AppEnv } from "../types";
export interface ScrapeRunnerOptions {
    /** Only main content (passed to transformHtml). */
    onlyMainContent?: boolean;
    /** Use fire-engine for fetch when FIRE_ENGINE_URL is set. Falls back to Worker fetch on error. */
    useFireEngine?: boolean;
    /** Fetch engine for web URLs: native (default), cloudflare, or auto. Documents always use native. */
    engine?: "native" | "cloudflare" | "auto";
    /** Output formats to produce. */
    formats?: ScrapeFormat[];
    /** Tags to include. */
    includeTags?: string[];
    /** Tags to exclude. */
    excludeTags?: string[];
    /** Cache max age in ms. 0 = always fresh. */
    maxAge?: number;
    /** Store result in cache after scrape. Default true. */
    storeInCache?: boolean;
    /** Zero data retention — do not store in cache. */
    zeroDataRetention?: boolean;
    /** Custom headers — bypasses cache (request-specific). */
    headers?: Record<string, string>;
    /** Convert links to citations. Default: false. */
    citations?: boolean;
    /** Emulate mobile viewport for screenshots. Default: false. */
    mobile?: boolean;
    /** Remove base64 images from markdown. Default: true. */
    removeBase64Images?: boolean;
    /** Block ads when capturing screenshots. Default: true. */
    blockAds?: boolean;
    /** Skip TLS cert verification (fire-engine only). Default: true. */
    skipTlsVerification?: boolean;
    /** Timeout in ms. Default: 30000, max 300000. */
    timeout?: number;
    /** Base URL for screenshot links. Required for screenshot format. */
    screenshotBaseUrl?: string;
    /** Screenshot options from request. */
    screenshotOptions?: {
        fullPage?: boolean;
        viewport?: {
            width: number;
            height: number;
        };
        quality?: number;
    };
    /** Wait before screenshot capture (ms). */
    waitFor?: number;
    /** JSON (LLM extraction) options when formats includes "json". */
    jsonOptions?: {
        prompt?: string;
        schema?: Record<string, unknown>;
    };
}
/** Branding profile (Firecrawl-compatible). */
export type BrandingProfile = Record<string, unknown>;
/** Canonical document produced by the scrape runner. */
export interface ScrapeRunnerDocument {
    url: string;
    rawHtml: string;
    html: string;
    markdown: string;
    metadata: Record<string, unknown>;
    links: string[];
    images?: string[];
    assets?: string[];
    /** Native JSON from Sarvam PDF (for json format response). */
    documentJson?: unknown;
    statusCode?: number;
    branding?: BrandingProfile;
    brandingError?: string;
    screenshot?: string;
}
export interface ScrapeRunnerResult {
    success: true;
    document: ScrapeRunnerDocument;
}
export interface ScrapeRunnerError {
    success: false;
    url: string;
    error: string;
    statusCode?: number;
}
export type ScrapeRunnerOutput = ScrapeRunnerResult | ScrapeRunnerError;
export declare function runScrapeUrl(env: AppEnv["Bindings"], url: string, options?: ScrapeRunnerOptions): Promise<ScrapeRunnerOutput>;
//# sourceMappingURL=scrape-runner.d.ts.map