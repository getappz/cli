/**
 * Search controller: Firecrawl-compatible /search endpoint.
 * Adapted from firecrawl/apps/api/src/controllers/v2/search.ts.
 *
 * Uses native Rust implementation for DuckDuckGo and SearXNG search.
 * - Primary: SearXNG (if SEARXNG_ENDPOINT env var is set)
 * - Fallback: DuckDuckGo HTML scraping
 *
 * NOTE: This implementation does NOT support:
 * - Fire Engine integration (would require separate service)
 * - Image search (DuckDuckGo images API)
 * - News search (would need separate API)
 * - Async result scraping (would need SCRAPE_QUEUE integration)
 */
import { parseSearchRequestBody } from "../contracts/search";
import { logger } from "../lib/logger";
import { executeSearch } from "../services/search-execute";
export async function searchController(c) {
    let rawBody;
    try {
        rawBody = await c.req.json();
    }
    catch {
        return c.json({
            success: false,
            error: "Invalid JSON body; expected { query: string }",
        }, 400);
    }
    const parsed = parseSearchRequestBody(rawBody);
    if (!parsed.ok) {
        return c.json({ success: false, error: parsed.error }, 400);
    }
    const { data: body } = parsed;
    const auth = c.get("auth");
    if (!auth) {
        return c.json({ success: false, error: "Unauthorized" }, 401);
    }
    try {
        const result = await executeSearch({
            body,
            teamId: auth.team_id,
            env: c.env,
            logger,
        });
        return c.json(result.response, 200);
    }
    catch (error) {
        logger.error("[search] execution failed", {
            error: error instanceof Error ? error.message : String(error),
            query: body.query,
            teamId: auth.team_id,
        });
        return c.json({
            success: false,
            error: error instanceof Error ? error.message : "Search execution failed",
        }, 500);
    }
}
//# sourceMappingURL=search.js.map