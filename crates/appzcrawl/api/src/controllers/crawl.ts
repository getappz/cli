/**
 * Crawl controller: Firecrawl-compatible /crawl endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/crawl.ts + crawl-status.ts + crawl-cancel.ts.
 *
 * Flow: POST /crawl → D1 insert → enqueue to CRAWL_QUEUE → return 200 {id, url}
 * Status/cancel/ongoing/errors read from D1 (replaces Redis).
 */

import type { Context } from "hono";
import type {
  CrawlCancelResponse,
  CrawlErrorsResponse,
  CrawlOngoingResponse,
  CrawlStartResponse,
  CrawlStatusResponse,
} from "../contracts/crawl";
import { parseCrawlRequestBody } from "../contracts/crawl";
import { logger } from "../lib/logger";
import { checkUrl } from "../lib/validateUrl";
import {
  cancelCrawlJob,
  createCrawlJob,
  getCrawlErrors,
  getCrawlJob,
  getCrawlResults,
  getOngoingCrawls,
  getRobotsBlocked,
} from "../services/crawl-store";
import type { AppEnv, CrawlQueueMessage } from "../types";

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
// POST /v2/crawl — start a crawl (async, Firecrawl-compatible)
// Adapted from firecrawl/apps/api/src/controllers/v2/crawl.ts crawlController
// ---------------------------------------------------------------------------

export async function crawlController(c: Context<AppEnv>) {
  let rawBody: unknown;
  try {
    rawBody = await c.req.json();
  } catch {
    return c.json(
      { success: false, error: "Invalid JSON body; expected { url: string }" },
      400,
    );
  }

  const parsed = parseCrawlRequestBody(rawBody);
  if (!parsed.ok) {
    return c.json({ success: false, error: parsed.error }, 400);
  }

  const { data: body } = parsed;
  const url = normalizeUrl(body.url);
  try {
    checkUrl(url);
  } catch (e) {
    return c.json(
      { success: false, error: e instanceof Error ? e.message : "Invalid URL" },
      400,
    );
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  // Clamp limit to remaining credits
  const account = c.get("account");
  const remainingCredits = account?.remainingCredits ?? 999_999;
  const limit = Math.min(body.limit ?? 10000, remainingCredits);

  const crawlerOptions = {
    includePaths: body.includePaths ?? [],
    excludePaths: body.excludePaths ?? [],
    maxDiscoveryDepth: body.maxDiscoveryDepth,
    limit,
    crawlEntireDomain: body.crawlEntireDomain,
    allowExternalLinks: body.allowExternalLinks ?? false,
    allowSubdomains: body.allowSubdomains ?? false,
    ignoreRobotsTxt: body.ignoreRobotsTxt ?? false,
    sitemap: body.sitemap ?? "include",
    deduplicateSimilarURLs: body.deduplicateSimilarURLs ?? true,
    ignoreQueryParameters: body.ignoreQueryParameters ?? false,
    regexOnFullURL: body.regexOnFullURL ?? false,
    delay: body.delay,
  };

  const baseScrapeOpts = body.scrapeOptions ?? {};
  const wantsScreenshot = baseScrapeOpts.formats?.some(
    (f) => f === "screenshot" || f === "screenshot@fullPage",
  );
  const scrapeOptions = {
    ...baseScrapeOpts,
    ...(wantsScreenshot && {
      screenshotBaseUrl: `${new URL(c.req.url).origin}/v2/media/screenshot`,
    }),
  };

  const id = generateId();
  logger.info("[crawl] starting", {
    crawlId: id,
    url,
    limit,
    teamId: auth.team_id,
  });

  // 1. Persist to D1
  await createCrawlJob(c.env.DB, {
    id,
    teamId: auth.team_id,
    originUrl: url,
    crawlerOptions,
    scrapeOptions,
    webhook: body.webhook,
    zeroDataRetention: body.zeroDataRetention,
  });

  // 2. Enqueue to CRAWL_QUEUE
  const queueMsg: CrawlQueueMessage = {
    crawlId: id,
    url,
    teamId: auth.team_id,
  };

  if (c.env.CRAWL_QUEUE) {
    await c.env.CRAWL_QUEUE.send(queueMsg);
    logger.info("[crawl] enqueued to CRAWL_QUEUE", { crawlId: id });
  } else {
    // Fallback: run sync (dev mode without queues)
    logger.warn(
      "[crawl] CRAWL_QUEUE not bound; will run synchronously in background",
      { crawlId: id },
    );
    // Use waitUntil if available (Workers runtime) to not block response
    const { runCrawlAsync } = await import("../services/crawl-runner");
    c.executionCtx.waitUntil(
      runCrawlAsync(c.env, id).catch((e: unknown) => {
        logger.error("[crawl] sync fallback failed", {
          crawlId: id,
          error: e instanceof Error ? e.message : String(e),
        });
      }),
    );
  }

  // 3. Return async response (Firecrawl-compatible)
  const protocol = new URL(c.req.url).protocol.replace(":", "");
  const host = c.req.header("host") ?? new URL(c.req.url).host;
  const response: CrawlStartResponse = {
    success: true,
    id,
    url: `${protocol}://${host}/v2/crawl/${id}`,
  };

  return c.json(response, 200);
}

// ---------------------------------------------------------------------------
// POST /v2/crawl/params-preview — stub (LLM-based; deferred)
// ---------------------------------------------------------------------------

export async function crawlParamsPreviewController(c: Context<AppEnv>) {
  return c.json({
    success: true,
    data: { totalCredits: 0, urls: [] },
  });
}

// ---------------------------------------------------------------------------
// GET /v2/crawl/:jobId — poll status
// Adapted from firecrawl/apps/api/src/controllers/v2/crawl-status.ts
// ---------------------------------------------------------------------------

export async function crawlStatusController(
  c: Context<AppEnv>,
  _isBatchScrape = false,
) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const crawl = await getCrawlJob(c.env.DB, jobId);
  if (!crawl || crawl.teamId !== auth.team_id) {
    return c.json({ success: false, error: "Job not found" }, 404);
  }

  // Parse pagination
  const skipParam = c.req.query("skip");
  const limitParam = c.req.query("limit");
  const skip = skipParam ? Number.parseInt(skipParam, 10) : 0;
  const limit = limitParam ? Number.parseInt(limitParam, 10) : 100;

  // Fetch result documents
  const data = await getCrawlResults(c.env.DB, jobId, { skip, limit });

  // Build next URL for pagination
  const protocol = new URL(c.req.url).protocol.replace(":", "");
  const host = c.req.header("host") ?? new URL(c.req.url).host;
  const endpoint = _isBatchScrape ? "batch/scrape" : "crawl";
  const hasMore =
    crawl.status !== "completed" || crawl.completedCount > skip + data.length;
  const next = hasMore
    ? `${protocol}://${host}/v2/${endpoint}/${jobId}?skip=${skip + data.length}${limitParam ? `&limit=${limitParam}` : ""}`
    : undefined;

  const response: CrawlStatusResponse = {
    success: true,
    status: crawl.status,
    completed: crawl.completedCount,
    total: crawl.totalCount,
    creditsUsed: crawl.creditsBilled,
    expiresAt: crawl.expiresAt,
    data,
    next,
    cacheState: "miss" as const,
    cachedAt: new Date().toISOString(),
    concurrencyLimited: false,
  };

  return c.json(response, 200);
}

