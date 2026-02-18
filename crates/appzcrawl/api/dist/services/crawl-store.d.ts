/**
 * Crawl store: D1 operations for crawl_jobs and crawl_results.
 * Replaces Firecrawl's Redis crawl-redis.ts + BullMQ with D1.
 * Adapted from firecrawl/apps/api/src/lib/crawl-redis.ts.
 */
import type { CrawlDocument, CrawlerOptions, CrawlJobStatus } from "../contracts/crawl";
import type { ScrapeRequestBody } from "../contracts/scrape";
export interface CrawlJobRow {
    id: string;
    type: string;
    team_id: string;
    origin_url: string;
    status: string;
    crawler_options: string | null;
    scrape_options: string | null;
    robots_txt: string | null;
    completed_count: number;
    total_count: number;
    credits_billed: number;
    webhook: string | null;
    cancelled: number;
    zero_data_retention: number;
    created_at: string;
    updated_at: string;
    expires_at: string;
}
export interface CrawlResultRow {
    id: string;
    crawl_id: string;
    url: string;
    status: string;
    r2_key: string | null;
    document_json: string | null;
    error: string | null;
    code: string | null;
    status_code: number | null;
    created_at: string;
}
export interface StoredCrawl {
    id: string;
    type: "crawl" | "batch_scrape";
    teamId: string;
    originUrl: string;
    status: CrawlJobStatus;
    crawlerOptions: CrawlerOptions;
    scrapeOptions: Omit<ScrapeRequestBody, "url">;
    robotsTxt: string | null;
    completedCount: number;
    totalCount: number;
    creditsBilled: number;
    webhook: string | null;
    cancelled: boolean;
    zeroDataRetention: boolean;
    createdAt: string;
    updatedAt: string;
    expiresAt: string;
}
/** Create a new crawl job in D1 (replaces saveCrawl + markCrawlActive in Redis). */
export declare function createCrawlJob(db: D1Database, params: {
    id: string;
    type?: "crawl" | "batch_scrape";
    teamId: string;
    originUrl: string;
    crawlerOptions: CrawlerOptions;
    scrapeOptions: Omit<ScrapeRequestBody, "url">;
    webhook?: string;
    zeroDataRetention?: boolean;
    expiresAt?: string;
}): Promise<StoredCrawl>;
/** Fetch crawl job by ID (replaces getCrawl from Redis). */
export declare function getCrawlJob(db: D1Database, crawlId: string): Promise<StoredCrawl | null>;
/** Update crawl job status (replaces saveCrawl in Redis). */
export declare function updateCrawlStatus(db: D1Database, crawlId: string, status: CrawlJobStatus, updates?: {
    completedCount?: number;
    totalCount?: number;
    creditsBilled?: number;
    robotsTxt?: string;
    cancelled?: boolean;
}): Promise<void>;
/** Cancel a crawl job. */
export declare function cancelCrawlJob(db: D1Database, crawlId: string): Promise<void>;
/** List ongoing crawls for a team (replaces ongoingCrawlsController from Redis). */
export declare function getOngoingCrawls(db: D1Database, teamId: string): Promise<StoredCrawl[]>;
/** Insert a crawl result (per-URL scrape output). */
export declare function addCrawlResult(db: D1Database, params: {
    id: string;
    crawlId: string;
    url: string;
    status: "success" | "failed";
    documentJson?: string;
    r2Key?: string;
    error?: string;
    code?: string;
    statusCode?: number;
}): Promise<void>;
/** Get completed crawl results for a job, paginated. */
export declare function getCrawlResults(db: D1Database, crawlId: string, options?: {
    skip?: number;
    limit?: number;
}): Promise<CrawlDocument[]>;
/** Get failed results (errors) for a crawl. */
export declare function getCrawlErrors(db: D1Database, crawlId: string): Promise<Array<{
    id: string;
    url: string;
    error: string;
    code: string | null;
    timestamp: string;
}>>;
/** Increment completed count atomically. */
export declare function incrementCompleted(db: D1Database, crawlId: string, by?: number): Promise<void>;
/** Set total count (called after URL discovery). */
export declare function setTotalCount(db: D1Database, crawlId: string, total: number): Promise<void>;
/** Check if a crawl has been cancelled (quick read). */
export declare function isCrawlCancelled(db: D1Database, crawlId: string): Promise<boolean>;
/** Get URLs blocked by robots.txt during a crawl (Firecrawl-compatible). */
export declare function getRobotsBlocked(db: D1Database, crawlId: string): Promise<string[]>;
/** Add a URL blocked by robots.txt (replaces Redis SADD crawl:id:robots_blocked). */
export declare function addRobotsBlocked(db: D1Database, crawlId: string, url: string): Promise<void>;
//# sourceMappingURL=crawl-store.d.ts.map