/**
 * Job enqueue stub. Replace with Cloudflare Queues + D1 when implementing scrape/crawl workers.
 * R2 (env.BUCKET) can be used by workers to store scraped content.
 */
export declare function enqueueScrapeJob(_jobId: string, _payload: Record<string, unknown>, _env: {
    DB: D1Database;
    BUCKET?: R2Bucket;
}): Promise<void>;
export declare function enqueueCrawlJob(_jobId: string, _payload: Record<string, unknown>, _env: {
    DB: D1Database;
    BUCKET?: R2Bucket;
}): Promise<void>;
//# sourceMappingURL=jobs.d.ts.map