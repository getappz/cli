/**
 * Scrape cache: D1 metadata + R2 document storage.
 */
import type { D1Database, R2Bucket } from "@cloudflare/workers-types";
import type { CacheKeyInput } from "./key";
/** Minimal cached document (Firecrawl pattern: cache HTML only, derive links/images on hit). */
export interface CachedDocument {
    url: string;
    html: string;
    /** Full raw HTML when asset formats requested; needed for css/js extraction (head not stripped). */
    rawHtml?: string;
    statusCode?: number;
    /** Native markdown from Sarvam PDF; avoids re-deriving on cache hit. */
    markdown?: string;
    /** Native JSON from Sarvam PDF; for json format response. */
    documentJson?: unknown;
}
export interface CacheEnv {
    DB: D1Database;
    BUCKET: R2Bucket;
}
export interface CacheLookupOptions extends CacheKeyInput {
    maxAge: number;
}
export interface CacheStoreOptions extends CacheKeyInput {
    maxAge: number;
    /** Stored in DB for audit; not part of cache key. */
    formats?: string[];
}
/** Get cached document if fresh. Returns null on miss or expiry. */
export declare function getFromCache(env: CacheEnv, options: CacheLookupOptions): Promise<{
    document: CachedDocument;
    cachedAt: Date;
} | null>;
/** Store document in cache. Call only when status 200 and complete. */
export declare function putToCache(env: CacheEnv, options: CacheStoreOptions, document: CachedDocument, resolvedUrl: string): Promise<void>;
//# sourceMappingURL=storage.d.ts.map