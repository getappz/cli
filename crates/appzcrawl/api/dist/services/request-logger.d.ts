/**
 * Request logger: audit trail for all API requests using request_log table.
 * Adapted from Firecrawl's logging/log_job.ts (logRequest, logScrape, etc.).
 *
 * Logs are written asynchronously via waitUntil to avoid blocking responses.
 */
export interface RequestLogEntry {
    /** Team that made the request. */
    teamId: string;
    /** API key ID (from api_keys table). */
    apiKeyId?: number;
    /** API endpoint path: /v2/scrape, /v2/crawl, etc. */
    endpoint: string;
    /** HTTP method: POST, GET, DELETE. */
    method: string;
    /** Associated job ID if applicable. */
    jobId?: string;
    /** Credits billed for this request. */
    creditsBilled?: number;
    /** HTTP response status code. */
    statusCode?: number;
    /** Request duration in milliseconds. */
    durationMs?: number;
}
/**
 * Log an API request to the request_log table.
 * Should be called via waitUntil to avoid blocking the response.
 */
export declare function logRequest(db: D1Database, entry: RequestLogEntry): Promise<void>;
/**
 * Create a Hono middleware that logs requests after they complete.
 * Uses timing from requestTiming variable if available.
 */
export declare function requestLoggerMiddleware(): (c: import("hono").Context<import("../types").AppEnv>, next: import("hono").Next) => Promise<void>;
//# sourceMappingURL=request-logger.d.ts.map