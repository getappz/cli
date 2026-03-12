/**
 * Scrape store: D1 operations for scrapes table.
 * Stores individual scrape job metadata and results for status lookup.
 * Adapted from Firecrawl's supabase-jobs.ts pattern.
 */
import { logger } from "../lib/logger";
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function rowToScrape(row) {
    return {
        id: row.id,
        teamId: row.team_id,
        url: row.url,
        status: row.status,
        success: Boolean(row.success),
        options: row.options ? JSON.parse(row.options) : null,
        result: row.result ? JSON.parse(row.result) : null,
        r2Key: row.r2_key,
        error: row.error,
        zeroDataRetention: Boolean(row.zero_data_retention),
        createdAt: row.created_at,
        updatedAt: row.updated_at,
    };
}
// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------
/**
 * Create a new scrape job record (pending state).
 */
export async function createScrapeJob(db, params) {
    const now = new Date().toISOString();
    await db
        .prepare(`INSERT INTO scrapes (id, team_id, url, status, success, options, zero_data_retention, created_at, updated_at)
       VALUES (?, ?, ?, 'pending', 0, ?, ?, ?, ?)`)
        .bind(params.id, params.teamId, params.url, params.options ? JSON.stringify(params.options) : null, params.zeroDataRetention ? 1 : 0, now, now)
        .run();
}
/**
 * Get a scrape job by ID.
 */
export async function getScrapeJob(db, id) {
    const result = await db
        .prepare("SELECT * FROM scrapes WHERE id = ?")
        .bind(id)
        .first();
    if (!result)
        return null;
    return rowToScrape(result);
}
/**
 * Get only team_id from a scrape by ID (lightweight query for auth check).
 * Matches Firecrawl's supabaseGetScrapeByIdOnlyData pattern.
 */
export async function getScrapeTeamId(db, id) {
    const result = await db
        .prepare("SELECT team_id FROM scrapes WHERE id = ?")
        .bind(id)
        .first();
    if (!result)
        return null;
    return { teamId: result.team_id };
}
/**
 * Update scrape job with success result.
 */
export async function updateScrapeSuccess(db, id, result, r2Key) {
    const now = new Date().toISOString();
    await db
        .prepare(`UPDATE scrapes
       SET status = 'completed', success = 1, result = ?, r2_key = ?, updated_at = ?
       WHERE id = ?`)
        .bind(JSON.stringify(result), r2Key ?? null, now, id)
        .run();
}
/**
 * Update scrape job with failure.
 */
export async function updateScrapeFailure(db, id, error) {
    const now = new Date().toISOString();
    await db
        .prepare(`UPDATE scrapes
       SET status = 'failed', success = 0, error = ?, updated_at = ?
       WHERE id = ?`)
        .bind(error, now, id)
        .run();
}
/**
 * Delete old scrapes (for cleanup).
 */
export async function deleteOldScrapes(db, olderThanMs) {
    const cutoff = new Date(Date.now() - olderThanMs).toISOString();
    const result = await db
        .prepare("DELETE FROM scrapes WHERE created_at < ?")
        .bind(cutoff)
        .run();
    const deleted = result.meta?.changes ?? 0;
    if (deleted > 0) {
        logger.info("[scrape-store] cleaned up old scrapes", { deleted });
    }
    return deleted;
}
//# sourceMappingURL=scrape-store.js.map