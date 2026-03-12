import { app } from "./app";
import { AppzcrawlContainer } from "./container";
import { runScheduledCleanup } from "./services/cleanup";
import { processCrawlQueue } from "./services/crawl-consumer";
import { processScrapeQueue } from "./services/scrape-consumer";
const fetchHandler = app.fetch.bind(app);
export { AppzcrawlContainer };
export default {
    fetch: fetchHandler,
    /**
     * Cloudflare Queue consumer handler for both CRAWL_QUEUE and SCRAPE_QUEUE.
     * Routes to appropriate consumer based on queue name.
     *
     * - CRAWL_QUEUE: Processes crawl jobs (URL discovery + fan-out to SCRAPE_QUEUE)
     * - SCRAPE_QUEUE: Processes individual URL scrape jobs (parallel workers)
     */
    async queue(batch, env) {
        // Route to appropriate consumer based on queue name
        if (batch.queue === "appzcrawl-scrape-queue") {
            await processScrapeQueue(batch, env);
        }
        else {
            // Default to crawl queue (appzcrawl-crawl-queue)
            await processCrawlQueue(batch, env);
        }
    },
    /**
     * Cloudflare Cron Trigger handler for scheduled data cleanup.
     *
     * Cleans expired/old data across all tables:
     *   - Crawl jobs + results + visited URLs (by expires_at)
     *   - Extract/Agent jobs (by expires_at)
     *   - Scrapes (24h retention)
     *   - Scrape cache (by expires_at_ms)
     *   - Webhook logs (7 days)
     *   - Request log (30 days)
     *   - Billing log (90 days)
     *   - Orphaned R2 objects
     */
    async scheduled(_event, env, ctx) {
        ctx.waitUntil(runScheduledCleanup(env.DB, env.BUCKET));
    },
};
//# sourceMappingURL=index.js.map