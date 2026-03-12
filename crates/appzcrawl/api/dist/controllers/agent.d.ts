/**
 * Agent controller: Firecrawl-compatible /agent endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/agent.ts + agent-status.ts.
 *
 * Flow:
 * - POST /v2/agent → Store job in D1 → Return job ID (async)
 * - GET /v2/agent/:jobId → Look up job in D1 → Return status/result
 * - DELETE /v2/agent/:jobId → Mark job cancelled
 *
 * NOTE: Actual agent execution is not yet implemented; the job stays in
 * "pending" until an external agent service picks it up.
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function agentController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
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
export declare function agentStatusController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
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
export declare function agentCancelController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
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
    success: true;
    id: string;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">)>;
//# sourceMappingURL=agent.d.ts.map