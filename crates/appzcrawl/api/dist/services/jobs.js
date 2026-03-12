/**
 * Job enqueue stub. Replace with Cloudflare Queues + D1 when implementing scrape/crawl workers.
 * R2 (env.BUCKET) can be used by workers to store scraped content.
 */
export async function enqueueScrapeJob(_jobId, _payload, _env) {
    // TODO: send to Cloudflare Queue; consumer writes status to D1, stores output in R2
}
export async function enqueueCrawlJob(_jobId, _payload, _env) {
    // TODO: send to Cloudflare Queue; consumer uses D1 + R2
}
//# sourceMappingURL=jobs.js.map