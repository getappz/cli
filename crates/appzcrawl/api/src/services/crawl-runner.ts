/**
 * Crawl runner: discovers URLs from seed, filters via native filter_links,
 * scrapes each URL using runScrapeUrl, and persists results to D1 + R2.
 *
 * Called by the queue consumer worker (crawl-consumer.ts).
 */

import type { CrawlerOptions } from "../contracts/crawl";
import { logger } from "../lib/logger";
import type { AppEnv, ScrapeQueueMessage } from "../types";
import { persistCrawlResult } from "./crawl-persistence";
import {
  addRobotsBlocked,
  getCrawlJob,
  isCrawlCancelled,
  setTotalCount,
  updateCrawlStatus,
} from "./crawl-store";
import { filterLinks } from "./html-processor";
import { USER_AGENT } from "./scrape-fetcher";
import {
  buildScrapeOptsForQueue,
  buildScrapeRunnerOptions,
} from "./scrape-options";
import { runScrapeUrl } from "./scrape-runner";

const DEFAULT_CONCURRENCY = 5;
const QUEUE_BATCH_SIZE = 25;
const MIN_URLS_FOR_FANOUT = 50;

export interface CrawlRunnerOptions {
  limit?: number;
  sameOriginOnly?: boolean;
  onlyMainContent?: boolean;
  maxConcurrency?: number;
}

export interface CrawlRunnerResult {
  success: true;
  seedUrl: string;
  completed: number;
  total: number;
  data: Array<{
    url: string;
    rawHtml: string;
    metadata: Record<string, unknown>;
    links: string[];
    statusCode?: number;
  }>;
  errors: Array<{ url: string; error: string }>;
}

/** Normalize URL for crawl deduplication (Firecrawl-style). */
function normalizeUrlForCrawlDedup(url: string): string {
  try {
    const u = new URL(url);
    if (u.pathname.length > 1 && u.pathname.endsWith("/")) {
      u.pathname = u.pathname.slice(0, -1);
    }
    for (const suffix of ["/index.html", "/index.htm", "/index.php"]) {
      if (u.pathname.endsWith(suffix)) {
        u.pathname = u.pathname.slice(0, -suffix.length) || "/";
        break;
      }
    }
    if (u.hostname.startsWith("www.")) {
      u.hostname = u.hostname.slice(4);
    }
    u.protocol = "https:";
    u.hash = "";
    return u.toString();
  } catch {
    return url;
  }
}

function sameOrigin(seed: string, link: string): boolean {
  try {
    return new URL(seed).origin === new URL(link).origin;
  } catch {
    return false;
  }
}

