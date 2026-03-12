/**
 * Upload screenshot to R2 and return object key for serving.
 * Uses screenshots/ prefix. Served via GET /v2/media/screenshot/:filename
 * where filename = {uuid}.png or {uuid}.jpeg
 */
import type { R2Bucket } from "@cloudflare/workers-types";
/**
 * Upload screenshot buffer to R2.
 * Returns filename (e.g. uuid.png) for URL construction.
 */
export declare function uploadScreenshotToR2(bucket: R2Bucket, buffer: Uint8Array, contentType: "image/png" | "image/jpeg"): Promise<{
    filename: string;
    key: string;
}>;
/** Get R2 key from URL filename param. */
export declare function screenshotKeyFromFilename(filename: string): string;
//# sourceMappingURL=screenshot-upload.d.ts.map