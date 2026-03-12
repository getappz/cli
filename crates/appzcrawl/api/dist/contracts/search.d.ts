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
import type { ScrapeRequestBody } from "./scrape";
export type SearchSource = "web" | "images" | "news";
export interface SearchRequestBody {
    /** Search query string. */
    query: string;
    /** Maximum number of results per source (default: 10, max: 100). */
    limit?: number;
    /** Time-based search filter (e.g., "qdr:d" for past day). */
    tbs?: string;
    /** Search filter string. */
    filter?: string;
    /** Language code (default: "en"). */
    lang?: string;
    /** Country code for localized results. */
    country?: string;
    /** Location string for geo-targeted search results. */
    searchLocation?: string;
    /** Sources to search (default: ["web"]). */
    sources?: SearchSource[];
    /** Async scraping: return scrape IDs instead of inline content. */
    asyncScraping?: boolean;
    /** Request origin (api, dashboard, etc.). */
    origin?: string;
    /** Timeout in ms (default: 60000). */
    timeout?: number;
    /** Scrape options for fetched results. */
    scrapeOptions?: Omit<ScrapeRequestBody, "url">;
}
export interface SearchWebResult {
    url: string;
    title?: string;
    description?: string;
    content?: string;
    markdown?: string;
    html?: string;
    category?: string;
}
export interface SearchImageResult {
    url: string;
    title?: string;
    description?: string;
}
export interface SearchNewsResult {
    url: string;
    title?: string;
    description?: string;
    date?: string;
    content?: string;
    markdown?: string;
    html?: string;
}
export interface SearchResponse {
    success: true;
    data: {
        web?: SearchWebResult[];
        images?: SearchImageResult[];
        news?: SearchNewsResult[];
    };
    creditsUsed: number;
    id: string;
    cacheState: "hit" | "miss";
    cachedAt: string;
    concurrencyLimited: boolean;
}
export declare function parseSearchRequestBody(body: unknown): {
    ok: true;
    data: SearchRequestBody;
} | {
    ok: false;
    error: string;
};
//# sourceMappingURL=search.d.ts.map