function isValidUrl(url: string): boolean {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

interface DiscoverLinksParams {
  env: AppEnv["Bindings"];
  rawLinks: string[];
  seen: Set<string>;
  queue: string[];
  limit: number;
  currentCount: number;
  crawl: {
    originUrl: string;
    crawlerOptions: CrawlerOptions;
  };
  maxDepth: number;
  baseUrl: string;
  robotsTxt: string;
}

/** Discover and filter links from a scraped page. Adds to queue. */
async function discoverLinks(params: DiscoverLinksParams): Promise<void> {
  const {
    env,
    rawLinks,
    seen,
    queue,
    limit,
    currentCount,
    crawl,
    maxDepth,
    baseUrl,
    robotsTxt,
  } = params;
  const opts = crawl.crawlerOptions;

  if (rawLinks.length === 0) return;

  try {
    const filtered = await filterLinks(env, {
      links: rawLinks,
      limit: limit - currentCount,
      maxDepth,
      baseUrl,
      initialUrl: crawl.originUrl,
      regexOnFullUrl: opts.regexOnFullURL,
      excludes: opts.excludePaths,
      includes: opts.includePaths,
      allowBackwardCrawling: opts.crawlEntireDomain,
      ignoreRobotsTxt: opts.ignoreRobotsTxt,
      robotsTxt,
      allowExternalContentLinks: opts.allowExternalLinks,
      allowSubdomains: opts.allowSubdomains,
    });

    // Track robots.txt blocked URLs
    for (const [url, reason] of Object.entries(filtered.denialReasons)) {
      if (reason === "URL blocked by robots.txt") {
        await addRobotsBlocked(env.DB, params.crawl.originUrl, url);
      }
    }

    for (const link of filtered.links) {
      const linkNorm = normalizeUrlForCrawlDedup(link);
      if (!seen.has(linkNorm) && queue.length + currentCount < limit) {
        queue.push(linkNorm);
      }
    }
  } catch (e) {
    logger.warn("[crawl-runner] filterLinks failed, falling back", {
      error: e instanceof Error ? e.message : String(e),
    });
    // Fallback: simple same-origin filter
    for (const link of rawLinks) {
      const trimmed = link.trim();
      if (!trimmed) continue;
      const linkNorm = normalizeUrlForCrawlDedup(trimmed);
      if (seen.has(linkNorm)) continue;
      if (!opts.allowExternalLinks && !sameOrigin(crawl.originUrl, trimmed))
        continue;
      if (!isValidUrl(trimmed)) continue;
      if (queue.length + currentCount < limit) queue.push(linkNorm);
    }
  }
}

/** Sync crawl runner (legacy — used by old controller, kept for compat). */
export async function runCrawl(
  env: AppEnv["Bindings"],
  seedUrl: string,
  options: CrawlRunnerOptions = {},
): Promise<CrawlRunnerResult> {
  const limit = options.limit ?? 10;
  const sameOriginOnly = options.sameOriginOnly !== false;
  const onlyMainContent = Boolean(options.onlyMainContent);
  const concurrency = options.maxConcurrency ?? DEFAULT_CONCURRENCY;

  const data: CrawlRunnerResult["data"] = [];
  const errors: CrawlRunnerResult["errors"] = [];
  const seen = new Set<string>();
  const queue: string[] = [normalizeUrlForCrawlDedup(seedUrl)];

  while (queue.length > 0 && data.length < limit) {
    const batchSize = Math.min(concurrency, limit - data.length, queue.length);
    const batch: string[] = [];

    for (let i = 0; i < batchSize; i++) {
      const url = queue.shift();
      if (!url) continue;
      const norm = normalizeUrlForCrawlDedup(url);
      if (seen.has(norm)) {
        i--;
        continue;
      }
      seen.add(norm);
      batch.push(url);
    }

    if (batch.length === 0) continue;

    const results = await Promise.allSettled(
      batch.map((url) => runScrapeUrl(env, url, { onlyMainContent })),
    );

    for (let i = 0; i < results.length; i++) {
      const url = batch[i];
      const result = results[i];

      if (result.status === "rejected") {
        errors.push({ url, error: String(result.reason) });
        continue;
      }

      const scrapeResult = result.value;
      if (!scrapeResult.success) {
        errors.push({ url, error: scrapeResult.error });
        continue;
      }

      const doc = scrapeResult.document;
      data.push({
        url: doc.url,
        rawHtml: doc.rawHtml,
        metadata: doc.metadata,
        links: doc.links,
        statusCode: doc.statusCode,
      });

      if (data.length >= limit) break;

      for (const link of doc.links) {
        const trimmed = link.trim();
        if (!trimmed) continue;
        const linkNorm = normalizeUrlForCrawlDedup(trimmed);
        if (seen.has(linkNorm)) continue;
        if (sameOriginOnly && !sameOrigin(seedUrl, trimmed)) continue;
        if (!isValidUrl(trimmed)) continue;
        if (queue.length + data.length >= limit) break;
        queue.push(linkNorm);
      }
    }
  }

  return {
    success: true,
    seedUrl,
    completed: data.length,
    total: seen.size,
    data,
    errors,
  };
}

async function fetchRobotsTxt(seedUrl: string): Promise<string> {
  try {
    const origin = new URL(seedUrl).origin;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 10_000);
    const res = await fetch(`${origin}/robots.txt`, {
      signal: controller.signal,
      headers: { "User-Agent": USER_AGENT },
    });
    clearTimeout(timer);
    if (!res.ok) return "";
    return await res.text();
  } catch {
    return "";
  }
}

