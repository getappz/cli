/**
 * Scrape controller: Firecrawl-compatible /scrape endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/scrape.ts + scrape-status.ts.
 *
 * Flow:
 * - POST /v2/scrape → Store job in D1 → Run scrape → Update job → Return result
 * - GET /v2/scrape/:jobId → Look up job in D1 → Return status/result
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function scrapeController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    data: {
        markdown?: string | undefined;
        html?: string | null | undefined;
        rawHtml?: string | null | undefined;
        screenshot?: string | null | undefined;
        links?: string[] | undefined;
        images?: string[] | undefined;
        assets?: string[] | undefined;
        actions?: {
            screenshots?: string[] | undefined;
            scrapes?: {
                url: string;
                html: string;
            }[] | undefined;
            javascriptReturns?: {
                type: string;
                value: import("hono/utils/types").JSONValue;
            }[] | undefined;
            pdfs?: string[] | undefined;
        } | null | undefined;
        metadata?: {
            [x: string]: import("hono/utils/types").JSONValue;
            title?: string | undefined;
            description?: string | undefined;
            language?: string | null | undefined;
            sourceURL?: string | undefined;
            statusCode?: number | undefined;
            error?: string | null | undefined;
        } | undefined;
        llm_extraction?: import("hono/utils/types").JSONValue | undefined;
        warning?: string | null | undefined;
        changeTracking?: {
            previousScrapeAt?: string | null | undefined;
            changeStatus?: "new" | "same" | "changed" | "removed" | undefined;
            visibility?: "visible" | "hidden" | undefined;
            diff?: string | null | undefined;
            json?: {
                [x: string]: import("hono/utils/types").JSONValue;
            } | null | undefined;
        } | null | undefined;
        branding?: {
            [x: string]: import("hono/utils/types").JSONValue;
        } | null | undefined;
    };
    cacheState: "hit" | "miss";
    cachedAt: string;
    creditsUsed: number;
    concurrencyLimited: boolean;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
    url: string;
}, 502, "json">)>;
export declare function scrapeStatusController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
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
    status: string;
    id: string;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
    id: string;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    status: string;
    data: {
        markdown?: string | undefined;
        html?: string | null | undefined;
        rawHtml?: string | null | undefined;
        screenshot?: string | null | undefined;
        links?: string[] | undefined;
        images?: string[] | undefined;
        assets?: string[] | undefined;
        actions?: {
            screenshots?: string[] | undefined;
            scrapes?: {
                url: string;
                html: string;
            }[] | undefined;
            javascriptReturns?: {
                type: string;
                value: import("hono/utils/types").JSONValue;
            }[] | undefined;
            pdfs?: string[] | undefined;
        } | null | undefined;
        metadata?: {
            [x: string]: import("hono/utils/types").JSONValue;
            title?: string | undefined;
            description?: string | undefined;
            language?: string | null | undefined;
            sourceURL?: string | undefined;
            statusCode?: number | undefined;
            error?: string | null | undefined;
        } | undefined;
        llm_extraction?: import("hono/utils/types").JSONValue | undefined;
        warning?: string | null | undefined;
        changeTracking?: {
            previousScrapeAt?: string | null | undefined;
            changeStatus?: "new" | "same" | "changed" | "removed" | undefined;
            visibility?: "visible" | "hidden" | undefined;
            diff?: string | null | undefined;
            json?: {
                [x: string]: import("hono/utils/types").JSONValue;
            } | null | undefined;
        } | null | undefined;
        branding?: {
            [x: string]: import("hono/utils/types").JSONValue;
        } | null | undefined;
    } | null;
    cacheState: "miss";
    cachedAt: string;
    creditsUsed: number;
    concurrencyLimited: false;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">)>;
//# sourceMappingURL=scrape.d.ts.map