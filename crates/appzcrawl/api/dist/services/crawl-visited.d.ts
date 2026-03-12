/**
 * Crawl visited URLs: D1-backed persistent URL dedup for crawls.
 * Replaces Firecrawl's Redis crawl:{id}:visited set.
 *
 * Benefits over in-memory Set:
 *   - Persists across queue retries and worker restarts
 *   - Shared across parallel SCRAPE_QUEUE consumers
 *   - Cleaned up automatically by the scheduled cleanup service
 */
/**
 * Normalize URL for dedup comparison.
 * Adapted from Firecrawl's normalizeURL in crawl-redis.ts.
 * Strips trailing slashes, lowercases hostname, optionally removes query/hash.
 */
export declare function normalizeUrlForDedup(url: string, opts?: {
    ignoreQueryParameters?: boolean;
}): string;
/**
 * Try to lock a URL for a crawl (add to visited set).
 * Returns true if the URL was newly added, false if already visited.
 * Uses INSERT OR IGNORE to handle races between parallel consumers.
 */
export declare function lockUrl(db: D1Database, crawlId: string, url: string, opts?: {
    ignoreQueryParameters?: boolean;
}): Promise<boolean>;
/**
 * Try to lock multiple URLs at once. Returns the subset of URLs that were
 * newly added (not previously visited).
 */
export declare function lockUrls(db: D1Database, crawlId: string, urls: string[], opts?: {
    ignoreQueryParameters?: boolean;
}): Promise<string[]>;
/**
 * Check if a URL has been visited without locking it.
 */
export declare function isUrlVisited(db: D1Database, crawlId: string, url: string, opts?: {
    ignoreQueryParameters?: boolean;
}): Promise<boolean>;
/**
 * Get count of visited URLs for a crawl (used for limit checking).
 */
export declare function getVisitedCount(db: D1Database, crawlId: string): Promise<number>;
//# sourceMappingURL=crawl-visited.d.ts.map