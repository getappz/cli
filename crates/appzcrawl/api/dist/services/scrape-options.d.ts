/**
 * Shared scrape options builder.
 * Converts request-level scrape options to ScrapeRunnerOptions.
 * Used by crawl-runner and scrape-consumer (DRY).
 */
import { type ScrapeRequestBody } from "../contracts/scrape";
import type { ScrapeQueueMessage } from "../types";
import type { ScrapeRunnerOptions } from "./scrape-runner";
/**
 * Build ScrapeRunnerOptions from a crawl's per-page scrape options.
 * Firecrawl-compatible: when maxAge is undefined, defaults to 2 days (cache enabled).
 */
export declare function buildScrapeRunnerOptions(scrapeOpts: Omit<ScrapeRequestBody, "url"> | undefined): ScrapeRunnerOptions;
/**
 * Build ScrapeRunnerOptions from a queue message's scrape options.
 */
export declare function buildScrapeRunnerOptionsFromQueue(opts: ScrapeQueueMessage["scrapeOptions"]): ScrapeRunnerOptions;
/**
 * Build scrape options subset for queue messages from crawl's scrapeOptions.
 */
export declare function buildScrapeOptsForQueue(scrapeOpts: (Omit<ScrapeRequestBody, "url"> & {
    screenshotBaseUrl?: string;
}) | undefined): ScrapeQueueMessage["scrapeOptions"];
//# sourceMappingURL=scrape-options.d.ts.map