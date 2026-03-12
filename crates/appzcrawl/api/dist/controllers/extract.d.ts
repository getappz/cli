/**
 * Extract controller: Firecrawl-compatible /extract endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/extract.ts + extract-status.ts.
 *
 * Flow:
 * - POST /v2/extract → Store job in D1 → Return job ID (async)
 * - GET /v2/extract/:jobId → Look up job in D1 → Return status/result
 *
 * NOTE: Actual LLM extraction is not yet implemented; the job stays in
 * "pending" until a queue consumer or external service picks it up.
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function extractController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 500, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    id: string;
}, 200, "json">)>;
export declare function extractStatusController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 401, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 404, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 403, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    status: string;
    error: string;
    id: string;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    status: string;
    data: import("hono/utils/types").JSONValue;
    warning: string | undefined;
    creditsUsed: number;
    expiresAt: string;
    cacheState: "miss";
    cachedAt: string;
    concurrencyLimited: false;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    status: string;
    id: string;
    expiresAt: string;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">)>;
//# sourceMappingURL=extract.d.ts.map