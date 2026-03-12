/**
 * Canonical cache key and deterministic document ID for scrape cache.
 */
import { hashString } from "./url";
export const CACHE_SCHEMA_VERSION = 1;
/** Canonical JSON for D1+R2 cache key. Excludes formats (Firecrawl pattern). */
export function buildCacheKeyPayload(input) {
    return JSON.stringify({
        v: CACHE_SCHEMA_VERSION,
        url: input.url,
        onlyMainContent: input.onlyMainContent,
        includeTags: input.includeTags ?? [],
        excludeTags: input.excludeTags ?? [],
        mobile: input.mobile ?? false,
    });
}
/** Canonical JSON for edge cache key. Includes formats (response is format-specific). */
export function buildEdgeCacheKeyPayload(input) {
    return JSON.stringify({
        v: CACHE_SCHEMA_VERSION,
        url: input.url,
        formats: [...input.formats].sort(),
        onlyMainContent: input.onlyMainContent,
        includeTags: input.includeTags ?? [],
        excludeTags: input.excludeTags ?? [],
        citations: input.citations ?? false,
        mobile: input.mobile ?? false,
        removeBase64Images: input.removeBase64Images ?? true,
        blockAds: input.blockAds ?? true,
    });
}
/** SHA-256 of canonical payload (hex). */
export async function buildCacheKey(input) {
    return hashString(buildCacheKeyPayload(input));
}
/** Deterministic document ID: sha256(url + cacheKeyPayload + schemaVersion). */
export async function buildDocId(canonicalUrl, cacheKeyPayload) {
    const input = `${canonicalUrl}\0${cacheKeyPayload}\0${CACHE_SCHEMA_VERSION}`;
    return hashString(input);
}
/** R2 path with partitioning: cache/ab/cd/{id}.json.gz */
export function buildR2Key(docId) {
    const ab = docId.slice(0, 2);
    const cd = docId.slice(2, 4);
    return `cache/${ab}/${cd}/${docId}.json.gz`;
}
//# sourceMappingURL=key.js.map