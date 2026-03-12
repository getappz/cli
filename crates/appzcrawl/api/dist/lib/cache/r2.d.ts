/**
 * R2 compressed blob storage for scrape cache.
 */
import type { R2Bucket } from "@cloudflare/workers-types";
/** Put JSON as gzip-compressed blob to R2. Buffers compressed output so R2 gets a known-length body. */
export declare function putCompressed(bucket: R2Bucket, key: string, json: string): Promise<void>;
/** Get and decompress JSON from R2. Returns null if not found. */
export declare function getDecompressed(bucket: R2Bucket, key: string): Promise<string | null>;
//# sourceMappingURL=r2.d.ts.map