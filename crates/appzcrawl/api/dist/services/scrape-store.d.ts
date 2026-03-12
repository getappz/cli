/**
 * Scrape store: D1 operations for scrapes table.
 * Stores individual scrape job metadata and results for status lookup.
 * Adapted from Firecrawl's supabase-jobs.ts pattern.
 */
import type { ScrapeRequestBody, ScrapeResponseData } from "../contracts/scrape";
export interface ScrapeRow {
    id: string;
    team_id: string;
    url: string;
    status: string;
    success: number;
    options: string | null;
    result: string | null;
    r2_key: string | null;
    error: string | null;
    zero_data_retention: number;
    created_at: string;
    updated_at: string;
}
export interface StoredScrape {
    id: string;
    teamId: string;
    url: string;
    status: "pending" | "completed" | "failed";
    success: boolean;
    options: Omit<ScrapeRequestBody, "url"> | null;
    result: ScrapeResponseData | null;
    r2Key: string | null;
    error: string | null;
    zeroDataRetention: boolean;
    createdAt: string;
    updatedAt: string;
}
/**
 * Create a new scrape job record (pending state).
 */
export declare function createScrapeJob(db: D1Database, params: {
    id: string;
    teamId: string;
    url: string;
    options?: Omit<ScrapeRequestBody, "url">;
    zeroDataRetention?: boolean;
}): Promise<void>;
/**
 * Get a scrape job by ID.
 */
export declare function getScrapeJob(db: D1Database, id: string): Promise<StoredScrape | null>;
/**
 * Get only team_id from a scrape by ID (lightweight query for auth check).
 * Matches Firecrawl's supabaseGetScrapeByIdOnlyData pattern.
 */
export declare function getScrapeTeamId(db: D1Database, id: string): Promise<{
    teamId: string;
} | null>;
/**
 * Update scrape job with success result.
 */
export declare function updateScrapeSuccess(db: D1Database, id: string, result: ScrapeResponseData, r2Key?: string): Promise<void>;
/**
 * Update scrape job with failure.
 */
export declare function updateScrapeFailure(db: D1Database, id: string, error: string): Promise<void>;
/**
 * Delete old scrapes (for cleanup).
 */
export declare function deleteOldScrapes(db: D1Database, olderThanMs: number): Promise<number>;
//# sourceMappingURL=scrape-store.d.ts.map