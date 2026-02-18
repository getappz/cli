/**
 * Crawl controller: Firecrawl-compatible /crawl endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/crawl.ts + crawl-status.ts + crawl-cancel.ts.
 *
 * Flow: POST /crawl → D1 insert → enqueue to CRAWL_QUEUE → return 200 {id, url}
 * Status/cancel/ongoing/errors read from D1 (replaces Redis).
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function crawlController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    id: string;
    url: string;
}, 200, "json">)>;
export declare function crawlParamsPreviewController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    data: {
        totalCredits: number;
        urls: never[];
    };
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
export declare function crawlStatusController(c: Context<AppEnv>, _isBatchScrape?: boolean): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 404, "json">) | (Response & import("hono").TypedResponse<{
    success: boolean;
    status: import("../contracts/crawl").CrawlJobStatus;
    completed: number;
    total: number;
    creditsUsed: number;
    expiresAt?: string | undefined;
    data: {
        url: string;
        markdown?: string | undefined;
        html?: string | undefined;
        rawHtml?: string | undefined;
        links?: string[] | undefined;
        images?: string[] | undefined;
        screenshot?: string | undefined;
        metadata?: {
            [x: string]: import("hono/utils/types").JSONValue;
        } | undefined;
        warning?: string | undefined;
        branding?: {
            [x: string]: import("hono/utils/types").JSONValue;
        } | undefined;
    }[];
    next?: string | undefined;
    warning?: string | undefined;
    error?: string | undefined;
    cacheState: "hit" | "miss";
    cachedAt: string;
    concurrencyLimited: boolean;
}, 200, "json">)>;
export declare function crawlCancelController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 404, "json">) | (Response & import("hono").TypedResponse<{
    success: boolean;
    status?: "cancelled" | undefined;
    error?: string | undefined;
}, 409, "json">) | (Response & import("hono").TypedResponse<{
    success: boolean;
    status?: "cancelled" | undefined;
    error?: string | undefined;
}, 200, "json">)>;
export declare function ongoingCrawlsController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: boolean;
    crawls: {
        id: string;
        teamId: string;
        url: string;
        created_at: string;
        options: {
            includePaths?: string[] | undefined;
            excludePaths?: string[] | undefined;
            maxDiscoveryDepth?: number | undefined;
            limit?: number | undefined;
            crawlEntireDomain?: boolean | undefined;
            allowExternalLinks?: boolean | undefined;
            allowSubdomains?: boolean | undefined;
            ignoreRobotsTxt?: boolean | undefined;
            sitemap?: "skip" | "include" | "only" | undefined;
            deduplicateSimilarURLs?: boolean | undefined;
            ignoreQueryParameters?: boolean | undefined;
            regexOnFullURL?: boolean | undefined;
            delay?: number | undefined;
            maxConcurrency?: number | undefined;
            scrapeOptions?: {
                screenshotOptions?: {
                    fullPage?: boolean | undefined;
                    viewport?: {
                        width: number;
                        height: number;
                    } | undefined;
                    quality?: number | undefined;
                } | undefined;
                formats?: import("../contracts/scrape").ScrapeFormat[] | undefined;
                onlyMainContent?: boolean | undefined;
                includeTags?: string[] | undefined;
                excludeTags?: string[] | undefined;
                maxAge?: number | undefined;
                waitFor?: number | undefined;
                mobile?: boolean | undefined;
                skipTlsVerification?: boolean | undefined;
                timeout?: number | undefined;
                removeBase64Images?: boolean | undefined;
                blockAds?: boolean | undefined;
                proxy?: "basic" | "enhanced" | "auto" | undefined;
                storeInCache?: boolean | undefined;
                zeroDataRetention?: boolean | undefined;
                headers?: {
                    [x: string]: string;
                } | undefined;
                actions?: import("hono/utils/types").JSONValue[] | undefined;
                location?: {
                    country?: string | undefined;
                    languages?: string[] | undefined;
                } | undefined;
                useFireEngine?: boolean | undefined;
                engine?: "native" | "cloudflare" | "auto" | undefined;
                citations?: boolean | undefined;
                jsonOptions?: {
                    prompt?: string | undefined;
                    schema?: {
                        [x: string]: import("hono/utils/types").JSONValue;
                    } | undefined;
                } | undefined;
            } | undefined;
        };
    }[];
}, 200, "json">)>;
export declare function crawlErrorsController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 404, "json">) | (Response & import("hono").TypedResponse<{
    success?: boolean | undefined;
    errors: {
        id?: string | undefined;
        timestamp?: string | undefined;
        url: string;
        code?: string | undefined;
        error: string;
    }[];
    robotsBlocked: string[];
}, 200, "json">)>;
//# sourceMappingURL=crawl.d.ts.map