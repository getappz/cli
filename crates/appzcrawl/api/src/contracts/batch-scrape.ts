/**
 * Firecrawl-compatible batch scrape API contracts.
 * Adapted from firecrawl/apps/api/src/controllers/v2/types.ts (batchScrapeRequestSchema).
 */

import type { ScrapeFormat, ScrapeRequestBody } from "./scrape";

// ---------------------------------------------------------------------------
// Batch scrape request body
// ---------------------------------------------------------------------------

export interface BatchScrapeRequestBody extends Omit<ScrapeRequestBody, "url"> {
  /** Array of URLs to scrape. */
  urls: string[];
  /** Webhook to notify on completion. */
  webhook?: string;
  /** Ignore invalid URLs and continue with valid ones. */
  ignoreInvalidURLs?: boolean;
  /** Request origin (api, dashboard, etc.). */
  origin?: string;
}

// ---------------------------------------------------------------------------
// Batch scrape response types (Firecrawl-compatible)
// ---------------------------------------------------------------------------

/** POST /v2/batch/scrape response. */
export interface BatchScrapeStartResponse {
  success: true;
  id: string;
  url: string;
  invalidURLs?: string[];
}

// ---------------------------------------------------------------------------
// Parse & validate batch scrape request body
// ---------------------------------------------------------------------------

export function parseBatchScrapeRequestBody(
  body: unknown,
): { ok: true; data: BatchScrapeRequestBody } | { ok: false; error: string } {
  if (body === null || typeof body !== "object") {
    return { ok: false, error: "Invalid JSON body; expected object" };
  }
  const raw = body as Record<string, unknown>;

  const urls = raw.urls;
  if (!Array.isArray(urls) || urls.length === 0) {
    return { ok: false, error: "Missing or invalid urls array in body" };
  }

  const urlStrings: string[] = [];
  for (const url of urls) {
    if (typeof url !== "string" || !url.trim()) {
      return { ok: false, error: "All URLs must be non-empty strings" };
    }
    urlStrings.push(url.trim());
  }

  const data: BatchScrapeRequestBody = {
    urls: urlStrings,
    formats:
      Array.isArray(raw.formats) &&
      raw.formats.every((f) => typeof f === "string")
        ? (raw.formats as ScrapeFormat[])
        : undefined,
    onlyMainContent:
      typeof raw.onlyMainContent === "boolean" ? raw.onlyMainContent : true,
    includeTags: Array.isArray(raw.includeTags)
      ? (raw.includeTags as string[])
      : undefined,
    excludeTags: Array.isArray(raw.excludeTags)
      ? (raw.excludeTags as string[])
      : undefined,
    headers:
      raw.headers && typeof raw.headers === "object"
        ? (raw.headers as Record<string, string>)
        : undefined,
    maxAge:
      typeof raw.maxAge === "number" && raw.maxAge >= 0
        ? raw.maxAge
        : undefined,
    storeInCache:
      typeof raw.storeInCache === "boolean" ? raw.storeInCache : undefined,
    zeroDataRetention: Boolean(raw.zeroDataRetention),
    webhook: typeof raw.webhook === "string" ? raw.webhook : undefined,
    ignoreInvalidURLs: Boolean(raw.ignoreInvalidURLs),
    origin: typeof raw.origin === "string" ? raw.origin : "api",
    citations: typeof raw.citations === "boolean" ? raw.citations : undefined,
    mobile: typeof raw.mobile === "boolean" ? raw.mobile : undefined,
    removeBase64Images:
      typeof raw.removeBase64Images === "boolean"
        ? raw.removeBase64Images
        : undefined,
    blockAds: typeof raw.blockAds === "boolean" ? raw.blockAds : undefined,
    skipTlsVerification:
      typeof raw.skipTlsVerification === "boolean"
        ? raw.skipTlsVerification
        : undefined,
    timeout:
      typeof raw.timeout === "number" && raw.timeout > 0
        ? raw.timeout
        : undefined,
    waitFor:
      typeof raw.waitFor === "number" && raw.waitFor > 0
        ? raw.waitFor
        : undefined,
    useFireEngine:
      typeof raw.useFireEngine === "boolean" ? raw.useFireEngine : undefined,
    engine:
      raw.engine === "native" ||
      raw.engine === "cloudflare" ||
      raw.engine === "auto"
        ? raw.engine
        : undefined,
    screenshotOptions:
      raw.screenshotOptions && typeof raw.screenshotOptions === "object"
        ? (raw.screenshotOptions as Record<string, unknown>)
        : undefined,
    jsonOptions:
      raw.jsonOptions && typeof raw.jsonOptions === "object"
        ? (() => {
            const jo = raw.jsonOptions as Record<string, unknown>;
            return {
              prompt: typeof jo.prompt === "string" ? jo.prompt : undefined,
              schema:
                jo.schema && typeof jo.schema === "object"
                  ? (jo.schema as Record<string, unknown>)
                  : undefined,
            };
          })()
        : undefined,
  };

  return { ok: true, data };
}
