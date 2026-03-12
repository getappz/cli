/**
 * Centralised R2 key patterns for all appzcrawl storage.
 *
 * Maps Firecrawl GCS patterns to R2:
 *   GCS {jobId}.json → R2 prefixed paths
 *   GCS pdf-cache-v2/{sha256}.json → R2 pdf-cache/{sha256}.json
 *   GCS media (screenshots) → R2 screenshots/{uuid}.{ext}
 *
 * Existing patterns preserved:
 *   - Scrape cache:    cache/{ab}/{cd}/{docId}.json.gz  (see lib/cache/key.ts)
 *   - Screenshots:     screenshots/{uuid}.{ext}         (see lib/screenshot-upload.ts)
 *   - Crawl results:   crawl/{crawlId}/{uuid}.json      (see services/crawl-runner.ts)
 */
/** Inline-vs-R2 threshold in bytes. Documents smaller go in D1 TEXT column. */
export declare const MAX_INLINE_SIZE: number;
/** R2 key for a scrape result document. */
export declare function scrapeResultKey(scrapeId: string): string;
/** R2 key for a crawl result document (per-URL). */
export declare function crawlResultKey(crawlId: string, resultId: string): string;
/** R2 key for an extract job result. */
export declare function extractResultKey(extractId: string): string;
/** R2 key for an agent job result. */
export declare function agentResultKey(agentId: string): string;
/** R2 key for a search result (async search). */
export declare function searchResultKey(searchId: string): string;
/** R2 key for a map result. */
export declare function mapResultKey(mapId: string): string;
/** R2 key for PDF cache (by content hash). */
export declare function pdfCacheKey(sha256: string): string;
/** All known R2 prefixes. Useful for iterating over all object types. */
export declare const R2_PREFIXES: readonly ["scrapes/", "crawl/", "extracts/", "agents/", "searches/", "maps/", "cache/", "screenshots/", "pdf-cache/"];
/** Prefix for all crawl results under a specific crawl. */
export declare function crawlPrefix(crawlId: string): string;
/**
 * Delete a single R2 object by key. Silently succeeds if key does not exist.
 */
export declare function deleteR2Object(bucket: R2Bucket, key: string): Promise<void>;
/**
 * Delete multiple R2 objects by keys. Uses batch delete (up to 1000 keys per call).
 */
export declare function deleteR2Objects(bucket: R2Bucket, keys: string[]): Promise<void>;
/**
 * Delete all R2 objects under a prefix (e.g. all crawl results for a crawl).
 * Lists objects with the prefix, then batch-deletes them.
 * Returns the number of objects deleted.
 */
export declare function deleteR2Prefix(bucket: R2Bucket, prefix: string): Promise<number>;
/**
 * Store JSON in R2 if it exceeds the inline threshold, otherwise return it for inline storage.
 * Returns { r2Key, inlineJson } — exactly one will be set.
 */
export declare function storeOrInline(bucket: R2Bucket, r2Key: string, json: string): Promise<{
    r2Key?: string;
    inlineJson?: string;
}>;
/**
 * Retrieve JSON from either inline D1 storage or R2.
 * Pass the inline value and the R2 key; returns parsed JSON string or null.
 */
export declare function getFromInlineOrR2(bucket: R2Bucket, inlineJson: string | null, r2Key: string | null): Promise<string | null>;
//# sourceMappingURL=r2-keys.d.ts.map