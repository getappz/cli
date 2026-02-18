/**
 * Crawl visited URLs: D1-backed persistent URL dedup for crawls.
 * Replaces Firecrawl's Redis crawl:{id}:visited set.
 *
 * Benefits over in-memory Set:
 *   - Persists across queue retries and worker restarts
 *   - Shared across parallel SCRAPE_QUEUE consumers
 *   - Cleaned up automatically by the scheduled cleanup service
 */
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
/** SHA-256 hash of URL (hex, first 32 chars for compactness). */
async function hashUrl(url) {
    const data = new TextEncoder().encode(url);
    const hash = await crypto.subtle.digest("SHA-256", data);
    const hex = Array.from(new Uint8Array(hash))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    return hex.slice(0, 32);
}
/**
 * Normalize URL for dedup comparison.
 * Adapted from Firecrawl's normalizeURL in crawl-redis.ts.
 * Strips trailing slashes, lowercases hostname, optionally removes query/hash.
 */
export function normalizeUrlForDedup(url, opts) {
    try {
        const u = new URL(url);
        u.hostname = u.hostname.toLowerCase();
        // Remove trailing slash from pathname
        if (u.pathname.endsWith("/") && u.pathname !== "/") {
            u.pathname = u.pathname.slice(0, -1);
        }
        // Optionally strip query parameters
        if (opts?.ignoreQueryParameters) {
            u.search = "";
        }
        // Always remove hash
        u.hash = "";
        return u.toString();
    }
    catch {
        return url;
    }
}
// ---------------------------------------------------------------------------
// D1 operations
// ---------------------------------------------------------------------------
/**
 * Try to lock a URL for a crawl (add to visited set).
 * Returns true if the URL was newly added, false if already visited.
 * Uses INSERT OR IGNORE to handle races between parallel consumers.
 */
export async function lockUrl(db, crawlId, url, opts) {
    const normalized = normalizeUrlForDedup(url, opts);
    const urlHash = await hashUrl(normalized);
    const now = new Date().toISOString();
    try {
        const result = await db
            .prepare(`INSERT OR IGNORE INTO crawl_visited_urls (crawl_id, url_hash, url, created_at)
         VALUES (?, ?, ?, ?)`)
            .bind(crawlId, urlHash, url, now)
            .run();
        // If rows_written is 0, the URL was already visited (IGNORE fired)
        return (result.meta?.changes ?? 0) > 0;
    }
    catch (e) {
        // Unique constraint or other error — treat as already visited
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes("UNIQUE") || msg.includes("constraint")) {
            return false;
        }
        throw e;
    }
}
/**
 * Try to lock multiple URLs at once. Returns the subset of URLs that were
 * newly added (not previously visited).
 */
export async function lockUrls(db, crawlId, urls, opts) {
    const newUrls = [];
    for (const url of urls) {
        const isNew = await lockUrl(db, crawlId, url, opts);
        if (isNew) {
            newUrls.push(url);
        }
    }
    return newUrls;
}
/**
 * Check if a URL has been visited without locking it.
 */
export async function isUrlVisited(db, crawlId, url, opts) {
    const normalized = normalizeUrlForDedup(url, opts);
    const urlHash = await hashUrl(normalized);
    const row = await db
        .prepare("SELECT 1 FROM crawl_visited_urls WHERE crawl_id = ? AND url_hash = ? LIMIT 1")
        .bind(crawlId, urlHash)
        .first();
    return row !== null;
}
/**
 * Get count of visited URLs for a crawl (used for limit checking).
 */
export async function getVisitedCount(db, crawlId) {
    const row = await db
        .prepare("SELECT COUNT(*) as cnt FROM crawl_visited_urls WHERE crawl_id = ?")
        .bind(crawlId)
        .first();
    return row?.cnt ?? 0;
}
//# sourceMappingURL=crawl-visited.js.map