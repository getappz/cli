import type { MiddlewareHandler } from "hono";
import { Hono } from "hono";
import * as agent from "../controllers/agent";
import * as batchScrape from "../controllers/batch-scrape";
import * as benchmark from "../controllers/benchmark";
import * as browser from "../controllers/browser";
import * as concurrencyCheck from "../controllers/concurrency-check";
import * as crawl from "../controllers/crawl";
import * as creditUsage from "../controllers/credit-usage";
import * as extract from "../controllers/extract";
import * as keys from "../controllers/keys";
import * as map from "../controllers/map";
import * as queueStatus from "../controllers/queue-status";
import * as scrape from "../controllers/scrape";
import * as search from "../controllers/search";
import * as tokenUsage from "../controllers/token-usage";
import { screenshotKeyFromFilename } from "../lib/screenshot-upload";
import {
  authMiddleware,
  blocklistMiddleware,
  checkCreditsMiddleware,
  countryCheck,
  edgeCacheMiddleware,
  idempotencyMiddleware,
  requestTimingMiddleware,
  validateJobIdParam,
} from "../middleware";
import { nativeHealth } from "../services/html-processor";
import type { AppEnv } from "../types";
import { RateLimiterMode } from "../types";

const v2Router = new Hono<AppEnv>();

v2Router.use("*", requestTimingMiddleware("v2"));

// POST /v2/search
v2Router.post(
  "/search",
  authMiddleware(RateLimiterMode.Search),
  countryCheck,
  checkCreditsMiddleware(),
  blocklistMiddleware,
  search.searchController,
);

// POST /v2/scrape
v2Router.post(
  "/scrape",
  authMiddleware(RateLimiterMode.Scrape),
  countryCheck,
  checkCreditsMiddleware(1),
  blocklistMiddleware,
  edgeCacheMiddleware,
  scrape.scrapeController,
);

// GET /v2/scrape/:jobId
v2Router.get(
  "/scrape/:jobId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  scrape.scrapeStatusController,
);

// POST /v2/batch/scrape
v2Router.post(
  "/batch/scrape",
  authMiddleware(RateLimiterMode.Scrape),
  countryCheck,
  checkCreditsMiddleware(),
  blocklistMiddleware,
  batchScrape.batchScrapeController,
);

// POST /v2/map
v2Router.post(
  "/map",
  authMiddleware(RateLimiterMode.Map),
  checkCreditsMiddleware(1),
  blocklistMiddleware,
  map.mapController,
);

// POST /v2/crawl
v2Router.post(
  "/crawl",
  authMiddleware(RateLimiterMode.Crawl),
  countryCheck,
  checkCreditsMiddleware(),
  blocklistMiddleware,
  idempotencyMiddleware,
  crawl.crawlController,
);

// POST /v2/crawl/params-preview
v2Router.post(
  "/crawl/params-preview",
  authMiddleware(RateLimiterMode.Crawl),
  checkCreditsMiddleware(),
  crawl.crawlParamsPreviewController,
);

// GET /v2/crawl/ongoing, /v2/crawl/active
v2Router.get(
  "/crawl/ongoing",
  authMiddleware(RateLimiterMode.CrawlStatus),
  crawl.ongoingCrawlsController,
);
v2Router.get(
  "/crawl/active",
  authMiddleware(RateLimiterMode.CrawlStatus),
  crawl.ongoingCrawlsController,
);

// GET /v2/crawl/:jobId
v2Router.get(
  "/crawl/:jobId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  (c) => crawl.crawlStatusController(c, false),
);

// DELETE /v2/crawl/:jobId
v2Router.delete(
  "/crawl/:jobId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  (c) => crawl.crawlCancelController(c),
);

// WebSocket /v2/crawl/:jobId - return 501 for phase 1
v2Router.get("/crawl/:jobId/ws", (c) =>
  c.json(
    { success: false, error: "WebSocket not implemented; use polling" },
    501,
  ),
);

// GET /v2/batch/scrape/:jobId
v2Router.get(
  "/batch/scrape/:jobId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  (c) => crawl.crawlStatusController(c, true),
);

// DELETE /v2/batch/scrape/:jobId
v2Router.delete(
  "/batch/scrape/:jobId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  (c) => crawl.crawlCancelController(c),
);

// GET /v2/batch/scrape/:jobId/errors
v2Router.get(
  "/batch/scrape/:jobId/errors",
  authMiddleware(RateLimiterMode.CrawlStatus),
  crawl.crawlErrorsController,
);

