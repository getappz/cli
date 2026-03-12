/**
 * Scrape cache: D1 metadata + R2 document storage.
 */
import { buildCacheKey, buildCacheKeyPayload, buildDocId, buildR2Key, } from "./key";
import { getDecompressed, putCompressed } from "./r2";
import { normalizeUrlForCache } from "./url";
/** Get cached document if fresh. Returns null on miss or expiry. */
export async function getFromCache(env, options) {
    const canonicalUrl = normalizeUrlForCache(options.url);
    const _cacheKeyPayload = buildCacheKeyPayload(options);
    const [urlHash, cacheKey] = await Promise.all([
        import("./url").then((m) => m.hashUrl(canonicalUrl)),
        buildCacheKey(options),
    ]);
    const nowMs = Date.now();
    const minExpiresAtMs = nowMs; // entry is valid if expires_at_ms > now
    const row = await env.DB.prepare(`SELECT id, r2_key, created_at_ms FROM scrape_cache
     WHERE url_hash = ? AND cache_key = ? AND expires_at_ms > ?
     ORDER BY created_at_ms DESC LIMIT 1`)
        .bind(urlHash.slice(0, 16), cacheKey, minExpiresAtMs)
        .first();
    if (!row)
        return null;
    const json = await getDecompressed(env.BUCKET, row.r2_key);
    if (!json)
        return null;
    const document = JSON.parse(json);
    const cachedAt = new Date(row.created_at_ms);
    return { document, cachedAt };
}
/** Store document in cache. Call only when status 200 and complete. */
export async function putToCache(env, options, document, resolvedUrl) {
    const canonicalUrl = normalizeUrlForCache(options.url);
    const cacheKeyPayload = buildCacheKeyPayload(options);
    const [urlHash, cacheKey, docId] = await Promise.all([
        import("./url").then((m) => m.hashUrl(canonicalUrl)),
        buildCacheKey(options),
        buildDocId(canonicalUrl, cacheKeyPayload),
    ]);
    const nowMs = Date.now();
    const expiresAtMs = nowMs + options.maxAge;
    const r2Key = buildR2Key(docId);
    await putCompressed(env.BUCKET, r2Key, JSON.stringify(document));
    await env.DB.prepare(`INSERT OR REPLACE INTO scrape_cache
     (id, url_hash, cache_key, url_resolved, created_at_ms, expires_at_ms, schema_version, status_code, r2_key, formats)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
        .bind(docId, urlHash.slice(0, 16), cacheKey, resolvedUrl, nowMs, expiresAtMs, 1, document.statusCode ?? 200, r2Key, JSON.stringify(options.formats ?? []))
        .run();
}
//# sourceMappingURL=storage.js.map