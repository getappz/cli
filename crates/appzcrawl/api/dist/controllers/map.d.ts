/**
 * POST /v2/map — Firecrawl-compatible map endpoint.
 * Maps multiple URLs from a base URL (sitemap and/or page links).
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function mapController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    links: {
        url: string;
        title?: string | undefined;
        description?: string | undefined;
    }[];
    cacheState: "miss";
    cachedAt: string;
    creditsUsed: number;
    concurrencyLimited: false;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
    code: string;
}, 408, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 500, "json">)>;
//# sourceMappingURL=map.d.ts.map