// Crawl
export type { CrawlProgress } from "./crawl-completion-checker";
export {
  checkAllCrawlCompletions,
  checkCrawlCompletion,
  getCrawlProgress,
} from "./crawl-completion-checker";
export { processCrawlQueue } from "./crawl-consumer";
export { persistCrawlResult } from "./crawl-persistence";
export type { CrawlRunnerOptions, CrawlRunnerResult } from "./crawl-runner";
export { runCrawl, runCrawlAsync } from "./crawl-runner";
export type { StoredCrawl } from "./crawl-store";
export {
  addCrawlResult,
  cancelCrawlJob,
  createCrawlJob,
  getCrawlErrors,
  getCrawlJob,
  getCrawlResults,
  getOngoingCrawls,
  incrementCompleted,
  isCrawlCancelled,
  setTotalCount,
  updateCrawlStatus,
} from "./crawl-store";

// Fire engine
export type {
  FireEngineFetchError,
  FireEngineFetchOutput,
  FireEngineFetchResult,
} from "./fire-engine-client";
export {
  fireEngineFetchHtml,
  isFireEngineEnabled,
} from "./fire-engine-client";
// HTML processing (Worker-native with container fallback)
export type {
  FilterLinksParams,
  FilterLinksResult,
  TransformHtmlParams,
} from "./html-processor";
export {
  extractAttributes,
  extractImages,
  extractLinks,
  extractMetadata,
  filterLinks,
  getInnerJson,
  getPdfMetadata,
  nativeHealth,
  postProcessMarkdown,
  transformHtml,
} from "./html-processor";
// Jobs
export { enqueueCrawlJob, enqueueScrapeJob } from "./jobs";

// Scrape
export { processScrapeQueue } from "./scrape-consumer";
export {
  captureScreenshotAndUpload,
  fetchContent,
  fetchDocumentWithWorker,
  fetchHtmlWithWorker,
  isDocumentUrl,
  resolveEngine,
} from "./scrape-fetcher";
export { processFormats } from "./scrape-format-processor";

// Scrape shared utilities
export {
  buildScrapeOptsForQueue,
  buildScrapeRunnerOptions,
  buildScrapeRunnerOptionsFromQueue,
} from "./scrape-options";
export type {
  ScrapeRunnerError,
  ScrapeRunnerOptions,
  ScrapeRunnerOutput,
  ScrapeRunnerResult,
} from "./scrape-runner";
export { runScrapeUrl } from "./scrape-runner";