// GET /v2/crawl/:jobId/errors
v2Router.get(
  "/crawl/:jobId/errors",
  authMiddleware(RateLimiterMode.CrawlStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  crawl.crawlErrorsController,
);

// POST /v2/extract
v2Router.post(
  "/extract",
  authMiddleware(RateLimiterMode.Extract),
  countryCheck,
  checkCreditsMiddleware(20),
  blocklistMiddleware,
  extract.extractController,
);

// GET /v2/extract/:jobId
v2Router.get(
  "/extract/:jobId",
  authMiddleware(RateLimiterMode.ExtractStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  extract.extractStatusController,
);

// POST /v2/agent
v2Router.post(
  "/agent",
  authMiddleware(RateLimiterMode.Extract),
  countryCheck,
  checkCreditsMiddleware(20),
  blocklistMiddleware,
  agent.agentController,
);

// GET /v2/agent/:jobId
v2Router.get(
  "/agent/:jobId",
  authMiddleware(RateLimiterMode.ExtractStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  agent.agentStatusController,
);

// DELETE /v2/agent/:jobId
v2Router.delete(
  "/agent/:jobId",
  authMiddleware(RateLimiterMode.ExtractStatus),
  validateJobIdParam as MiddlewareHandler<AppEnv>,
  agent.agentCancelController,
);

// ============================================================================
// Browser API
// ============================================================================

// POST /v2/browser - Create a browser session
v2Router.post(
  "/browser",
  authMiddleware(RateLimiterMode.Scrape),
  checkCreditsMiddleware(1),
  browser.browserCreateController,
);

// POST /v2/browser/execute - Execute actions in a browser session
v2Router.post(
  "/browser/execute",
  authMiddleware(RateLimiterMode.Scrape),
  checkCreditsMiddleware(1),
  browser.browserExecuteController,
);

// DELETE /v2/browser - Delete a browser session
v2Router.delete(
  "/browser",
  authMiddleware(RateLimiterMode.Scrape),
  browser.browserDeleteController,
);

// GET /v2/browser/:browserId - Get browser session status
v2Router.get(
  "/browser/:browserId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  browser.browserStatusController,
);

// GET /v2/team/credit-usage
v2Router.get(
  "/team/credit-usage",
  authMiddleware(RateLimiterMode.CrawlStatus),
  creditUsage.creditUsageController,
);

// GET /v2/team/credit-usage/historical
v2Router.get(
  "/team/credit-usage/historical",
  authMiddleware(RateLimiterMode.CrawlStatus),
  creditUsage.creditUsageHistoricalController,
);

// GET /v2/team/token-usage
v2Router.get(
  "/team/token-usage",
  authMiddleware(RateLimiterMode.ExtractStatus),
  tokenUsage.tokenUsageController,
);

// GET /v2/team/token-usage/historical
v2Router.get(
  "/team/token-usage/historical",
  authMiddleware(RateLimiterMode.ExtractStatus),
  tokenUsage.tokenUsageHistoricalController,
);

// GET /v2/concurrency-check
v2Router.get(
  "/concurrency-check",
  authMiddleware(RateLimiterMode.CrawlStatus),
  concurrencyCheck.concurrencyCheckController,
);

// GET /v2/team/queue-status
v2Router.get(
  "/team/queue-status",
  authMiddleware(RateLimiterMode.CrawlStatus),
  queueStatus.queueStatusController,
);

// ============================================================================
// API Key Management
// ============================================================================

// POST /v2/keys - Create a new API key
v2Router.post(
  "/keys",
  authMiddleware(RateLimiterMode.CrawlStatus),
  keys.createKeyController,
);

// GET /v2/keys - List all API keys for the team
v2Router.get(
  "/keys",
  authMiddleware(RateLimiterMode.CrawlStatus),
  keys.listKeysController,
);

// DELETE /v2/keys/:keyId - Revoke an API key
v2Router.delete(
  "/keys/:keyId",
  authMiddleware(RateLimiterMode.CrawlStatus),
  keys.revokeKeyController,
);

// POST /v2/keys/:keyId/rotate - Rotate an API key (create new + revoke old)
v2Router.post(
  "/keys/:keyId/rotate",
  authMiddleware(RateLimiterMode.CrawlStatus),
  keys.rotateKeyController,
);

// GET /v2/media/screenshot/:filename - serve screenshot from R2 (no auth, public)
v2Router.get("/media/screenshot/:filename", async (c) => {
  const filename = c.req.param("filename");
  if (!filename || !/^[a-f0-9]+\.(png|jpeg)$/i.test(filename)) {
    return c.json({ success: false, error: "Invalid filename" }, 400);
  }
  try {
    const key = screenshotKeyFromFilename(filename);
    const obj = await c.env.BUCKET.get(key);
    if (!obj || !obj.body) {
      return c.json({ success: false, error: "Not found" }, 404);
    }
    const contentType =
      obj.httpMetadata?.contentType ??
      (filename.endsWith(".jpeg") ? "image/jpeg" : "image/png");
    return new Response(obj.body, {
      headers: {
        "Content-Type": contentType,
        "Cache-Control": "public, max-age=86400",
      },
    });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to serve",
      },
      500,
    );
  }
});

// GET /v2/native-health - probe native container (no auth)
v2Router.get("/native-health", async (c) => {
  try {
    const out = await nativeHealth(c.env);
    return c.json(out);
  } catch (e) {
    return c.json(
      { ok: false, error: e instanceof Error ? e.message : "Unknown error" },
      503,
    );
  }
});

// POST /v2/benchmark - benchmark HTML processing backends (devel only, no auth)
// v2Router.post("/benchmark", benchmark.benchmarkController);

// Info at /v2
v2Router.get("/", (c) =>
  c.json({ version: "v2", endpoints: "see OpenAPI or plan" }),
);

export { v2Router };
