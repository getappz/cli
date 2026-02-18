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

// ---------------------------------------------------------------------------
// Search request types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Search response types (Firecrawl-compatible)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Parse & validate search request body
// ---------------------------------------------------------------------------

export function parseSearchRequestBody(
  body: unknown,
): { ok: true; data: SearchRequestBody } | { ok: false; error: string } {
  if (body === null || typeof body !== "object") {
    return { ok: false, error: "Invalid JSON body; expected object" };
  }
  const raw = body as Record<string, unknown>;

  const query = raw.query;
  if (typeof query !== "string" || !query.trim()) {
    return { ok: false, error: "Missing or invalid query in body" };
  }

  const limit =
    typeof raw.limit === "number" && raw.limit > 0 && raw.limit <= 100
      ? raw.limit
      : 10;

  const sources: SearchSource[] = [];
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

  const data: SearchRequestBody = {
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
    timeout:
      typeof raw.timeout === "number" && raw.timeout > 0 ? raw.timeout : 60000,
    // Scrape options (if provided)
    scrapeOptions:
      raw.scrapeOptions && typeof raw.scrapeOptions === "object"
        ? (raw.scrapeOptions as Omit<ScrapeRequestBody, "url">)
        : undefined,
  };

  return { ok: true, data };
}
