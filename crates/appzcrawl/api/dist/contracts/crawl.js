/**
 * Firecrawl-compatible crawl API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (crawlerOptions, crawlRequestSchema).
 * Uses Cloudflare D1 + Queues instead of Redis + BullMQ.
 */
export const CRAWLER_DEFAULTS = {
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
// Parse & validate crawl request body
// ---------------------------------------------------------------------------
export function parseCrawlRequestBody(body) {
    if (body === null || typeof body !== "object") {
        return { ok: false, error: "Invalid JSON body; expected object" };
    }
    const raw = body;
    const url = raw.url;
    if (typeof url !== "string" || !url.trim()) {
        return { ok: false, error: "Missing or invalid url in body" };
    }
    // Validate includePaths/excludePaths regex
    const includePaths = parseStringArray(raw.includePaths);
    for (const p of includePaths) {
        try {
            new RegExp(p);
        }
        catch (e) {
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
        }
        catch (e) {
            return {
                ok: false,
                error: `Invalid regex in excludePaths: ${e instanceof Error ? e.message : p}`,
            };
        }
    }
    const limit = typeof raw.limit === "number" && raw.limit > 0
        ? Math.min(raw.limit, CRAWLER_DEFAULTS.limit)
        : CRAWLER_DEFAULTS.limit;
    const sitemap = raw.sitemap === "skip" ||
        raw.sitemap === "include" ||
        raw.sitemap === "only"
        ? raw.sitemap
        : CRAWLER_DEFAULTS.sitemap;
    const data = {
        url: url.trim(),
        includePaths,
        excludePaths,
        maxDiscoveryDepth: typeof raw.maxDiscoveryDepth === "number" && raw.maxDiscoveryDepth >= 0
            ? raw.maxDiscoveryDepth
            : undefined,
        limit,
        crawlEntireDomain: Boolean(raw.crawlEntireDomain),
        allowExternalLinks: typeof raw.allowExternalLinks === "boolean"
            ? raw.allowExternalLinks
            : CRAWLER_DEFAULTS.allowExternalLinks,
        allowSubdomains: typeof raw.allowSubdomains === "boolean"
            ? raw.allowSubdomains
            : CRAWLER_DEFAULTS.allowSubdomains,
        ignoreRobotsTxt: typeof raw.ignoreRobotsTxt === "boolean"
            ? raw.ignoreRobotsTxt
            : CRAWLER_DEFAULTS.ignoreRobotsTxt,
        sitemap,
        deduplicateSimilarURLs: typeof raw.deduplicateSimilarURLs === "boolean"
            ? raw.deduplicateSimilarURLs
            : CRAWLER_DEFAULTS.deduplicateSimilarURLs,
        ignoreQueryParameters: typeof raw.ignoreQueryParameters === "boolean"
            ? raw.ignoreQueryParameters
            : CRAWLER_DEFAULTS.ignoreQueryParameters,
        regexOnFullURL: typeof raw.regexOnFullURL === "boolean"
            ? raw.regexOnFullURL
            : CRAWLER_DEFAULTS.regexOnFullURL,
        delay: typeof raw.delay === "number" && raw.delay > 0 ? raw.delay : undefined,
        scrapeOptions: raw.scrapeOptions && typeof raw.scrapeOptions === "object"
            ? raw.scrapeOptions
            : undefined,
        webhook: typeof raw.webhook === "string" ? raw.webhook : undefined,
        maxConcurrency: typeof raw.maxConcurrency === "number" && raw.maxConcurrency > 0
            ? raw.maxConcurrency
            : undefined,
        zeroDataRetention: Boolean(raw.zeroDataRetention),
        origin: typeof raw.origin === "string" ? raw.origin : "api",
    };
    return { ok: true, data };
}
function parseStringArray(val) {
    if (!Array.isArray(val))
        return [];
    return val.filter((v) => typeof v === "string");
}
//# sourceMappingURL=crawl.js.map