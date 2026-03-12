/**
 * Scheduled cleanup service for expired data across all tables.
 * Mirrors Firecrawl's pg_cron cleanup jobs using Cloudflare Cron Triggers.
 *
 * Retention policies:
 *   - crawl_jobs + crawl_results + crawl_visited_urls: expires_at column (default 24h)
 *   - extract_jobs + agent_jobs:                       expires_at column (default 24h)
 *   - scrapes:                                         24h after created_at
 *   - scrape_cache:                                    expires_at_ms column
 *   - webhook_logs:                                    7 days
 *   - request_log:                                     30 days
 *   - billing_log:                                     90 days
 *   - R2 objects: orphaned objects deleted alongside their D1 rows
 */
import { logger } from "../lib/logger";
import { crawlPrefix, deleteR2Objects, deleteR2Prefix } from "../lib/r2-keys";
// ---------------------------------------------------------------------------
// Retention constants
// ---------------------------------------------------------------------------
const SCRAPE_TTL_MS = 24 * 60 * 60 * 1000;
const WEBHOOK_LOG_TTL_MS = 7 * 24 * 60 * 60 * 1000;
const REQUEST_LOG_TTL_MS = 30 * 24 * 60 * 60 * 1000;
const BILLING_LOG_TTL_MS = 90 * 24 * 60 * 60 * 1000;
/** Maximum rows to delete per cleanup pass (avoid D1 row-write limits). */
const DELETE_BATCH_SIZE = 500;
// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------
export async function runScheduledCleanup(db, bucket) {
    const results = [];
    const tasks = [
        { name: "expired_crawls", fn: () => cleanExpiredCrawls(db, bucket) },
        {
            name: "expired_extracts",
            fn: () => cleanExpiredJobsWithR2(db, bucket, "extract_jobs"),
        },
        {
            name: "expired_agents",
            fn: () => cleanExpiredJobsWithR2(db, bucket, "agent_jobs"),
        },
        {
            name: "old_scrapes",
            fn: () => cleanOldJobsWithR2(db, bucket, "scrapes", SCRAPE_TTL_MS),
        },
        { name: "expired_cache", fn: () => cleanExpiredCache(db, bucket) },
        {
            name: "old_webhook_logs",
            fn: () => cleanOldRows(db, "webhook_logs", WEBHOOK_LOG_TTL_MS),
        },
        {
            name: "old_request_log",
            fn: () => cleanOldRows(db, "request_log", REQUEST_LOG_TTL_MS),
        },
        {
            name: "old_billing_log",
            fn: () => cleanOldRows(db, "billing_log", BILLING_LOG_TTL_MS),
        },
    ];
    for (const task of tasks) {
        try {
            const deleted = await task.fn();
            results.push({ task: task.name, deleted });
        }
        catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            logger.error(`[cleanup] ${task.name} failed`, { error: msg });
            results.push({ task: task.name, deleted: 0, error: msg });
        }
    }
    const totalDeleted = results.reduce((sum, r) => sum + r.deleted, 0);
    if (totalDeleted > 0) {
        logger.info("[cleanup] scheduled cleanup complete", { results });
    }
}
// ---------------------------------------------------------------------------
// Generic: expired jobs with R2 (extract_jobs, agent_jobs)
// ---------------------------------------------------------------------------
/** Delete expired jobs from a table that uses expires_at + optional R2 storage. */
async function cleanExpiredJobsWithR2(db, bucket, table) {
    const now = new Date().toISOString();
    const { results: rows } = await db
        .prepare(`SELECT id, r2_key FROM ${table}
       WHERE expires_at < ? AND status IN ('completed', 'failed')
       LIMIT ?`)
        .bind(now, DELETE_BATCH_SIZE)
        .all();
    if (!rows || rows.length === 0)
        return 0;
    return deleteRowsWithR2(db, bucket, table, rows);
}
// ---------------------------------------------------------------------------
// Generic: old rows with R2 (scrapes — uses created_at instead of expires_at)
// ---------------------------------------------------------------------------
/** Delete old rows from a table that uses created_at + optional R2 storage. */
async function cleanOldJobsWithR2(db, bucket, table, ttlMs) {
    const cutoff = new Date(Date.now() - ttlMs).toISOString();
    const { results: rows } = await db
        .prepare(`SELECT id, r2_key FROM ${table} WHERE created_at < ? LIMIT ?`)
        .bind(cutoff, DELETE_BATCH_SIZE)
        .all();
    if (!rows || rows.length === 0)
        return 0;
    return deleteRowsWithR2(db, bucket, table, rows);
}
// ---------------------------------------------------------------------------
// Shared: delete rows + their R2 objects
// ---------------------------------------------------------------------------
async function deleteRowsWithR2(db, bucket, table, rows) {
    const r2Keys = rows.filter((r) => r.r2_key).map((r) => r.r2_key);
    if (r2Keys.length > 0) {
        await deleteR2Objects(bucket, r2Keys);
    }
    const ids = rows.map((r) => r.id);
    const placeholders = ids.map(() => "?").join(", ");
    await db
        .prepare(`DELETE FROM ${table} WHERE id IN (${placeholders})`)
        .bind(...ids)
        .run();
    return ids.length;
}
// ---------------------------------------------------------------------------
// Crawl cleanup (special case — cascading deletes)
// ---------------------------------------------------------------------------
async function cleanExpiredCrawls(db, bucket) {
    const now = new Date().toISOString();
    const { results: expiredRows } = await db
        .prepare(`SELECT id FROM crawl_jobs
       WHERE expires_at < ? AND status IN ('completed', 'cancelled', 'failed')
       LIMIT ?`)
        .bind(now, DELETE_BATCH_SIZE)
        .all();
    if (!expiredRows || expiredRows.length === 0)
        return 0;
    for (const { id: crawlId } of expiredRows) {
        // Collect and delete R2 objects
        const { results: r2Rows } = await db
            .prepare("SELECT r2_key FROM crawl_results WHERE crawl_id = ? AND r2_key IS NOT NULL")
            .bind(crawlId)
            .all();
        const r2Keys = (r2Rows ?? []).map((r) => r.r2_key);
        if (r2Keys.length > 0) {
            await deleteR2Objects(bucket, r2Keys);
        }
        await deleteR2Prefix(bucket, crawlPrefix(crawlId));
        // Cascade delete child rows
        for (const childTable of [
            "crawl_results",
            "crawl_robots_blocked",
            "crawl_visited_urls",
        ]) {
            await db
                .prepare(`DELETE FROM ${childTable} WHERE crawl_id = ?`)
                .bind(crawlId)
                .run();
        }
        await db.prepare("DELETE FROM crawl_jobs WHERE id = ?").bind(crawlId).run();
    }
    return expiredRows.length;
}
// ---------------------------------------------------------------------------
// Cache cleanup
// ---------------------------------------------------------------------------
async function cleanExpiredCache(db, bucket) {
    const nowMs = Date.now();
    const { results: rows } = await db
        .prepare(`SELECT id, r2_key FROM scrape_cache WHERE expires_at_ms < ? LIMIT ?`)
        .bind(nowMs, DELETE_BATCH_SIZE)
        .all();
    if (!rows || rows.length === 0)
        return 0;
    const r2Keys = rows.map((r) => r.r2_key);
    if (r2Keys.length > 0) {
        await deleteR2Objects(bucket, r2Keys);
    }
    const ids = rows.map((r) => r.id);
    const placeholders = ids.map(() => "?").join(", ");
    await db
        .prepare(`DELETE FROM scrape_cache WHERE id IN (${placeholders})`)
        .bind(...ids)
        .run();
    return ids.length;
}
// ---------------------------------------------------------------------------
// Generic row cleanup (no R2)
// ---------------------------------------------------------------------------
async function cleanOldRows(db, table, ttlMs) {
    const cutoff = new Date(Date.now() - ttlMs).toISOString();
    const result = await db
        .prepare(`DELETE FROM ${table} WHERE created_at < ?`)
        .bind(cutoff)
        .run();
    return result.meta?.changes ?? 0;
}
//# sourceMappingURL=cleanup.js.map