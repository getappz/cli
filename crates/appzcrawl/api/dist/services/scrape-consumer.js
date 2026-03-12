/**
 * Scrape queue consumer: processes individual URL scrape jobs from SCRAPE_QUEUE.
 * Enables high-volume crawling via fan-out pattern:
 *   crawl-consumer discovers URLs → enqueues to SCRAPE_QUEUE → scrape-consumer processes each URL
 */
import { logger } from "../lib/logger";
import { checkCrawlCompletion } from "./crawl-completion-checker";
import { persistCrawlResult } from "./crawl-persistence";
import { addCrawlResult, isCrawlCancelled } from "./crawl-store";
import { buildScrapeRunnerOptionsFromQueue } from "./scrape-options";
import { runScrapeUrl } from "./scrape-runner";
// ---------------------------------------------------------------------------
// Per-domain rate limiting (bounded LRU-style)
// ---------------------------------------------------------------------------
/** Maximum number of domains to track (prevents unbounded memory growth). */
const MAX_DOMAIN_ENTRIES = 1000;
/** Minimum delay between requests to the same domain (ms). */
const MIN_DOMAIN_DELAY_MS = 1000;
/** Per-domain rate limiting state. Bounded to prevent memory leaks. */
const domainLastRequest = new Map();
/** Evict oldest entries when the map exceeds the size limit. */
function evictOldestDomains() {
    if (domainLastRequest.size <= MAX_DOMAIN_ENTRIES)
        return;
    // Maps iterate in insertion order; delete the oldest entries
    const toDelete = domainLastRequest.size - MAX_DOMAIN_ENTRIES;
    let count = 0;
    for (const key of domainLastRequest.keys()) {
        if (count >= toDelete)
            break;
        domainLastRequest.delete(key);
        count++;
    }
}
async function throttleForDomain(url) {
    try {
        const domain = new URL(url).hostname;
        const lastRequest = domainLastRequest.get(domain) || 0;
        const elapsed = Date.now() - lastRequest;
        if (elapsed < MIN_DOMAIN_DELAY_MS) {
            const waitTime = MIN_DOMAIN_DELAY_MS - elapsed;
            await new Promise((resolve) => setTimeout(resolve, waitTime));
        }
        domainLastRequest.set(domain, Date.now());
        evictOldestDomains();
    }
    catch {
        // Invalid URL, skip throttling
    }
}
// ---------------------------------------------------------------------------
// Queue consumer
// ---------------------------------------------------------------------------
/**
 * Process a batch of scrape queue messages.
 * Called from the Worker's queue() handler in index.ts.
 */
export async function processScrapeQueue(batch, env) {
    const results = await Promise.allSettled(batch.messages.map((message) => processSingleMessage(message, env)));
    // Log batch summary
    const succeeded = results.filter((r) => r.status === "fulfilled").length;
    const failed = results.filter((r) => r.status === "rejected").length;
    logger.info("[scrape-consumer] batch processed", {
        total: batch.messages.length,
        succeeded,
        failed,
    });
    // Check if any crawls completed after this batch
    const crawlIds = new Set(batch.messages.map((m) => m.body.crawlId));
    for (const crawlId of crawlIds) {
        try {
            await checkCrawlCompletion(env.DB, crawlId);
        }
        catch (e) {
            logger.warn("[scrape-consumer] completion check failed", {
                crawlId,
                error: e instanceof Error ? e.message : String(e),
            });
        }
    }
}
async function processSingleMessage(message, env) {
    const { crawlId, url, teamId, scrapeOptions } = message.body;
    logger.info("[scrape-consumer] processing", {
        crawlId,
        url,
        teamId,
        attempts: message.attempts,
    });
    try {
        // Check if parent crawl was cancelled
        if (await isCrawlCancelled(env.DB, crawlId)) {
            logger.info("[scrape-consumer] parent cancelled, skipping", {
                crawlId,
                url,
            });
            message.ack();
            return;
        }
        await throttleForDomain(url);
        const scrapeOpts = buildScrapeRunnerOptionsFromQueue(scrapeOptions);
        const result = await runScrapeUrl(env, url, scrapeOpts);
        await persistCrawlResult(env, crawlId, url, result);
        if (!result.success) {
            logger.warn("[scrape-consumer] scrape failed", {
                crawlId,
                url,
                error: result.error,
            });
        }
        else {
            logger.info("[scrape-consumer] scrape completed", { crawlId, url });
        }
        message.ack();
    }
    catch (e) {
        const errorMsg = e instanceof Error ? e.message : String(e);
        logger.error("[scrape-consumer] job failed", {
            crawlId,
            url,
            error: errorMsg,
            attempts: message.attempts,
        });
        // Record failure in D1 (best effort)
        try {
            await addCrawlResult(env.DB, {
                id: crypto.randomUUID(),
                crawlId,
                url,
                status: "failed",
                error: errorMsg,
            });
        }
        catch {
            // Best effort
        }
        message.retry();
    }
}
//# sourceMappingURL=scrape-consumer.js.map