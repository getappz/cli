/**
 * Firecrawl-compatible batch scrape API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (batchScrapeRequestSchema).
 */
import type { ScrapeRequestBody } from "./scrape";
export interface BatchScrapeRequestBody extends Omit<ScrapeRequestBody, "url"> {
    /** Array of URLs to scrape. */
    urls: string[];
    /** Webhook to notify on completion. */
    webhook?: string;
    /** Ignore invalid URLs and continue with valid ones. */
    ignoreInvalidURLs?: boolean;
    /** Request origin (api, dashboard, etc.). */
    origin?: string;
}
/** POST /v2/batch/scrape response. */
export interface BatchScrapeStartResponse {
    success: true;
    id: string;
    url: string;
    invalidURLs?: string[];
}
export declare function parseBatchScrapeRequestBody(body: unknown): {
    ok: true;
    data: BatchScrapeRequestBody;
} | {
    ok: false;
    error: string;
};
//# sourceMappingURL=batch-scrape.d.ts.map