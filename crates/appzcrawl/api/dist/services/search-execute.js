/**
 * Search execution service.
 * Adapted from firecrawl/apps/api/src/search/execute.ts
 */
import { nativeSearch } from "./html-processor";
/**
 * Execute search using native Rust implementation (DuckDuckGo or SearXNG).
 * - Calls native container /search endpoint
 * - Returns structured response matching Firecrawl's SearchResponse
 * - Calculates credits based on results
 */
export async function executeSearch(options) {
    const { body, teamId: _teamId, env, logger } = options;
    const { query, limit = 5, sources = ["web"] } = body;
    // Buffer to fetch more results than needed (like Firecrawl does)
    const num_results_buffer = Math.floor(limit * 2);
    logger.info("Executing search via native container", {
        query,
        limit,
        sources,
    });
    // Call native Rust search (DuckDuckGo with SearXNG fallback)
    const searchResponse = await nativeSearch(env, {
        query,
        num_results: num_results_buffer,
        tbs: body.tbs,
        filter: body.filter,
        lang: body.lang,
        country: body.country,
        location: body.searchLocation,
        timeout_ms: body.timeout ?? 5000,
    });
    let totalResultsCount = 0;
    // Build response based on requested sources
    const now = new Date().toISOString();
    const response = {
        success: true,
        data: {},
        creditsUsed: 0,
        id: crypto.randomUUID(),
        cacheState: "miss",
        cachedAt: now,
        concurrencyLimited: false,
    };
    // Handle web results
    if (sources.includes("web") &&
        searchResponse.web &&
        searchResponse.web.length > 0) {
        const webResults = searchResponse.web.slice(0, limit);
        response.data.web = webResults.map((result, index) => ({
            url: result.url,
            title: result.title,
            description: result.description,
            position: index + 1,
            category: "web",
        }));
        totalResultsCount += webResults.length;
    }
    // Images and news sources are not supported yet (would require different search APIs)
    if (sources.includes("images")) {
        response.data.images = [];
    }
    if (sources.includes("news")) {
        response.data.news = [];
    }
    // Calculate credits (Firecrawl uses 2 credits per 10 results, ZDR uses 10 per 10)
    const creditsPerTenResults = 2; // Standard rate (not ZDR)
    const searchCredits = Math.ceil(totalResultsCount / 10) * creditsPerTenResults;
    response.creditsUsed = searchCredits;
    logger.info("Search completed", {
        totalResults: totalResultsCount,
        creditsUsed: searchCredits,
    });
    return {
        response,
        totalResultsCount,
        searchCredits,
    };
}
//# sourceMappingURL=search-execute.js.map