/**
 * Scrape queue consumer: processes individual URL scrape jobs from SCRAPE_QUEUE.
 * Enables high-volume crawling via fan-out pattern:
 *   crawl-consumer discovers URLs → enqueues to SCRAPE_QUEUE → scrape-consumer processes each URL
 */
import type { AppEnv, ScrapeQueueMessage } from "../types";
/**
 * Process a batch of scrape queue messages.
 * Called from the Worker's queue() handler in index.ts.
 */
export declare function processScrapeQueue(batch: MessageBatch<ScrapeQueueMessage>, env: AppEnv["Bindings"]): Promise<void>;
//# sourceMappingURL=scrape-consumer.d.ts.map