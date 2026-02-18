/**
 * Shared crawl document persistence logic.
 * Handles serializing ScrapeRunnerDocument to JSON and storing in D1/R2.
 * Used by crawl-runner (inline mode + fan-out fallback) and scrape-consumer.
 */
import type { AppEnv } from "../types";
import type { ScrapeRunnerOutput } from "./scrape-runner";
/**
 * Persist a scrape result (success or failure) as a crawl result in D1/R2.
 * Handles inline vs R2 storage based on document size.
 */
export declare function persistCrawlResult(env: AppEnv["Bindings"], crawlId: string, url: string, result: ScrapeRunnerOutput): Promise<void>;
//# sourceMappingURL=crawl-persistence.d.ts.map