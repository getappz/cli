/**
 * Firecrawl-compatible crawl API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (crawlerOptions, crawlRequestSchema).
 * Uses Cloudflare D1 + Queues instead of Redis + BullMQ.
 */
import type { ScrapeRequestBody } from "./scrape";
export interface CrawlerOptions {
    /** Regex patterns for paths to include (default []). */
    includePaths?: string[];
    /** Regex patterns for paths to exclude (default []). */
    excludePaths?: string[];
    /** Max depth of discovery hops from seed URL. */
    maxDiscoveryDepth?: number;
    /** Max number of pages to crawl (default 10000). */
    limit?: number;
    /** Crawl the entire base domain (ignores subpath hierarchy). */
    crawlEntireDomain?: boolean;
    /** Follow links to external domains. Default: false. */
    allowExternalLinks?: boolean;
    /** Follow links to subdomains (same registrable domain). Default: false. */
    allowSubdomains?: boolean;
    /** Ignore robots.txt directives. Default: false. */
    ignoreRobotsTxt?: boolean;
    /** Sitemap handling: skip | include | only. Default: include. */
    sitemap?: "skip" | "include" | "only";
    /** Deduplicate similar URLs. Default: true. */
    deduplicateSimilarURLs?: boolean;
    /** Ignore query parameters in URLs. Default: false. */
    ignoreQueryParameters?: boolean;
    /** Apply include/exclude regex on full URL (not just path). Default: false. */
    regexOnFullURL?: boolean;
    /** Delay between scrapes in ms. */
    delay?: number;
    /** Max concurrent scrapes within a crawl (default: 5). */
    maxConcurrency?: number;
}
export declare const CRAWLER_DEFAULTS: Required<Pick<CrawlerOptions, "includePaths" | "excludePaths" | "limit" | "allowExternalLinks" | "allowSubdomains" | "ignoreRobotsTxt" | "sitemap" | "deduplicateSimilarURLs" | "ignoreQueryParameters" | "regexOnFullURL">>;
export interface CrawlRequestBody extends CrawlerOptions {
    /** Required: seed URL. */
    url: string;
    /** Per-page scrape options (formats, onlyMainContent, tags, etc.). */
    scrapeOptions?: Omit<ScrapeRequestBody, "url">;
    /** Webhook to notify on completion. */
    webhook?: string;
    /** Max concurrent scrapes (optional). */
    maxConcurrency?: number;
    /** Zero data retention. */
    zeroDataRetention?: boolean;
    /** Request origin (api, dashboard, etc.). */
    origin?: string;
}
/** Crawl job status (Firecrawl-compatible). */
export type CrawlJobStatus = "pending" | "scraping" | "completed" | "cancelled" | "failed";
/** Scraped document stored per URL in a crawl. */
export interface CrawlDocument {
    url: string;
    markdown?: string;
    html?: string;
    rawHtml?: string;
    links?: string[];
    images?: string[];
    screenshot?: string;
    metadata?: Record<string, unknown>;
    warning?: string;
    branding?: Record<string, unknown>;
}
/** POST /v2/crawl response (async). */
export interface CrawlStartResponse {
    success: true;
    id: string;
    url: string;
}
/** GET /v2/crawl/:jobId response (status poll). */
export interface CrawlStatusResponse {
    success: boolean;
    status: CrawlJobStatus;
    completed: number;
    total: number;
    creditsUsed: number;
    expiresAt?: string;
    data: CrawlDocument[];
    next?: string;
    warning?: string;
    error?: string;
    cacheState: "hit" | "miss";
    cachedAt: string;
    concurrencyLimited: boolean;
}
/** DELETE /v2/crawl/:jobId response. */
export interface CrawlCancelResponse {
    success: boolean;
    status?: "cancelled";
    error?: string;
}
/** GET /v2/crawl/ongoing response. */
export interface CrawlOngoingResponse {
    success: boolean;
    crawls: Array<{
        id: string;
        teamId: string;
        url: string;
        created_at: string;
        options: CrawlerOptions & {
            scrapeOptions?: Omit<ScrapeRequestBody, "url">;
        };
    }>;
}
/** GET /v2/crawl/:jobId/errors response. */
export interface CrawlErrorsResponse {
    success?: boolean;
    errors: Array<{
        id?: string;
        timestamp?: string;
        url: string;
        code?: string;
        error: string;
    }>;
    robotsBlocked: string[];
}
export declare function parseCrawlRequestBody(body: unknown): {
    ok: true;
    data: CrawlRequestBody;
} | {
    ok: false;
    error: string;
};
//# sourceMappingURL=crawl.d.ts.map