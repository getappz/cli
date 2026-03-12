/**
 * Upload screenshot to R2 and return object key for serving.
 * Uses screenshots/ prefix. Served via GET /v2/media/screenshot/:filename
 * where filename = {uuid}.png or {uuid}.jpeg
 */

import type { R2Bucket } from "@cloudflare/workers-types";

const SCREENSHOT_PREFIX = "screenshots";

/**
 * Upload screenshot buffer to R2.
 * Returns filename (e.g. uuid.png) for URL construction.
 */
export async function uploadScreenshotToR2(
  bucket: R2Bucket,
  buffer: Uint8Array,
  contentType: "image/png" | "image/jpeg",
): Promise<{ filename: string; key: string }> {
  const ext = contentType === "image/jpeg" ? "jpeg" : "png";
  const uuid = crypto.randomUUID().replaceAll("-", "");
  const filename = `${uuid}.${ext}`;
  const key = `${SCREENSHOT_PREFIX}/${filename}`;

  await bucket.put(key, buffer, {
    httpMetadata: { contentType },
  });

  return { filename, key };
}

/** Get R2 key from URL filename param. */
export function screenshotKeyFromFilename(filename: string): string {
  return `${SCREENSHOT_PREFIX}/${filename}`;
}
