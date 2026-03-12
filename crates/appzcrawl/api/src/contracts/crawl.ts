/**
 * Firecrawl-compatible crawl API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (crawlerOptions, crawlRequestSchema).
 * Uses Cloudflare D1 + Queues instead of Redis + BullMQ.
 */

import type { ScrapeRequestBody } from "./scrape";

// ---------------------------------------------------------------------------
// Crawler options (Firecrawl-compatible)
// ---------------------------------------------------------------------------

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

export const CRAWLER_DEFAULTS: Required<
  Pick<
    CrawlerOptions,
    | "includePaths"
    | "excludePaths"
    | "limit"
    | "allowExternalLinks"
    | "allowSubdomains"
    | "ignoreRobotsTxt"
    | "sitemap"
    | "deduplicateSimilarURLs"
    | "ignoreQueryParameters"
    | "regexOnFullURL"
  >
> = {
  includePaths: [],
  excludePaths: [],
  limit: 10000,
  allowExternalLinks: false,
  allowSubdomains: false,
  ignoreRobotsTxt: false,
  sitemap: "include",
  deduplicateSimilarURLs: true,
  ignoreQueryParameters: false,
  regexOnFullURL: false,
};

// ---------------------------------------------------------------------------
// Crawl request body (Firecrawl /v2/crawl compatible)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Crawl response types (Firecrawl-compatible)
// ---------------------------------------------------------------------------

/** Crawl job status (Firecrawl-compatible). */
export type CrawlJobStatus =
  | "pending"
  | "scraping"
  | "completed"
  | "cancelled"
  | "failed";

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

// ---------------------------------------------------------------------------
// Parse & validate crawl request body
// ---------------------------------------------------------------------------

export function parseCrawlRequestBody(
  body: unknown,
): { ok: true; data: CrawlRequestBody } | { ok: false; error: string } {
  if (body === null || typeof body !== "object") {
    return { ok: false, error: "Invalid JSON body; expected object" };
  }
  const raw = body as Record<string, unknown>;

  const url = raw.url;
  if (typeof url !== "string" || !url.trim()) {
    return { ok: false, error: "Missing or invalid url in body" };
  }

  // Validate includePaths/excludePaths regex
  const includePaths = parseStringArray(raw.includePaths);
  for (const p of includePaths) {
    try {
      new RegExp(p);
    } catch (e) {
      return {
        ok: false,
        error: `Invalid regex in includePaths: ${e instanceof Error ? e.message : p}`,
      };
    }
  }
  const excludePaths = parseStringArray(raw.excludePaths);
  for (const p of excludePaths) {
    try {
      new RegExp(p);
    } catch (e) {
      return {
        ok: false,
        error: `Invalid regex in excludePaths: ${e instanceof Error ? e.message : p}`,
      };
    }
  }

  const limit =
    typeof raw.limit === "number" && raw.limit > 0
      ? Math.min(raw.limit, CRAWLER_DEFAULTS.limit)
      : CRAWLER_DEFAULTS.limit;

  const sitemap =
    raw.sitemap === "skip" ||
    raw.sitemap === "include" ||
    raw.sitemap === "only"
      ? raw.sitemap
      : CRAWLER_DEFAULTS.sitemap;

  const data: CrawlRequestBody = {
    url: url.trim(),
    includePaths,
    excludePaths,
    maxDiscoveryDepth:
      typeof raw.maxDiscoveryDepth === "number" && raw.maxDiscoveryDepth >= 0
        ? raw.maxDiscoveryDepth
        : undefined,
    limit,
    crawlEntireDomain: Boolean(raw.crawlEntireDomain),
    allowExternalLinks:
      typeof raw.allowExternalLinks === "boolean"
        ? raw.allowExternalLinks
        : CRAWLER_DEFAULTS.allowExternalLinks,
    allowSubdomains:
      typeof raw.allowSubdomains === "boolean"
        ? raw.allowSubdomains
        : CRAWLER_DEFAULTS.allowSubdomains,
    ignoreRobotsTxt:
      typeof raw.ignoreRobotsTxt === "boolean"
        ? raw.ignoreRobotsTxt
        : CRAWLER_DEFAULTS.ignoreRobotsTxt,
    sitemap,
    deduplicateSimilarURLs:
      typeof raw.deduplicateSimilarURLs === "boolean"
        ? raw.deduplicateSimilarURLs
        : CRAWLER_DEFAULTS.deduplicateSimilarURLs,
    ignoreQueryParameters:
      typeof raw.ignoreQueryParameters === "boolean"
        ? raw.ignoreQueryParameters
        : CRAWLER_DEFAULTS.ignoreQueryParameters,
    regexOnFullURL:
      typeof raw.regexOnFullURL === "boolean"
        ? raw.regexOnFullURL
        : CRAWLER_DEFAULTS.regexOnFullURL,
    delay:
      typeof raw.delay === "number" && raw.delay > 0 ? raw.delay : undefined,
    scrapeOptions:
      raw.scrapeOptions && typeof raw.scrapeOptions === "object"
        ? (raw.scrapeOptions as Omit<ScrapeRequestBody, "url">)
        : undefined,
    webhook: typeof raw.webhook === "string" ? raw.webhook : undefined,
    maxConcurrency:
      typeof raw.maxConcurrency === "number" && raw.maxConcurrency > 0
        ? raw.maxConcurrency
        : undefined,
    zeroDataRetention: Boolean(raw.zeroDataRetention),
    origin: typeof raw.origin === "string" ? raw.origin : "api",
  };

  return { ok: true, data };
}

function parseStringArray(val: unknown): string[] {
  if (!Array.isArray(val)) return [];
  return val.filter((v): v is string => typeof v === "string");
}
