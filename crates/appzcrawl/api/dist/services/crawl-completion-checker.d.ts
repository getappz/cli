/**
 * Crawl completion checker: detects when all URLs in a fan-out crawl have been processed.
 *
 * For high-volume crawls using the SCRAPE_QUEUE fan-out pattern:
 * - crawl-runner discovers URLs and enqueues them to SCRAPE_QUEUE
 * - scrape-consumer processes each URL independently
 * - This checker polls D1 to detect when completed_count >= total_count
 *
 * Can be called:
 * 1. After each scrape-consumer batch completes
 * 2. Via a scheduled cron trigger
 * 3. By the status endpoint when queried
 */
/**
 * Check if a crawl job has completed (all URLs processed).
 * If complete, updates status to "completed" and sends webhook.
 *
 * @returns true if the crawl was just marked as completed
 */
export declare function checkCrawlCompletion(db: D1Database, crawlId: string): Promise<boolean>;
/**
 * Check completion for all "scraping" crawls (called by cron).
 * Returns the number of crawls that were marked as completed.
 */
export declare function checkAllCrawlCompletions(db: D1Database): Promise<number>;
/**
 * Get the current progress of a crawl (for status endpoint).
 */
export interface CrawlProgress {
    status: string;
    completed: number;
    total: number;
    /** Percentage complete (0-100) */
    percentage: number;
    /** Estimated time remaining in seconds (null if unknown) */
    etaSeconds: number | null;
}
export declare function getCrawlProgress(db: D1Database, crawlId: string): Promise<CrawlProgress | null>;
//# sourceMappingURL=crawl-completion-checker.d.ts.map