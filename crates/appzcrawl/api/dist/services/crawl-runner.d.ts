/**
 * Crawl runner: discovers URLs from seed, filters via native filter_links,
 * scrapes each URL using runScrapeUrl, and persists results to D1 + R2.
 *
 * Called by the queue consumer worker (crawl-consumer.ts).
 */
import type { AppEnv } from "../types";
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
    errors: Array<{
        url: string;
        error: string;
    }>;
}
/** Sync crawl runner (legacy — used by old controller, kept for compat). */
export declare function runCrawl(env: AppEnv["Bindings"], seedUrl: string, options?: CrawlRunnerOptions): Promise<CrawlRunnerResult>;
export declare function runCrawlAsync(env: AppEnv["Bindings"], crawlId: string): Promise<void>;
//# sourceMappingURL=crawl-runner.d.ts.map