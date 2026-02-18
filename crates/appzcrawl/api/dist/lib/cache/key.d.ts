/**
 * Canonical cache key and deterministic document ID for scrape cache.
 */
export declare const CACHE_SCHEMA_VERSION = 1;
/** Input for cache key. Excludes formats/citations (same HTML serves all). */
export interface CacheKeyInput {
    url: string;
    onlyMainContent: boolean;
    includeTags: string[];
    excludeTags: string[];
    /** Mobile emulation; affects screenshot when formats include screenshot. */
    mobile?: boolean;
}
/** Input for edge cache key (full response cache; includes formats). */
export interface EdgeCacheKeyInput extends CacheKeyInput {
    formats: string[];
    citations?: boolean;
    removeBase64Images?: boolean;
    blockAds?: boolean;
}
/** Canonical JSON for D1+R2 cache key. Excludes formats (Firecrawl pattern). */
export declare function buildCacheKeyPayload(input: CacheKeyInput): string;
/** Canonical JSON for edge cache key. Includes formats (response is format-specific). */
export declare function buildEdgeCacheKeyPayload(input: EdgeCacheKeyInput): string;
/** SHA-256 of canonical payload (hex). */
export declare function buildCacheKey(input: CacheKeyInput): Promise<string>;
/** Deterministic document ID: sha256(url + cacheKeyPayload + schemaVersion). */
export declare function buildDocId(canonicalUrl: string, cacheKeyPayload: string): Promise<string>;
/** R2 path with partitioning: cache/ab/cd/{id}.json.gz */
export declare function buildR2Key(docId: string): string;
//# sourceMappingURL=key.d.ts.map