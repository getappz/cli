/**
 * Crawl store: D1 operations for crawl_jobs and crawl_results.
 * Replaces Firecrawl's Redis crawl-redis.ts + BullMQ with D1.
 * Adapted from firecrawl/apps/api/src/lib/crawl-redis.ts.
 */
import { logger } from "../lib/logger";
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function rowToCrawl(row) {
    return {
        id: row.id,
        type: row.type || "crawl",
        teamId: row.team_id,
        originUrl: row.origin_url,
        status: row.status,
        crawlerOptions: row.crawler_options ? JSON.parse(row.crawler_options) : {},
        scrapeOptions: row.scrape_options ? JSON.parse(row.scrape_options) : {},
        robotsTxt: row.robots_txt,
        completedCount: row.completed_count,
        totalCount: row.total_count,
        creditsBilled: row.credits_billed,
        webhook: row.webhook,
        cancelled: Boolean(row.cancelled),
        zeroDataRetention: Boolean(row.zero_data_retention),
        createdAt: row.created_at,
        updatedAt: row.updated_at,
        expiresAt: row.expires_at,
    };
}
// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------
/** Create a new crawl job in D1 (replaces saveCrawl + markCrawlActive in Redis). */
export async function createCrawlJob(db, params) {
    const now = new Date().toISOString();
    const expiresAt = params.expiresAt ??
        new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
    const type = params.type ?? "crawl";
    await db
        .prepare(`INSERT INTO crawl_jobs
        (id, type, team_id, origin_url, status, crawler_options, scrape_options,
         webhook, zero_data_retention, created_at, updated_at, expires_at)
       VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)`)
        .bind(params.id, type, params.teamId, params.originUrl, JSON.stringify(params.crawlerOptions), JSON.stringify(params.scrapeOptions), params.webhook ?? null, params.zeroDataRetention ? 1 : 0, now, now, expiresAt)
        .run();
    return {
        id: params.id,
        type,
        teamId: params.teamId,
        originUrl: params.originUrl,
        status: "pending",
        crawlerOptions: params.crawlerOptions,
        scrapeOptions: params.scrapeOptions,
        robotsTxt: null,
        completedCount: 0,
        totalCount: 0,
        creditsBilled: 0,
        webhook: params.webhook ?? null,
        cancelled: false,
        zeroDataRetention: Boolean(params.zeroDataRetention),
        createdAt: now,
        updatedAt: now,
        expiresAt,
    };
}
/** Fetch crawl job by ID (replaces getCrawl from Redis). */
export async function getCrawlJob(db, crawlId) {
    const row = await db
        .prepare("SELECT * FROM crawl_jobs WHERE id = ? LIMIT 1")
        .bind(crawlId)
        .first();
    return row ? rowToCrawl(row) : null;
}
/** Update crawl job status (replaces saveCrawl in Redis). */
export async function updateCrawlStatus(db, crawlId, status, updates) {
    const now = new Date().toISOString();
    const sets = ["status = ?", "updated_at = ?"];
    const values = [status, now];
    if (updates?.completedCount !== undefined) {
        sets.push("completed_count = ?");
        values.push(updates.completedCount);
    }
    if (updates?.totalCount !== undefined) {
        sets.push("total_count = ?");
        values.push(updates.totalCount);
    }
    if (updates?.creditsBilled !== undefined) {
        sets.push("credits_billed = ?");
        values.push(updates.creditsBilled);
    }
    if (updates?.robotsTxt !== undefined) {
        sets.push("robots_txt = ?");
        values.push(updates.robotsTxt);
    }
    if (updates?.cancelled !== undefined) {
        sets.push("cancelled = ?");
        values.push(updates.cancelled ? 1 : 0);
    }
    values.push(crawlId);
    await db
        .prepare(`UPDATE crawl_jobs SET ${sets.join(", ")} WHERE id = ?`)
        .bind(...values)
        .run();
}
/** Cancel a crawl job. */
export async function cancelCrawlJob(db, crawlId) {
    await updateCrawlStatus(db, crawlId, "cancelled", { cancelled: true });
}
/** List ongoing crawls for a team (replaces ongoingCrawlsController from Redis). */
export async function getOngoingCrawls(db, teamId) {
    const { results } = await db
        .prepare(`SELECT * FROM crawl_jobs
       WHERE team_id = ? AND status IN ('pending', 'scraping')
       ORDER BY created_at DESC`)
        .bind(teamId)
        .all();
    return (results ?? []).map(rowToCrawl);
}
// ---------------------------------------------------------------------------
// Crawl results
// ---------------------------------------------------------------------------
/** Insert a crawl result (per-URL scrape output). */
export async function addCrawlResult(db, params) {
    const now = new Date().toISOString();
    await db
        .prepare(`INSERT INTO crawl_results
        (id, crawl_id, url, status, document_json, r2_key, error, code, status_code, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
        .bind(params.id, params.crawlId, params.url, params.status, params.documentJson ?? null, params.r2Key ?? null, params.error ?? null, params.code ?? null, params.statusCode ?? null, now)
        .run();
}
/** Get completed crawl results for a job, paginated. */
export async function getCrawlResults(db, crawlId, options = {}) {
    const skip = options.skip ?? 0;
    const limit = options.limit ?? 100;
    const { results } = await db
        .prepare(`SELECT * FROM crawl_results
       WHERE crawl_id = ? AND status = 'success'
       ORDER BY created_at ASC
       LIMIT ? OFFSET ?`)
        .bind(crawlId, limit, skip)
        .all();
    const docs = [];
    for (const row of results ?? []) {
        if (row.document_json) {
            try {
                docs.push(JSON.parse(row.document_json));
            }
            catch {
                logger.warn("[crawl-store] failed to parse document_json", {
                    resultId: row.id,
                });
            }
        }
    }
    return docs;
}
/** Get failed results (errors) for a crawl. */
export async function getCrawlErrors(db, crawlId) {
    const { results } = await db
        .prepare(`SELECT id, url, error, code, created_at FROM crawl_results
       WHERE crawl_id = ? AND status = 'failed'
       ORDER BY created_at ASC`)
        .bind(crawlId)
        .all();
    return (results ?? []).map((r) => ({
        id: r.id,
        url: r.url,
        error: r.error,
        code: r.code,
        timestamp: r.created_at,
    }));
}
/** Increment completed count atomically. */
export async function incrementCompleted(db, crawlId, by = 1) {
    await db
        .prepare(`UPDATE crawl_jobs
       SET completed_count = completed_count + ?, updated_at = ?
       WHERE id = ?`)
        .bind(by, new Date().toISOString(), crawlId)
        .run();
}
/** Set total count (called after URL discovery). */
export async function setTotalCount(db, crawlId, total) {
    await db
        .prepare(`UPDATE crawl_jobs SET total_count = ?, updated_at = ? WHERE id = ?`)
        .bind(total, new Date().toISOString(), crawlId)
        .run();
}
/** Check if a crawl has been cancelled (quick read). */
export async function isCrawlCancelled(db, crawlId) {
    const row = await db
        .prepare("SELECT cancelled FROM crawl_jobs WHERE id = ? LIMIT 1")
        .bind(crawlId)
        .first();
    return Boolean(row?.cancelled);
}
/** Get URLs blocked by robots.txt during a crawl (Firecrawl-compatible). */
export async function getRobotsBlocked(db, crawlId) {
    const { results } = await db
        .prepare(`SELECT url FROM crawl_robots_blocked
       WHERE crawl_id = ?
       ORDER BY created_at ASC`)
        .bind(crawlId)
        .all();
    return (results ?? []).map((r) => r.url);
}
/** Add a URL blocked by robots.txt (replaces Redis SADD crawl:id:robots_blocked). */
export async function addRobotsBlocked(db, crawlId, url) {
    const now = new Date().toISOString();
    try {
        await db
            .prepare(`INSERT INTO crawl_robots_blocked (crawl_id, url, created_at)
         VALUES (?, ?, ?)`)
            .bind(crawlId, url, now)
            .run();
    }
    catch (e) {
        // Unique constraint violation — URL already tracked; ignore
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes("UNIQUE") ||
            msg.includes("unique") ||
            msg.includes("constraint")) {
            return;
        }
        throw e;
    }
}
//# sourceMappingURL=crawl-store.js.map