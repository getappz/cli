/**
 * Batch scrape controller: Firecrawl-compatible /batch/scrape endpoint.
 * Adapted from firecrawl/apps/api/src/controllers/v2/batch-scrape.ts.
 *
 * Flow: POST /batch/scrape → D1 insert → enqueue URLs to SCRAPE_QUEUE → return 200 {id, url}
 * Status/cancel reuse existing crawl controllers (they already support isBatchScrape).
 */

import type { Context } from "hono";
import type { BatchScrapeStartResponse } from "../contracts/batch-scrape";
import { parseBatchScrapeRequestBody } from "../contracts/batch-scrape";
import { logger } from "../lib/logger";
import { checkUrl } from "../lib/validateUrl";
import { createCrawlJob } from "../services/crawl-store";
import type { AppEnv, ScrapeQueueMessage } from "../types";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function normalizeUrl(url: string): string {
  if (!/^https?:\/\//i.test(url)) return `https://${url}`;
  return url;
}

function generateId(): string {
  return crypto.randomUUID();
}

// ---------------------------------------------------------------------------
// POST /v2/batch/scrape — start a batch scrape (async, Firecrawl-compatible)
// Adapted from firecrawl/apps/api/src/controllers/v2/batch-scrape.ts
// ---------------------------------------------------------------------------

export async function batchScrapeController(c: Context<AppEnv>) {
  let rawBody: unknown;
  try {
    rawBody = await c.req.json();
  } catch {
    return c.json(
      { success: false, error: "Invalid JSON body; expected { urls: [...] }" },
      400,
    );
  }

  const parsed = parseBatchScrapeRequestBody(rawBody);
  if (!parsed.ok) {
    return c.json({ success: false, error: parsed.error }, 400);
  }

  const { data: body } = parsed;
  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  // Validate and normalize URLs
  const normalizedUrls: string[] = [];
  const invalidURLs: string[] = [];

  for (const url of body.urls) {
    try {
      const normalized = normalizeUrl(url);
      checkUrl(normalized);
      normalizedUrls.push(normalized);
    } catch (e) {
      if (body.ignoreInvalidURLs) {
        invalidURLs.push(url);
      } else {
        return c.json(
          {
            success: false,
            error: `Invalid URL: ${url} - ${e instanceof Error ? e.message : String(e)}`,
          },
          400,
        );
      }
    }
  }

  if (normalizedUrls.length === 0) {
    return c.json({ success: false, error: "No valid URLs provided" }, 400);
  }

  // Clamp total URLs to remaining credits
  const account = c.get("account");
  const remainingCredits = account?.remainingCredits ?? 999_999;
  const urls = normalizedUrls.slice(0, remainingCredits);
  if (normalizedUrls.length > urls.length) {
    logger.warn("[batch-scrape] URLs clamped to remaining credits", {
      requested: normalizedUrls.length,
      clamped: urls.length,
      remainingCredits,
    });
  }

  const wantsScreenshot = body.formats?.some(
    (f) => f === "screenshot" || f === "screenshot@fullPage",
  );
  const screenshotBaseUrl = wantsScreenshot
    ? `${new URL(c.req.url).origin}/v2/media/screenshot`
    : undefined;

  const scrapeOptions = {
    formats: body.formats,
    onlyMainContent: body.onlyMainContent ?? true,
    includeTags: body.includeTags,
    excludeTags: body.excludeTags,
    headers: body.headers,
    maxAge: body.maxAge,
    storeInCache: body.storeInCache,
    zeroDataRetention: body.zeroDataRetention,
    citations: body.citations,
    mobile: body.mobile,
    removeBase64Images: body.removeBase64Images,
    blockAds: body.blockAds,
    skipTlsVerification: body.skipTlsVerification,
    timeout: body.timeout,
    waitFor: body.waitFor,
    useFireEngine: body.useFireEngine,
    engine: body.engine,
    screenshotBaseUrl,
    screenshotOptions: body.screenshotOptions,
    jsonOptions: body.jsonOptions,
  };

  const id = generateId();
  logger.info("[batch-scrape] starting", {
    batchId: id,
    urlCount: urls.length,
    teamId: auth.team_id,
  });

  // 1. Persist to D1 as a batch_scrape job
  await createCrawlJob(c.env.DB, {
    id,
    type: "batch_scrape",
    teamId: auth.team_id,
    originUrl: urls[0], // First URL as origin
    crawlerOptions: {}, // Empty for batch scrape
    scrapeOptions,
    webhook: body.webhook,
    zeroDataRetention: body.zeroDataRetention,
  });

  // 2. Enqueue all URLs to SCRAPE_QUEUE
  const messages: { body: ScrapeQueueMessage }[] = urls.map((url) => ({
    body: {
      jobType: "batch_scrape",
      crawlId: id,
      url,
      teamId: auth.team_id,
      scrapeOptions: {
        onlyMainContent: scrapeOptions.onlyMainContent,
        useFireEngine: scrapeOptions.useFireEngine,
        engine: scrapeOptions.engine,
        screenshotBaseUrl: scrapeOptions.screenshotBaseUrl,
        formats: scrapeOptions.formats,
        includeTags: scrapeOptions.includeTags,
        excludeTags: scrapeOptions.excludeTags,
        maxAge: scrapeOptions.maxAge,
        storeInCache: scrapeOptions.storeInCache,
        zeroDataRetention: scrapeOptions.zeroDataRetention,
        headers: scrapeOptions.headers,
        citations: scrapeOptions.citations,
        mobile: scrapeOptions.mobile,
        removeBase64Images: scrapeOptions.removeBase64Images,
        blockAds: scrapeOptions.blockAds,
        skipTlsVerification: scrapeOptions.skipTlsVerification,
        timeout: scrapeOptions.timeout,
        screenshotOptions: scrapeOptions.screenshotOptions,
        waitFor: scrapeOptions.waitFor,
        jsonOptions: scrapeOptions.jsonOptions,
      },
    },
  }));

  if (c.env.SCRAPE_QUEUE) {
    // Send in batches (Cloudflare Queues supports batch send)
    const BATCH_SIZE = 100;
    for (let i = 0; i < messages.length; i += BATCH_SIZE) {
      const batch = messages.slice(i, i + BATCH_SIZE);
      await c.env.SCRAPE_QUEUE.sendBatch(batch);
    }
    logger.info("[batch-scrape] enqueued to SCRAPE_QUEUE", {
      batchId: id,
      count: urls.length,
    });
  } else {
    // Fallback: run sync (dev mode without queues)
    logger.warn(
      "[batch-scrape] SCRAPE_QUEUE not bound; will run synchronously in background",
      { batchId: id },
    );
    // Process async in background
    const { processScrapeQueue } = await import("../services/scrape-consumer");
    c.executionCtx.waitUntil(
      processScrapeQueue(
        {
          messages: messages.map((m, idx) => ({
            id: `${id}-${idx}`,
            timestamp: new Date(),
            body: m.body,
            attempts: 0,
            ack: () => {},
            retry: () => {},
          })),
          queue: "SCRAPE_QUEUE",
          ackAll: () => {},
          retryAll: () => {},
        },
        c.env,
      ).catch((e: unknown) => {
        logger.error("[batch-scrape] sync fallback failed", {
          batchId: id,
          error: e instanceof Error ? e.message : String(e),
        });
      }),
    );
  }

  // 3. Return async response (Firecrawl-compatible)
  const protocol = new URL(c.req.url).protocol.replace(":", "");
  const host = c.req.header("host") ?? new URL(c.req.url).host;
  const response: BatchScrapeStartResponse = {
    success: true,
    id,
    url: `${protocol}://${host}/v2/batch/scrape/${id}`,
    invalidURLs: invalidURLs.length > 0 ? invalidURLs : undefined,
  };

  return c.json(response, 200);
}
