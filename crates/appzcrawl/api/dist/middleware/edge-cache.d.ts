/**
 * Edge cache middleware using Cloudflare Cache API.
 * Acts as L1 (per-PoP, sub-ms) in front of the D1+R2 L2 cache.
 *
 * Caches POST /v2/scrape responses by hashing the request body into a
 * synthetic GET URL, following Cloudflare's recommended pattern for POST caching.
 *
 * Read path: obeys bypass rules (maxAge<=0, changeTracking, custom headers, zeroDataRetention).
 * Write path: always updates L1 after next() produces a 200 response (unless zeroDataRetention).
 */
import type { MiddlewareHandler } from "hono";
import type { AppEnv } from "../types";
/**
 * Cloudflare Edge Cache middleware for POST /v2/scrape.
 * Must be placed after auth middleware but before the scrape controller.
 */
export declare const edgeCacheMiddleware: MiddlewareHandler<AppEnv>;
//# sourceMappingURL=edge-cache.d.ts.map