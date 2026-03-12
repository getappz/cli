/**
 * Request logger: audit trail for all API requests using request_log table.
 * Adapted from Firecrawl's logging/log_job.ts (logRequest, logScrape, etc.).
 *
 * Logs are written asynchronously via waitUntil to avoid blocking responses.
 */
import { logger } from "../lib/logger";
// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------
/**
 * Log an API request to the request_log table.
 * Should be called via waitUntil to avoid blocking the response.
 */
export async function logRequest(db, entry) {
    const id = crypto.randomUUID();
    const now = new Date().toISOString();
    try {
        await db
            .prepare(`INSERT INTO request_log
          (id, team_id, api_key_id, endpoint, method, job_id, credits_billed, status_code, duration_ms, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
            .bind(id, entry.teamId, entry.apiKeyId ?? null, entry.endpoint, entry.method, entry.jobId ?? null, entry.creditsBilled ?? 0, entry.statusCode ?? null, entry.durationMs ?? null, now)
            .run();
    }
    catch (e) {
        // Best-effort logging — don't fail the request
        logger.warn("[request-logger] failed to log request", {
            error: e instanceof Error ? e.message : String(e),
            endpoint: entry.endpoint,
        });
    }
}
/**
 * Create a Hono middleware that logs requests after they complete.
 * Uses timing from requestTiming variable if available.
 */
export function requestLoggerMiddleware() {
    return async (c, next) => {
        await next();
        // Log asynchronously after response
        const auth = c.get("auth");
        if (!auth)
            return; // Don't log unauthenticated requests
        const timing = c.get("requestTiming");
        const durationMs = timing?.startTime
            ? Date.now() - timing.startTime
            : undefined;
        c.executionCtx.waitUntil(logRequest(c.env.DB, {
            teamId: auth.team_id,
            endpoint: new URL(c.req.url).pathname,
            method: c.req.method,
            statusCode: c.res.status,
            durationMs,
        }));
    };
}
//# sourceMappingURL=request-logger.js.map