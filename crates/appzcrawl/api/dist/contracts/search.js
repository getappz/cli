/**
 * Firecrawl-compatible search API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (searchRequestSchema).
 *
 * NOTE: Search implementation requires external service integration:
 * - Fire Engine (browser-based search)
 * - DuckDuckGo API
 * - SearXNG instance
 * For now, returns stub/empty results. Full implementation in future phase.
 */
// ---------------------------------------------------------------------------
// Parse & validate search request body
// ---------------------------------------------------------------------------
export function parseSearchRequestBody(body) {
    if (body === null || typeof body !== "object") {
        return { ok: false, error: "Invalid JSON body; expected object" };
    }
    const raw = body;
    const query = raw.query;
    if (typeof query !== "string" || !query.trim()) {
        return { ok: false, error: "Missing or invalid query in body" };
    }
    const limit = typeof raw.limit === "number" && raw.limit > 0 && raw.limit <= 100
        ? raw.limit
        : 10;
    const sources = [];
    if (Array.isArray(raw.sources)) {
        for (const s of raw.sources) {
            if (s === "web" || s === "images" || s === "news") {
                sources.push(s);
            }
        }
    }
    if (sources.length === 0) {
        sources.push("web");
    }
    const data = {
        query: query.trim(),
        limit,
        tbs: typeof raw.tbs === "string" ? raw.tbs : undefined,
        filter: typeof raw.filter === "string" ? raw.filter : undefined,
        lang: typeof raw.lang === "string" ? raw.lang : "en",
        country: typeof raw.country === "string" ? raw.country : undefined,
        searchLocation: typeof raw.location === "string" ? raw.location : undefined,
        sources,
        asyncScraping: Boolean(raw.asyncScraping),
        origin: typeof raw.origin === "string" ? raw.origin : "api",
        timeout: typeof raw.timeout === "number" && raw.timeout > 0 ? raw.timeout : 60000,
        // Scrape options (if provided)
        scrapeOptions: raw.scrapeOptions && typeof raw.scrapeOptions === "object"
            ? raw.scrapeOptions
            : undefined,
    };
    return { ok: true, data };
}
//# sourceMappingURL=search.js.map