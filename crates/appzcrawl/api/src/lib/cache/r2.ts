/**
 * R2 compressed blob storage for scrape cache.
 */

import type { R2Bucket } from "@cloudflare/workers-types";

/** Put JSON as gzip-compressed blob to R2. Buffers compressed output so R2 gets a known-length body. */
export async function putCompressed(
  bucket: R2Bucket,
  key: string,
  json: string,
): Promise<void> {
  const stream = new Blob([json])
    .stream()
    .pipeThrough(new CompressionStream("gzip"));
  const compressed = await new Response(stream).arrayBuffer();
  await bucket.put(key, compressed, {
    customMetadata: { "Content-Encoding": "gzip" },
    httpMetadata: { contentType: "application/json" },
  });
}

/** Get and decompress JSON from R2. Returns null if not found. */
export async function getDecompressed(
  bucket: R2Bucket,
  key: string,
): Promise<string | null> {
  const obj = await bucket.get(key);
  if (!obj || !obj.body) return null;

  const decompressed = obj.body.pipeThrough(new DecompressionStream("gzip"));
  const blob = await new Response(decompressed).blob();
  return blob.text();
}
