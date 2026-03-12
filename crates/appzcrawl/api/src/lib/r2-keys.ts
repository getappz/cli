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

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Inline-vs-R2 threshold in bytes. Documents smaller go in D1 TEXT column. */
export const MAX_INLINE_SIZE = 256 * 1024; // 256 KB

// ---------------------------------------------------------------------------
// Key builders
// ---------------------------------------------------------------------------

/** R2 key for a scrape result document. */
export function scrapeResultKey(scrapeId: string): string {
  return `scrapes/${scrapeId}.json`;
}

/** R2 key for a crawl result document (per-URL). */
export function crawlResultKey(crawlId: string, resultId: string): string {
  return `crawl/${crawlId}/${resultId}.json`;
}

/** R2 key for an extract job result. */
export function extractResultKey(extractId: string): string {
  return `extracts/${extractId}.json`;
}

/** R2 key for an agent job result. */
export function agentResultKey(agentId: string): string {
  return `agents/${agentId}.json`;
}

/** R2 key for a search result (async search). */
export function searchResultKey(searchId: string): string {
  return `searches/${searchId}.json`;
}

/** R2 key for a map result. */
export function mapResultKey(mapId: string): string {
  return `maps/${mapId}.json`;
}

/** R2 key for PDF cache (by content hash). */
export function pdfCacheKey(sha256: string): string {
  return `pdf-cache/${sha256}.json`;
}

// ---------------------------------------------------------------------------
// Prefix helpers (for batch delete / list operations)
// ---------------------------------------------------------------------------

/** All known R2 prefixes. Useful for iterating over all object types. */
export const R2_PREFIXES = [
  "scrapes/",
  "crawl/",
  "extracts/",
  "agents/",
  "searches/",
  "maps/",
  "cache/",
  "screenshots/",
  "pdf-cache/",
] as const;

/** Prefix for all crawl results under a specific crawl. */
export function crawlPrefix(crawlId: string): string {
  return `crawl/${crawlId}/`;
}

// ---------------------------------------------------------------------------
// R2 cleanup helpers
// ---------------------------------------------------------------------------

/**
 * Delete a single R2 object by key. Silently succeeds if key does not exist.
 */
export async function deleteR2Object(
  bucket: R2Bucket,
  key: string,
): Promise<void> {
  await bucket.delete(key);
}

/**
 * Delete multiple R2 objects by keys. Uses batch delete (up to 1000 keys per call).
 */
export async function deleteR2Objects(
  bucket: R2Bucket,
  keys: string[],
): Promise<void> {
  if (keys.length === 0) return;

  // R2 batch delete supports up to 1000 keys per call
  const BATCH_SIZE = 1000;
  for (let i = 0; i < keys.length; i += BATCH_SIZE) {
    const batch = keys.slice(i, i + BATCH_SIZE);
    await bucket.delete(batch);
  }
}

/**
 * Delete all R2 objects under a prefix (e.g. all crawl results for a crawl).
 * Lists objects with the prefix, then batch-deletes them.
 * Returns the number of objects deleted.
 */
export async function deleteR2Prefix(
  bucket: R2Bucket,
  prefix: string,
): Promise<number> {
  let totalDeleted = 0;
  let cursor: string | undefined;

  do {
    const listed = await bucket.list({
      prefix,
      limit: 1000,
      cursor,
    });

    if (listed.objects.length > 0) {
      const keys = listed.objects.map((obj) => obj.key);
      await bucket.delete(keys);
      totalDeleted += keys.length;
    }

    cursor = listed.truncated ? listed.cursor : undefined;
  } while (cursor);

  return totalDeleted;
}

/**
 * Store JSON in R2 if it exceeds the inline threshold, otherwise return it for inline storage.
 * Returns { r2Key, inlineJson } — exactly one will be set.
 */
export async function storeOrInline(
  bucket: R2Bucket,
  r2Key: string,
  json: string,
): Promise<{ r2Key?: string; inlineJson?: string }> {
  if (json.length > MAX_INLINE_SIZE) {
    await bucket.put(r2Key, json, {
      httpMetadata: { contentType: "application/json" },
    });
    return { r2Key };
  }
  return { inlineJson: json };
}

/**
 * Retrieve JSON from either inline D1 storage or R2.
 * Pass the inline value and the R2 key; returns parsed JSON string or null.
 */
export async function getFromInlineOrR2(
  bucket: R2Bucket,
  inlineJson: string | null,
  r2Key: string | null,
): Promise<string | null> {
  if (inlineJson) return inlineJson;
  if (!r2Key) return null;

  const obj = await bucket.get(r2Key);
  if (!obj?.body) return null;
  return obj.text();
}
