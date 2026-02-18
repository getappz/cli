/**
 * Batch scrape controller: Firecrawl-compatible /batch/scrape endpoint.
 * Adapted from firecrawl/apps/api/src/controllers/v2/batch-scrape.ts.
 *
 * Flow: POST /batch/scrape → D1 insert → enqueue URLs to SCRAPE_QUEUE → return 200 {id, url}
 * Status/cancel reuse existing crawl controllers (they already support isBatchScrape).
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function batchScrapeController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    id: string;
    url: string;
    invalidURLs?: string[] | undefined;
}, 200, "json">)>;
//# sourceMappingURL=batch-scrape.d.ts.map