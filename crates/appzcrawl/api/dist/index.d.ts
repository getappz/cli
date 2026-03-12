import { AppzcrawlContainer } from "./container";
import type { AppEnv, CrawlQueueMessage, ScrapeQueueMessage } from "./types";
export { AppzcrawlContainer };
declare const _default: {
    fetch: (request: Request, Env?: {} | {
        DB: D1Database;
        BUCKET: R2Bucket;
        BROWSER_SERVICE?: import("@services/appz-browser").BrowserServiceBinding;
        APPZCRAWL_CONTAINER: DurableObjectNamespace<AppzcrawlContainer>;
        APPZCRAWL_ENGINE?: import("./types").AppzcrawlEngineRpc;
        MARKDOWN_SERVICE_URL?: string;
        FIRE_ENGINE_URL?: string;
        CLOUDFLARE_ACCOUNT_ID?: string;
        CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
        AI?: Ai;
        LLAMAPARSE_API_KEY?: string;
        SARVAM_API_KEY?: string;
        ENVIRONMENT?: string;
        USE_CONTAINER_BACKEND?: string;
        DISABLE_WASM_BACKEND?: string;
        API_KEY?: string;
        DEV_CREATE_KEY?: string;
        CRAWL_QUEUE?: Queue<CrawlQueueMessage>;
        SCRAPE_QUEUE?: Queue<ScrapeQueueMessage>;
    } | undefined, executionCtx?: import("hono").ExecutionContext) => Response | Promise<Response>;
    /**
     * Cloudflare Queue consumer handler for both CRAWL_QUEUE and SCRAPE_QUEUE.
     * Routes to appropriate consumer based on queue name.
     *
     * - CRAWL_QUEUE: Processes crawl jobs (URL discovery + fan-out to SCRAPE_QUEUE)
     * - SCRAPE_QUEUE: Processes individual URL scrape jobs (parallel workers)
     */
    queue(batch: MessageBatch<CrawlQueueMessage | ScrapeQueueMessage>, env: AppEnv["Bindings"]): Promise<void>;
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
    scheduled(_event: ScheduledEvent, env: AppEnv["Bindings"], ctx: ExecutionContext): Promise<void>;
};
export default _default;
//# sourceMappingURL=index.d.ts.map