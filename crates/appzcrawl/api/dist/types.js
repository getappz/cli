/** Rate limiter mode for auth middleware (used for future rate limiting). */
export var RateLimiterMode;
(function (RateLimiterMode) {
    RateLimiterMode["Crawl"] = "crawl";
    RateLimiterMode["CrawlStatus"] = "crawlStatus";
    RateLimiterMode["Scrape"] = "scrape";
    RateLimiterMode["ScrapeAgentPreview"] = "scrapeAgentPreview";
    RateLimiterMode["Preview"] = "preview";
    RateLimiterMode["Search"] = "search";
    RateLimiterMode["Map"] = "map";
    RateLimiterMode["Extract"] = "extract";
    RateLimiterMode["ExtractStatus"] = "extractStatus";
    RateLimiterMode["ExtractAgentPreview"] = "extractAgentPreview";
})(RateLimiterMode || (RateLimiterMode = {}));
export const UNSUPPORTED_SITE_MESSAGE = "This site is not supported. Please check the blocklist or contact support.";
//# sourceMappingURL=types.js.map