// ---------------------------------------------------------------------------
// DELETE /v2/crawl/:jobId — cancel crawl
// Adapted from firecrawl/apps/api/src/controllers/v2/crawl-cancel.ts
// ---------------------------------------------------------------------------

export async function crawlCancelController(c: Context<AppEnv>) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const crawl = await getCrawlJob(c.env.DB, jobId);
  if (!crawl || crawl.teamId !== auth.team_id) {
    return c.json({ success: false, error: "Job not found" }, 404);
  }

  if (crawl.status === "completed") {
    return c.json(
      {
        success: false,
        error: "Crawl is already completed",
      } as CrawlCancelResponse,
      409,
    );
  }

  if (crawl.status === "cancelled") {
    return c.json(
      { success: true, status: "cancelled" } as CrawlCancelResponse,
      200,
    );
  }

  await cancelCrawlJob(c.env.DB, jobId);

  return c.json(
    { success: true, status: "cancelled" } as CrawlCancelResponse,
    200,
  );
}

// ---------------------------------------------------------------------------
// GET /v2/crawl/ongoing — list ongoing crawls for team
// Adapted from firecrawl/apps/api/src/controllers/v2/crawl-ongoing.ts
// ---------------------------------------------------------------------------

export async function ongoingCrawlsController(c: Context<AppEnv>) {
  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const crawls = await getOngoingCrawls(c.env.DB, auth.team_id);

  const response: CrawlOngoingResponse = {
    success: true,
    crawls: crawls.map((cr) => ({
      id: cr.id,
      teamId: cr.teamId,
      url: cr.originUrl,
      created_at: cr.createdAt,
      options: {
        ...cr.crawlerOptions,
        scrapeOptions: cr.scrapeOptions,
      },
    })),
  };

  return c.json(response, 200);
}

// ---------------------------------------------------------------------------
// GET /v2/crawl/:jobId/errors — list failed URLs
// Adapted from firecrawl/apps/api/src/controllers/v2/crawl-errors.ts
// ---------------------------------------------------------------------------

export async function crawlErrorsController(c: Context<AppEnv>) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const crawl = await getCrawlJob(c.env.DB, jobId);
  if (!crawl || crawl.teamId !== auth.team_id) {
    return c.json({ success: false, error: "Job not found" }, 404);
  }

  const errors = await getCrawlErrors(c.env.DB, jobId);
  const robotsBlocked = await getRobotsBlocked(c.env.DB, jobId);

  const response: CrawlErrorsResponse = {
    errors: errors.map((e) => ({
      id: e.id,
      timestamp: e.timestamp,
      url: e.url,
      code: e.code ?? undefined,
      error: e.error,
    })),
    robotsBlocked,
  };

  return c.json(response, 200);
}
