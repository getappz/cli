/**
 * Shared scrape options builder.
 * Converts request-level scrape options to ScrapeRunnerOptions.
 * Used by crawl-runner and scrape-consumer (DRY).
 */

import { SCRAPE_DEFAULTS, type ScrapeRequestBody } from "../contracts/scrape";
import type { ScrapeQueueMessage } from "../types";
import type { ScrapeRunnerOptions } from "./scrape-runner";

/**
 * Build ScrapeRunnerOptions from a crawl's per-page scrape options.
 * Firecrawl-compatible: when maxAge is undefined, defaults to 2 days (cache enabled).
 */
export function buildScrapeRunnerOptions(
  scrapeOpts: Omit<ScrapeRequestBody, "url"> | undefined,
): ScrapeRunnerOptions {
  if (!scrapeOpts) {
    return {
      onlyMainContent: true,
      maxAge: SCRAPE_DEFAULTS.maxAge,
      storeInCache: SCRAPE_DEFAULTS.storeInCache,
    };
  }
  return {
    onlyMainContent: scrapeOpts.onlyMainContent ?? true,
    useFireEngine: scrapeOpts.useFireEngine,
    engine: scrapeOpts.engine,
    formats: scrapeOpts.formats,
    includeTags: scrapeOpts.includeTags,
    excludeTags: scrapeOpts.excludeTags,
    maxAge:
      scrapeOpts.maxAge !== undefined
        ? scrapeOpts.maxAge
        : SCRAPE_DEFAULTS.maxAge,
    storeInCache:
      scrapeOpts.storeInCache !== undefined
        ? scrapeOpts.storeInCache
        : SCRAPE_DEFAULTS.storeInCache,
    zeroDataRetention: scrapeOpts.zeroDataRetention,
    headers: scrapeOpts.headers,
    citations: scrapeOpts.citations,
    mobile: scrapeOpts.mobile,
    removeBase64Images: scrapeOpts.removeBase64Images,
    blockAds: scrapeOpts.blockAds,
    skipTlsVerification: scrapeOpts.skipTlsVerification,
    timeout: scrapeOpts.timeout,
    screenshotOptions: scrapeOpts.screenshotOptions,
    waitFor: scrapeOpts.waitFor,
    jsonOptions: scrapeOpts.jsonOptions,
  };
}

/**
 * Build ScrapeRunnerOptions from a queue message's scrape options.
 */
export function buildScrapeRunnerOptionsFromQueue(
  opts: ScrapeQueueMessage["scrapeOptions"],
): ScrapeRunnerOptions {
  if (!opts) {
    return {
      onlyMainContent: true,
      maxAge: SCRAPE_DEFAULTS.maxAge,
      storeInCache: SCRAPE_DEFAULTS.storeInCache,
    };
  }
  return {
    onlyMainContent: opts.onlyMainContent ?? true,
    useFireEngine: opts.useFireEngine,
    engine: opts.engine,
    screenshotBaseUrl: opts.screenshotBaseUrl,
    formats: opts.formats,
    includeTags: opts.includeTags,
    excludeTags: opts.excludeTags,
    maxAge: opts.maxAge ?? SCRAPE_DEFAULTS.maxAge,
    storeInCache: opts.storeInCache ?? SCRAPE_DEFAULTS.storeInCache,
    zeroDataRetention: opts.zeroDataRetention,
    headers: opts.headers,
    citations: opts.citations,
    mobile: opts.mobile,
    removeBase64Images: opts.removeBase64Images,
    blockAds: opts.blockAds,
    skipTlsVerification: opts.skipTlsVerification,
    timeout: opts.timeout,
    screenshotOptions: opts.screenshotOptions,
    waitFor: opts.waitFor,
    jsonOptions: opts.jsonOptions,
  };
}

/**
 * Build scrape options subset for queue messages from crawl's scrapeOptions.
 */
export function buildScrapeOptsForQueue(
  scrapeOpts:
    | (Omit<ScrapeRequestBody, "url"> & { screenshotBaseUrl?: string })
    | undefined,
): ScrapeQueueMessage["scrapeOptions"] {
  if (!scrapeOpts) return undefined;
  return {
    onlyMainContent: scrapeOpts.onlyMainContent,
    useFireEngine: scrapeOpts.useFireEngine,
    engine: scrapeOpts.engine,
    screenshotBaseUrl: scrapeOpts.screenshotBaseUrl,
    formats: scrapeOpts.formats,
    includeTags: scrapeOpts.includeTags,
    excludeTags: scrapeOpts.excludeTags,
    maxAge: scrapeOpts.maxAge,
    storeInCache: scrapeOpts.storeInCache,
    zeroDataRetention: scrapeOpts.zeroDataRetention,
    headers: scrapeOpts.headers,
    citations: scrapeOpts.citations,
    mobile: scrapeOpts.mobile,
    removeBase64Images: scrapeOpts.removeBase64Images,
    blockAds: scrapeOpts.blockAds,
    skipTlsVerification: scrapeOpts.skipTlsVerification,
    timeout: scrapeOpts.timeout,
    screenshotOptions: scrapeOpts.screenshotOptions,
    waitFor: scrapeOpts.waitFor,
    jsonOptions: scrapeOpts.jsonOptions,
  };
}
