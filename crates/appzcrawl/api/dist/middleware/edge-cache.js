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
import { SCRAPE_DEFAULTS } from "../contracts/scrape";
import { buildEdgeCacheKeyPayload } from "../lib/cache/key";
import { normalizeUrlForCache } from "../lib/cache/url";
import { logger } from "../lib/logger";
/** Checks whether we can read from edge cache for this request. */
function shouldReadEdgeCache(body) {
    if (body.maxAge <= 0)
        return false;
    if (body.formats.includes("changeTracking"))
        return false;
    if (body.zeroDataRetention)
        return false;
    if (body.headers && Object.keys(body.headers).length > 0)
        return false;
    return true;
}
/** Checks whether we should write to edge cache after the response. */
function shouldWriteEdgeCache(body) {
    if (body.zeroDataRetention)
        return false;
    return true;
}
function parseBody(raw) {
    if (!raw || typeof raw !== "object")
        return null;
    const obj = raw;
    const url = typeof obj.url === "string" ? obj.url.trim() : "";
    if (!url)
        return null;
    const formats = Array.isArray(obj.formats)
        ? obj.formats.filter((f) => typeof f === "string")
        : SCRAPE_DEFAULTS.formats;
    return {
        url,
        formats: formats.length > 0 ? formats : [...SCRAPE_DEFAULTS.formats],
        onlyMainContent: typeof obj.onlyMainContent === "boolean"
            ? obj.onlyMainContent
            : SCRAPE_DEFAULTS.onlyMainContent,
        includeTags: Array.isArray(obj.includeTags)
            ? obj.includeTags.filter((t) => typeof t === "string")
            : [],
        excludeTags: Array.isArray(obj.excludeTags)
            ? obj.excludeTags.filter((t) => typeof t === "string")
            : [],
        mobile: typeof obj.mobile === "boolean" ? obj.mobile : SCRAPE_DEFAULTS.mobile,
        removeBase64Images: typeof obj.removeBase64Images === "boolean"
            ? obj.removeBase64Images
            : SCRAPE_DEFAULTS.removeBase64Images,
        blockAds: typeof obj.blockAds === "boolean"
            ? obj.blockAds
            : SCRAPE_DEFAULTS.blockAds,
        maxAge: typeof obj.maxAge === "number" && obj.maxAge >= 0
            ? obj.maxAge
            : SCRAPE_DEFAULTS.maxAge,
        storeInCache: typeof obj.storeInCache === "boolean"
            ? obj.storeInCache
            : SCRAPE_DEFAULTS.storeInCache,
        zeroDataRetention: typeof obj.zeroDataRetention === "boolean"
            ? obj.zeroDataRetention
            : SCRAPE_DEFAULTS.zeroDataRetention,
        citations: typeof obj.citations === "boolean" ? obj.citations : false,
        headers: obj.headers &&
            typeof obj.headers === "object" &&
            !Array.isArray(obj.headers)
            ? obj.headers
            : undefined,
    };
}
/** SHA-256 hex hash using Web Crypto (same as lib/cache/url.ts hashString). */
async function sha256Hex(value) {
    const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
    return Array.from(new Uint8Array(buf))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
}
/** Build deterministic edge cache key Request from parsed body. */
async function buildEdgeCacheKey(requestUrl, body) {
    const canonicalUrl = normalizeUrlForCache(/^https?:\/\//i.test(body.url) ? body.url : `https://${body.url}`);
    const payload = buildEdgeCacheKeyPayload({
        url: canonicalUrl,
        formats: body.formats,
        onlyMainContent: body.onlyMainContent,
        includeTags: body.includeTags,
        excludeTags: body.excludeTags,
        citations: body.citations,
        mobile: body.mobile,
        removeBase64Images: body.removeBase64Images,
        blockAds: body.blockAds,
    });
    const keyHash = await sha256Hex(payload);
    const cacheUrl = new URL(requestUrl);
    cacheUrl.pathname = `/_edge_cache/${keyHash}`;
    cacheUrl.search = "";
    return new Request(cacheUrl.toString(), { method: "GET" });
}
/**
 * Cloudflare Edge Cache middleware for POST /v2/scrape.
 * Must be placed after auth middleware but before the scrape controller.
 */
export const edgeCacheMiddleware = async (c, next) => {
    const t0 = Date.now();
    // Gracefully skip when Cache API is unavailable (local dev without --remote)
    if (typeof caches === "undefined") {
        logger.debug("[edge-cache] SKIP: caches API not available (local dev)");
        await next();
        return;
    }
    // Clone body before it's consumed by the controller
    let rawBody;
    try {
        rawBody = await c.req.raw.clone().json();
    }
    catch {
        // Invalid JSON — let the controller handle the error
        await next();
        return;
    }
    const body = parseBody(rawBody);
    if (!body) {
        await next();
        return;
    }
    const cacheKeyReq = await buildEdgeCacheKey(c.req.url, body);
    const cache = caches.default;
    // --- Read path ---
    const canRead = shouldReadEdgeCache(body);
    if (canRead) {
        const cached = await cache.match(cacheKeyReq);
        if (cached) {
            logger.info("[edge-cache] HIT", { url: body.url, ms: Date.now() - t0 });
            const cloned = cached.clone();
            let bodyJson;
            try {
                bodyJson = await cloned.json();
            }
            catch {
                const resp = new Response(cached.body, cached);
                resp.headers.set("X-Cache", "HIT");
                return resp;
            }
            const obj = bodyJson;
            if (obj && obj.success === true && typeof obj === "object") {
                obj.cacheState = "hit";
                obj.cachedAt = new Date().toISOString();
            }
            const resp = new Response(JSON.stringify(obj), {
                status: cached.status,
                headers: new Headers(cached.headers),
            });
            resp.headers.set("X-Cache", "HIT");
            return resp;
        }
        logger.info("[edge-cache] MISS", { url: body.url, ms: Date.now() - t0 });
    }
    else {
        logger.info("[edge-cache] BYPASS read", {
            url: body.url,
            maxAge: body.maxAge,
        });
    }
    // --- Execute the scrape controller ---
    await next();
    // --- Write path: always update L1 after a 200 response ---
    const res = c.res;
    if (res.status === 200 && shouldWriteEdgeCache(body)) {
        const ttlMs = Math.max(body.maxAge, SCRAPE_DEFAULTS.maxAge);
        const ttlSeconds = Math.ceil(ttlMs / 1000);
        // Clone and set Cache-Control for the edge
        const responseToCache = new Response(res.clone().body, res);
        responseToCache.headers.set("Cache-Control", `public, max-age=${ttlSeconds}`);
        responseToCache.headers.set("X-Cache", canRead ? "MISS" : "BYPASS");
        c.executionCtx.waitUntil(cache.put(cacheKeyReq, responseToCache).catch((e) => {
            logger.warn("[edge-cache] store failed", {
                error: e instanceof Error ? e.message : String(e),
            });
        }));
        logger.debug("[edge-cache] stored", { ttlSeconds, ms: Date.now() - t0 });
    }
    // Tag the response header for observability
    if (!c.res.headers.has("X-Cache")) {
        c.res.headers.set("X-Cache", canRead ? "MISS" : "BYPASS");
    }
};
//# sourceMappingURL=edge-cache.js.map