export async function runCrawlAsync(
  env: AppEnv["Bindings"],
  crawlId: string,
): Promise<void> {
  const crawl = await getCrawlJob(env.DB, crawlId);
  if (!crawl) {
    logger.error("[crawl-runner] crawl job not found", { crawlId });
    return;
  }
  if (crawl.cancelled) {
    logger.info("[crawl-runner] crawl already cancelled", { crawlId });
    return;
  }

  const opts = crawl.crawlerOptions;
  const scrapeOpts = buildScrapeRunnerOptions(crawl.scrapeOptions);
  const limit = opts.limit ?? 10000;
  const maxDepth = opts.maxDiscoveryDepth ?? 10;
  const concurrency = opts.maxConcurrency ?? DEFAULT_CONCURRENCY;
  const useFanOut = Boolean(env.SCRAPE_QUEUE) && limit >= MIN_URLS_FOR_FANOUT;

  logger.info("[crawl-runner] starting", {
    crawlId,
    limit,
    maxDepth,
    concurrency,
    useFanOut,
  });

  await updateCrawlStatus(env.DB, crawlId, "scraping");

  // Fetch robots.txt
  let robotsTxt = "";
  if (!opts.ignoreRobotsTxt) {
    robotsTxt = await fetchRobotsTxt(crawl.originUrl);
    if (robotsTxt) {
      await updateCrawlStatus(env.DB, crawlId, "scraping", { robotsTxt });
    }
  }

  const seen = new Set<string>();
  const queue: string[] = [normalizeUrlForCrawlDedup(crawl.originUrl)];
  let enqueuedCount = 0;
  let completedInRunner = 0;
  let baseUrl: string;
  try {
    baseUrl = new URL(crawl.originUrl).origin;
  } catch {
    baseUrl = crawl.originUrl;
  }

  const scrapeOptsForQueue = buildScrapeOptsForQueue(crawl.scrapeOptions);

  while (queue.length > 0 && enqueuedCount < limit) {
    // Periodic cancellation check
    if (enqueuedCount > 0 && enqueuedCount % 50 === 0) {
      if (await isCrawlCancelled(env.DB, crawlId)) {
        await updateCrawlStatus(env.DB, crawlId, "cancelled", {
          cancelled: true,
        });
        return;
      }
    }

    if (useFanOut) {
      await processFanOutBatch(env, {
        crawlId,
        crawl,
        queue,
        seen,
        limit,
        enqueuedCount,
        scrapeOpts,
        scrapeOptsForQueue,
        maxDepth,
        baseUrl,
        robotsTxt,
      });
      // Update counters from the mutable state
      enqueuedCount = seen.size;
    } else {
      completedInRunner = await processInlineBatch(env, {
        crawlId,
        crawl,
        queue,
        seen,
        limit,
        completedInRunner,
        concurrency,
        scrapeOpts,
        maxDepth,
        baseUrl,
        robotsTxt,
        opts,
      });
      enqueuedCount = completedInRunner;
    }
  }

  // Final cancellation check
  if (await isCrawlCancelled(env.DB, crawlId)) {
    await updateCrawlStatus(env.DB, crawlId, "cancelled", { cancelled: true });
    return;
  }

  if (useFanOut) {
    await updateCrawlStatus(env.DB, crawlId, "scraping", {
      totalCount: enqueuedCount,
    });
    logger.info("[crawl-runner] discovery complete, URLs enqueued", {
      crawlId,
      enqueuedCount,
      totalDiscovered: seen.size,
    });
  } else {
    await updateCrawlStatus(env.DB, crawlId, "completed", {
      completedCount: completedInRunner,
      totalCount: seen.size,
    });
    logger.info("[crawl-runner] completed (inline)", {
      crawlId,
      completed: completedInRunner,
      total: seen.size,
    });
  }

  // Webhook (inline mode only; fan-out uses completion checker)
  if (!useFanOut && crawl.webhook) {
    try {
      await fetch(crawl.webhook, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          success: true,
          type: "crawl.completed",
          id: crawlId,
          data: { completed: completedInRunner, total: seen.size },
        }),
      });
    } catch (e) {
      logger.warn("[crawl-runner] webhook failed", {
        crawlId,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  }
}

interface BatchParamsBase {
  crawlId: string;
  crawl: Awaited<ReturnType<typeof getCrawlJob>> & {};
  queue: string[];
  seen: Set<string>;
  limit: number;
  scrapeOpts: ReturnType<typeof buildScrapeRunnerOptions>;
  maxDepth: number;
  baseUrl: string;
  robotsTxt: string;
}

async function processFanOutBatch(
  env: AppEnv["Bindings"],
  params: BatchParamsBase & {
    enqueuedCount: number;
    scrapeOptsForQueue: ScrapeQueueMessage["scrapeOptions"];
  },
): Promise<void> {
  const {
    crawlId,
    crawl,
    queue,
    seen,
    limit,
    scrapeOpts,
    scrapeOptsForQueue,
    maxDepth,
    baseUrl,
    robotsTxt,
  } = params;
  let { enqueuedCount } = params;

  const urlBatch: string[] = [];
  while (
    queue.length > 0 &&
    urlBatch.length < QUEUE_BATCH_SIZE &&
    enqueuedCount + urlBatch.length < limit
  ) {
    const url = queue.shift();
    if (!url) continue;
    const norm = normalizeUrlForCrawlDedup(url);
    if (seen.has(norm)) continue;
    seen.add(norm);
    urlBatch.push(url);
  }

  if (urlBatch.length === 0) return;

  const messages: { body: ScrapeQueueMessage }[] = urlBatch.map((url) => ({
    body: {
      jobType: "crawl",
      crawlId,
      url,
      teamId: crawl.teamId,
      scrapeOptions: scrapeOptsForQueue,
    },
  }));

  try {
    await env.SCRAPE_QUEUE?.sendBatch(messages);
    enqueuedCount += urlBatch.length;
    await setTotalCount(env.DB, crawlId, enqueuedCount);

    // For the first batch, scrape seed URL ourselves to discover links
    if (enqueuedCount === urlBatch.length) {
      const result = await runScrapeUrl(env, urlBatch[0], scrapeOpts);
      if (result.success) {
        await discoverLinks({
          env,
          rawLinks: result.document.links ?? [],
          seen,
          queue,
          limit,
          currentCount: enqueuedCount,
          crawl,
          maxDepth,
          baseUrl,
          robotsTxt,
        });
      }
    }
  } catch (e) {
    logger.error("[crawl-runner] failed to enqueue to SCRAPE_QUEUE", {
      crawlId,
      error: e instanceof Error ? e.message : String(e),
    });
    // Fall back to processing inline
    for (const url of urlBatch) {
      const result = await runScrapeUrl(env, url, scrapeOpts);
      await persistCrawlResult(env, crawlId, url, result);
    }
  }
}

async function processInlineBatch(
  env: AppEnv["Bindings"],
  params: BatchParamsBase & {
    completedInRunner: number;
    concurrency: number;
    opts: CrawlerOptions;
  },
): Promise<number> {
  const {
    crawlId,
    crawl,
    queue,
    seen,
    limit,
    concurrency,
    scrapeOpts,
    maxDepth,
    baseUrl,
    robotsTxt,
    opts,
  } = params;
  let { completedInRunner } = params;

  const batchSize = Math.min(
    concurrency,
    limit - completedInRunner,
    queue.length,
  );
  const batch: string[] = [];

  for (let i = 0; i < batchSize; i++) {
    const url = queue.shift();
    if (!url) continue;
    const norm = normalizeUrlForCrawlDedup(url);
    if (seen.has(norm)) {
      i--;
      continue;
    }
    seen.add(norm);
    batch.push(url);
  }

  if (batch.length === 0) return completedInRunner;

  await setTotalCount(env.DB, crawlId, seen.size);

  if (opts.delay && completedInRunner > 0) {
    await new Promise((resolve) => setTimeout(resolve, opts.delay));
  }

  const results = await Promise.allSettled(
    batch.map((url) => runScrapeUrl(env, url, scrapeOpts)),
  );

  for (let i = 0; i < results.length; i++) {
    const url = batch[i];
    const result = results[i];

    if (result.status === "rejected") {
      await persistCrawlResult(env, crawlId, url, {
        success: false,
        url,
        error: String(result.reason),
      });
      continue;
    }

    const scrapeResult = result.value;
    await persistCrawlResult(env, crawlId, url, scrapeResult);

    if (scrapeResult.success) {
      completedInRunner++;
      if (completedInRunner >= limit) break;

      await discoverLinks({
        env,
        rawLinks: scrapeResult.document.links ?? [],
        seen,
        queue,
        limit,
        currentCount: completedInRunner,
        crawl,
        maxDepth,
        baseUrl,
        robotsTxt,
      });
    }
  }

  return completedInRunner;
}
