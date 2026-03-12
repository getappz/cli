/**
 * Search controller: Firecrawl-compatible /search endpoint.
 * Adapted from firecrawl/apps/api/src/controllers/v2/search.ts.
 *
 * Uses native Rust implementation for DuckDuckGo and SearXNG search.
 * - Primary: SearXNG (if SEARXNG_ENDPOINT env var is set)
 * - Fallback: DuckDuckGo HTML scraping
 *
 * NOTE: This implementation does NOT support:
 * - Fire Engine integration (would require separate service)
 * - Image search (DuckDuckGo images API)
 * - News search (would need separate API)
 * - Async result scraping (would need SCRAPE_QUEUE integration)
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function searchController(c: Context<AppEnv>): Promise<Response>;
//# sourceMappingURL=search.d.ts.map