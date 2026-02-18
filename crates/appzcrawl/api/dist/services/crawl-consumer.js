/**
 * Crawl queue consumer: processes messages from CRAWL_QUEUE.
 * Adapted from Firecrawl's BullMQ worker (services/worker/) that processes scrape jobs.
 *
 * Cloudflare Queues consumer: receives batch of messages, processes each crawl job.
 * Wrangler config sets max_batch_size=1 so each invocation handles one crawl.
 */
import { logger } from "../lib/logger";
import { runCrawlAsync } from "./crawl-runner";
import { updateCrawlStatus } from "./crawl-store";
/**
 * Process a batch of crawl queue messages.
 * Called from the Worker's queue() handler in index.ts.
 */
export async function processCrawlQueue(batch, env) {
    for (const message of batch.messages) {
        const { crawlId, url, teamId } = message.body;
        logger.info("[crawl-consumer] processing crawl job", {
            crawlId,
            url,
            teamId,
            messageId: message.id,
            attempts: message.attempts,
        });
        try {
            await runCrawlAsync(env, crawlId);
            message.ack();
            logger.info("[crawl-consumer] crawl job completed", { crawlId });
        }
        catch (e) {
            const errorMsg = e instanceof Error ? e.message : String(e);
            logger.error("[crawl-consumer] crawl job failed", {
                crawlId,
                error: errorMsg,
                attempts: message.attempts,
            });
            // Mark as failed in D1 so status endpoint reflects it
            try {
                await updateCrawlStatus(env.DB, crawlId, "failed");
            }
            catch {
                // Best effort
            }
            // Retry via queue (up to max_retries in wrangler config)
            message.retry();
        }
    }
}
//# sourceMappingURL=crawl-consumer.js.map