/**
 * Scrape controller: Firecrawl-compatible /scrape endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/scrape.ts + scrape-status.ts.
 *
 * Flow:
 * - POST /v2/scrape → Store job in D1 → Run scrape → Update job → Return result
 * - GET /v2/scrape/:jobId → Look up job in D1 → Return status/result
 */

import type { Context } from "hono";
import { parseScrapeRequestBody } from "../contracts/scrape";
import { logger } from "../lib/logger";
import { checkUrl } from "../lib/validateUrl";
import { mapToFirecrawlResponse } from "../services/scrape-response-mapper";
import { runScrapeUrl } from "../services/scrape-runner";
import {
  createScrapeJob,
  getScrapeJob,
  getScrapeTeamId,
  updateScrapeFailure,
  updateScrapeSuccess,
} from "../services/scrape-store";
import type { AppEnv } from "../types";

function normalizeUrl(url: string): string {
  if (!/^https?:\/\//i.test(url)) return `https://${url}`;
  return url;
}

function generateId(): string {
  return crypto.randomUUID();
}

export async function scrapeController(c: Context<AppEnv>) {
  let rawBody: unknown;
  try {
    rawBody = await c.req.json();
  } catch {
    return c.json(
      { success: false, error: "Invalid JSON body; expected { url: string }" },
      400,
    );
  }

  const parsed = parseScrapeRequestBody(rawBody);
  if (!parsed.ok) {
    return c.json({ success: false, error: parsed.error }, 400);
  }

  const { data: body } = parsed;
  const url = normalizeUrl(body.url);
  logger.debug("[scrape] request", {
    url,
    formats: body.formats,
    maxAge: body.maxAge,
  });
  try {
    checkUrl(url);
  } catch (e) {
    return c.json(
      { success: false, error: e instanceof Error ? e.message : "Invalid URL" },
      400,
    );
  }

  const auth = c.get("auth");
  const teamId = auth?.team_id ?? "anonymous";

  // Generate job ID and store in D1 before scraping
  const jobId = generateId();
  const { url: _url, ...scrapeOptions } = body;

  // Store job in D1 (skip for ZDR mode)
  if (!body.zeroDataRetention) {
    try {
      await createScrapeJob(c.env.DB, {
        id: jobId,
        teamId,
        url,
        options: scrapeOptions,
        zeroDataRetention: body.zeroDataRetention,
      });
    } catch (e) {
      logger.warn("[scrape] failed to create job record", {
        jobId,
        error: e instanceof Error ? e.message : String(e),
      });
      // Continue anyway - job storage is best-effort for sync scrapes
    }
  }

  const wantsScreenshotFormat = body.formats?.some(
    (f) => f === "screenshot" || f === "screenshot@fullPage",
  );
  const screenshotBaseUrl = wantsScreenshotFormat
    ? `${new URL(c.req.url).origin}/v2/media/screenshot`
    : undefined;

  const result = await runScrapeUrl(c.env, url, {
    onlyMainContent: body.onlyMainContent,
    useFireEngine: body.useFireEngine,
    engine: body.engine,
    formats: body.formats,
    includeTags: body.includeTags,
    excludeTags: body.excludeTags,
    maxAge: body.maxAge,
    storeInCache: body.storeInCache,
    zeroDataRetention: body.zeroDataRetention,
    headers: body.headers,
    citations: body.citations,
    mobile: body.mobile,
    removeBase64Images: body.removeBase64Images,
    blockAds: body.blockAds,
    skipTlsVerification: body.skipTlsVerification,
    timeout: body.timeout,
    screenshotBaseUrl,
    screenshotOptions: body.screenshotOptions,
    waitFor: body.waitFor,
    jsonOptions: body.jsonOptions,
  });

  if (result.success) {
    const response = mapToFirecrawlResponse(result.document, body);

    // Update job with success result (skip for ZDR mode)
    if (!body.zeroDataRetention) {
      c.executionCtx.waitUntil(
        updateScrapeSuccess(c.env.DB, jobId, response.data).catch((e) => {
          logger.warn("[scrape] failed to update job success", {
            jobId,
            error: e instanceof Error ? e.message : String(e),
          });
        }),
      );
    }

    return c.json(response);
  }

  // Update job with failure (skip for ZDR mode)
  if (!body.zeroDataRetention) {
    c.executionCtx.waitUntil(
      updateScrapeFailure(c.env.DB, jobId, result.error).catch((e) => {
        logger.warn("[scrape] failed to update job failure", {
          jobId,
          error: e instanceof Error ? e.message : String(e),
        });
      }),
    );
  }

  return c.json(
    {
      success: false,
      error: result.error,
      url: result.url,
    },
    502,
  );
}

// ---------------------------------------------------------------------------
// GET /v2/scrape/:jobId — get scrape status/result
// Adapted from firecrawl/apps/api/src/controllers/v2/scrape-status.ts
// ---------------------------------------------------------------------------

export async function scrapeStatusController(c: Context<AppEnv>) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  // Validate UUID format (Firecrawl-compatible)
  const uuidReg =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  if (!uuidReg.test(jobId)) {
    return c.json({ success: false, error: "Invalid scrape ID" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  // First, lightweight check for team ownership
  const scrapeTeam = await getScrapeTeamId(c.env.DB, jobId);
  if (!scrapeTeam) {
    return c.json({ success: false, error: "Job not found." }, 404);
  }

  if (scrapeTeam.teamId !== auth.team_id) {
    return c.json(
      { success: false, error: "You are not allowed to access this resource." },
      403,
    );
  }

  // Get full job data
  const job = await getScrapeJob(c.env.DB, jobId);
  if (!job) {
    return c.json({ success: false, error: "Job not found." }, 404);
  }

  // Check ZDR (zero data retention)
  if (job.zeroDataRetention) {
    return c.json(
      {
        success: false,
        error:
          "Zero data retention is enabled for this scrape. Status lookup is not available.",
      },
      400,
    );
  }

  // Return based on status
  if (job.status === "pending") {
    return c.json({
      success: true,
      status: "scraping",
      id: jobId,
    });
  }

  if (job.status === "failed") {
    return c.json({
      success: false,
      error: job.error ?? "Scrape failed",
      id: jobId,
    });
  }

  // Completed - return the result with Firecrawl envelope
  return c.json({
    success: true,
    status: "completed",
    data: job.result,
    cacheState: "miss" as const,
    cachedAt: new Date().toISOString(),
    creditsUsed: 1,
    concurrencyLimited: false,
  });
}
