/**
 * URL blocklist middleware using D1 url_blocklist table.
 * Adapted from firecrawl/apps/api/src/scraper/WebScraper/utils/blocklist.ts.
 *
 * Loads blocklist from D1 on first use, caches in memory for the worker lifetime.
 * Checks exact domain matches and subdomain matches.
 */
import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
/**
 * Check if a URL is blocked by the blocklist.
 * Matches:
 *   - Exact domain matches (e.g. "example.com" blocks "example.com")
 *   - Subdomain matches (e.g. "example.com" blocks "sub.example.com")
 */
export declare function isUrlBlocked(url: string, db: D1Database): Promise<boolean>;
/**
 * Route-level middleware: no-op passthrough for routes that don't have a URL yet.
 * Controllers that parse a body call checkBlocklist() directly.
 */
export declare function blocklistMiddleware(_c: Context<AppEnv>, next: Next): Promise<void>;
/**
 * Call from controller when you have parsed body to return 403 if URL is blocked.
 * Now uses D1 for the blocklist lookup.
 */
export declare function checkBlocklist(body: {
    url?: string;
} | null, db: D1Database): Promise<{
    blocked: boolean;
    error?: string;
}>;
//# sourceMappingURL=blocklist.d.ts.map