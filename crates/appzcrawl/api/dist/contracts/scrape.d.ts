/**
 * Firecrawl-compatible scrape API contracts.
 * Request/response types aligned to Firecrawl v2 scrape endpoint for drop-in replacement.
 */
/** Output formats supported by Firecrawl scrape API. */
export declare const SCRAPE_FORMATS: readonly ["markdown", "html", "rawHtml", "links", "images", "assets", "css", "js", "fonts", "videos", "audio", "iframes", "screenshot", "screenshot@fullPage", "json", "changeTracking", "branding"];
/** Asset format types that trigger extract-assets. "assets" = all, others = specific. */
export declare const ASSET_FORMAT_TYPES: readonly ["assets", "images", "css", "js", "fonts", "videos", "audio", "iframes"];
export type AssetFormatType = (typeof ASSET_FORMAT_TYPES)[number];
/** Formats that map to native extract-assets (excludes "assets" which means "all"). */
export declare const ASSET_TYPE_FORMATS: AssetFormatType[];
/**
 * Returns true if formats requests any asset extraction.
 */
export declare function wantsAnyAssets(formats: ScrapeFormat[]): boolean;
/**
 * Returns the asset formats to pass to native extract-assets.
 * - ["assets"] when user requested "assets" (all types)
 * - Otherwise the list of specific asset types requested (e.g. ["css", "js"])
 */
export declare function getAssetFormatsToExtract(formats: ScrapeFormat[]): string[];
export type ScrapeFormat = (typeof SCRAPE_FORMATS)[number];
/** Firecrawl-compatible scrape request body. */
export interface ScrapeRequestBody {
    /** Required: URL to scrape. */
    url: string;
    /** Output formats. Default: ["markdown"]. */
    formats?: ScrapeFormat[];
    /** Only main content. Default: true. */
    onlyMainContent?: boolean;
    /** Tags to include. */
    includeTags?: string[];
    /** Tags to exclude. */
    excludeTags?: string[];
    /** Cache max age in ms. */
    maxAge?: number;
    /** Custom headers for request. */
    headers?: Record<string, string>;
    /** Wait delay in ms. */
    waitFor?: number;
    /** Mobile emulation. */
    mobile?: boolean;
    /** Skip TLS verification. */
    skipTlsVerification?: boolean;
    /** Timeout in ms. */
    timeout?: number;
    /** Browser actions (partially supported). */
    actions?: unknown[];
    /** Location settings. */
    location?: {
        country?: string;
        languages?: string[];
    };
    /** Remove base64 images. */
    removeBase64Images?: boolean;
    /** Block ads. */
    blockAds?: boolean;
    /** Proxy mode. */
    proxy?: "basic" | "enhanced" | "auto";
    /** Store in cache. */
    storeInCache?: boolean;
    /** Zero data retention. */
    zeroDataRetention?: boolean;
    /** Appzcrawl: use fire-engine for fetch. */
    useFireEngine?: boolean;
    /** Fetch engine for web URLs: native (default), cloudflare, or auto. */
    engine?: "native" | "cloudflare" | "auto";
    /** Appzcrawl: convert links to citations (text⟨1⟩ + References section). */
    citations?: boolean;
    /** V2 screenshot options (when formats includes screenshot). */
    screenshotOptions?: {
        fullPage?: boolean;
        viewport?: {
            width: number;
            height: number;
        };
        quality?: number;
    };
    /** JSON (LLM extraction) options when formats includes "json". Prompt + schema drive structured output. */
    jsonOptions?: {
        prompt?: string;
        schema?: Record<string, unknown>;
    };
}
/** Resolved screenshot options from formats + screenshotOptions. */
export interface ScreenshotOptionsResolved {
    fullPage: boolean;
    viewport?: {
        width: number;
        height: number;
    };
    quality?: number;
}
/** Resolve screenshot options from request. */
export declare function resolveScreenshotOptions(formats: ScrapeFormat[], screenshotOptions?: ScrapeRequestBody["screenshotOptions"]): ScreenshotOptionsResolved | null;
/** Default request values matching Firecrawl. */
export declare const SCRAPE_DEFAULTS: Required<Pick<ScrapeRequestBody, "formats" | "onlyMainContent" | "includeTags" | "excludeTags" | "maxAge" | "waitFor" | "mobile" | "skipTlsVerification" | "timeout" | "removeBase64Images" | "blockAds" | "proxy" | "storeInCache" | "zeroDataRetention">>;
/** Firecrawl-compatible scrape response metadata. */
export interface ScrapeResponseMetadata {
    title?: string;
    description?: string;
    language?: string | null;
    sourceURL?: string;
    statusCode?: number;
    error?: string | null;
    [key: string]: unknown;
}
/** Firecrawl-compatible scrape response data. */
export interface ScrapeResponseData {
    markdown?: string;
    html?: string | null;
    rawHtml?: string | null;
    screenshot?: string | null;
    links?: string[];
    images?: string[];
    assets?: string[];
    actions?: {
        screenshots?: string[];
        scrapes?: Array<{
            url: string;
            html: string;
        }>;
        javascriptReturns?: Array<{
            type: string;
            value: unknown;
        }>;
        pdfs?: string[];
    } | null;
    metadata?: ScrapeResponseMetadata;
    llm_extraction?: unknown | null;
    warning?: string | null;
    changeTracking?: {
        previousScrapeAt?: string | null;
        changeStatus?: "new" | "same" | "changed" | "removed";
        visibility?: "visible" | "hidden";
        diff?: string | null;
        json?: Record<string, unknown> | null;
    } | null;
    branding?: Record<string, unknown> | null;
}
/** Firecrawl-compatible response envelope (cacheState, creditsUsed, etc.). */
export interface ScrapeResponseEnvelope {
    cacheState: "hit" | "miss";
    cachedAt: string;
    creditsUsed: number;
    concurrencyLimited: boolean;
}
/** Build response envelope for cache miss (e.g. crawl, extract, agent, search, map). */
export declare function responseEnvelope(creditsUsed?: number): ScrapeResponseEnvelope;
/** Firecrawl-compatible scrape success response. */
export interface ScrapeSuccessResponse {
    success: true;
    data: ScrapeResponseData;
    cacheState: "hit" | "miss";
    cachedAt: string;
    creditsUsed: number;
    concurrencyLimited: boolean;
}
/** Firecrawl-compatible scrape error response. */
export interface ScrapeErrorResponse {
    success: false;
    error: string;
    url?: string;
}
export type ScrapeResponse = ScrapeSuccessResponse | ScrapeErrorResponse;
/** Parse and normalize request body with Firecrawl defaults. Accepts unknown keys without failing. */
export declare function parseScrapeRequestBody(body: unknown): {
    ok: true;
    data: ScrapeRequestBody;
} | {
    ok: false;
    error: string;
};
//# sourceMappingURL=scrape.d.ts.map