/**
 * Crawl queue consumer: processes messages from CRAWL_QUEUE.
 * Adapted from Firecrawl's BullMQ worker (services/worker/) that processes scrape jobs.
 *
 * Cloudflare Queues consumer: receives batch of messages, processes each crawl job.
 * Wrangler config sets max_batch_size=1 so each invocation handles one crawl.
 */
import type { AppEnv, CrawlQueueMessage } from "../types";
/**
 * Process a batch of crawl queue messages.
 * Called from the Worker's queue() handler in index.ts.
 */
export declare function processCrawlQueue(batch: MessageBatch<CrawlQueueMessage>, env: AppEnv["Bindings"]): Promise<void>;
//# sourceMappingURL=crawl-consumer.d.ts.map