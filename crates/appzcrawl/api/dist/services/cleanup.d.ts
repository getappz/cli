/**
 * Scheduled cleanup service for expired data across all tables.
 * Mirrors Firecrawl's pg_cron cleanup jobs using Cloudflare Cron Triggers.
 *
 * Retention policies:
 *   - crawl_jobs + crawl_results + crawl_visited_urls: expires_at column (default 24h)
 *   - extract_jobs + agent_jobs:                       expires_at column (default 24h)
 *   - scrapes:                                         24h after created_at
 *   - scrape_cache:                                    expires_at_ms column
 *   - webhook_logs:                                    7 days
 *   - request_log:                                     30 days
 *   - billing_log:                                     90 days
 *   - R2 objects: orphaned objects deleted alongside their D1 rows
 */
export declare function runScheduledCleanup(db: D1Database, bucket: R2Bucket): Promise<void>;
//# sourceMappingURL=cleanup.d.ts.map