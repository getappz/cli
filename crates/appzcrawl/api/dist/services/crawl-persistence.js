/**
 * Shared crawl document persistence logic.
 * Handles serializing ScrapeRunnerDocument to JSON and storing in D1/R2.
 * Used by crawl-runner (inline mode + fan-out fallback) and scrape-consumer.
 */
import { logger } from "../lib/logger";
import { addCrawlResult, incrementCompleted } from "./crawl-store";
/** Maximum document JSON size to store inline in D1 (256 KB). Larger docs go to R2. */
const MAX_INLINE_SIZE = 256 * 1024;
/** Build a Firecrawl-compatible document object for storage. */
function buildCrawlDocument(doc) {
    return {
        url: doc.url,
        markdown: doc.markdown,
        html: doc.html,
        rawHtml: doc.rawHtml,
        links: doc.links,
        images: doc.images,
        screenshot: doc.screenshot,
        metadata: doc.metadata,
        branding: doc.branding,
    };
}
/**
 * Persist a scrape result (success or failure) as a crawl result in D1/R2.
 * Handles inline vs R2 storage based on document size.
 */
export async function persistCrawlResult(env, crawlId, url, result) {
    if (!result.success) {
        await addCrawlResult(env.DB, {
            id: crypto.randomUUID(),
            crawlId,
            url,
            status: "failed",
            error: result.error,
            statusCode: result.statusCode,
        });
        return;
    }
    const doc = result.document;
    const crawlDoc = buildCrawlDocument(doc);
    const docJson = JSON.stringify(crawlDoc);
    let r2Key;
    let documentJson;
    if (docJson.length > MAX_INLINE_SIZE && env.BUCKET) {
        r2Key = `crawl/${crawlId}/${crypto.randomUUID()}.json`;
        try {
            await env.BUCKET.put(r2Key, docJson, {
                httpMetadata: { contentType: "application/json" },
            });
        }
        catch (e) {
            logger.warn("[crawl-persistence] R2 upload failed, storing inline", {
                crawlId,
                url,
                error: e instanceof Error ? e.message : String(e),
            });
            r2Key = undefined;
            documentJson = docJson;
        }
    }
    else {
        documentJson = docJson;
    }
    await addCrawlResult(env.DB, {
        id: crypto.randomUUID(),
        crawlId,
        url: doc.url,
        status: "success",
        documentJson,
        r2Key,
        statusCode: doc.statusCode,
    });
    await incrementCompleted(env.DB, crawlId);
}
//# sourceMappingURL=crawl-persistence.js.map