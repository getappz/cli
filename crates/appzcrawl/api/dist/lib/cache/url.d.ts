/**
 * URL normalization and hashing for scrape cache.
 * Adopts Firecrawl rules for canonical URLs so variants map to the same cache key.
 */
/** Normalize URL for cache lookup. Variants like www, trailing slash, index.html map to same result. */
export declare function normalizeUrlForCache(url: string): string;
/** SHA-256 hash of string using Web Crypto. Returns hex string. */
export declare function hashString(value: string): Promise<string>;
/** Hash normalized URL; returns full 32-char hex. */
export declare function hashUrl(url: string): Promise<string>;
//# sourceMappingURL=url.d.ts.map