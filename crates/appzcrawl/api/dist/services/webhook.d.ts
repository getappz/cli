/**
 * Webhook delivery service with webhook_logs audit trail.
 * Adapted from Firecrawl's services/webhook/delivery.ts.
 *
 * Sends webhook events for crawl/batch-scrape/extract completion
 * and logs delivery attempts to D1 webhook_logs table.
 */
export type WebhookEventType = "crawl.started" | "crawl.page" | "crawl.completed" | "crawl.failed" | "batch_scrape.page" | "batch_scrape.completed" | "extract.completed" | "extract.failed" | "agent.completed" | "agent.failed";
export interface WebhookPayload {
    success: boolean;
    type: WebhookEventType;
    id: string;
    data?: unknown;
    error?: string;
    [key: string]: unknown;
}
export interface WebhookDeliveryParams {
    /** Webhook destination URL. */
    url: string;
    /** Team that owns the parent job. */
    teamId: string;
    /** Parent job ID. */
    jobId: string;
    /** Job type: crawl | batch_scrape | extract | agent. */
    jobType: string;
    /** Event type. */
    event: WebhookEventType;
    /** Payload to send. */
    payload: WebhookPayload;
    /** HMAC secret for signing (from team config). */
    hmacSecret?: string | null;
}
/**
 * Deliver a webhook and log the result to D1.
 * Returns true if delivery was successful.
 */
export declare function deliverWebhook(db: D1Database, params: WebhookDeliveryParams): Promise<boolean>;
/**
 * Send crawl completion webhook if configured.
 * Call via waitUntil() to not block the response.
 */
export declare function sendCrawlWebhook(db: D1Database, params: {
    webhookUrl: string;
    teamId: string;
    crawlId: string;
    event: "crawl.completed" | "crawl.failed" | "crawl.page";
    data?: unknown;
    error?: string;
    hmacSecret?: string | null;
}): Promise<void>;
//# sourceMappingURL=webhook.d.ts.map