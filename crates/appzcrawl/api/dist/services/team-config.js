/**
 * Team config service: loads team configuration from D1 `teams` table.
 * Adapted from Firecrawl's auth.ts getACUC() + TeamFlags pattern.
 *
 * Falls back to defaults if team row doesn't exist (backwards-compatible
 * with existing API key-only auth).
 */
import { logger } from "../lib/logger";
// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------
const DEFAULT_CONFIG = {
    id: "",
    name: null,
    hmacSecret: null,
    rateLimits: {
        scrape: 100,
        crawl: 15,
        search: 100,
        extract: 100,
        map: 100,
    },
    maxConcurrency: 10,
    crawlTtlHours: 24,
    flags: {},
    autoRecharge: false,
    autoRechargeThreshold: null,
};
/**
 * Get team configuration from D1. Returns defaults if team row doesn't exist.
 * Firecrawl caches this in Redis for 10 minutes; we rely on D1's fast reads
 * (same region as the worker).
 */
export async function getTeamConfig(db, teamId) {
    try {
        const row = await db
            .prepare("SELECT * FROM teams WHERE id = ?")
            .bind(teamId)
            .first();
        if (!row) {
            return { ...DEFAULT_CONFIG, id: teamId };
        }
        let flags = {};
        if (row.flags) {
            try {
                flags = JSON.parse(row.flags);
            }
            catch {
                logger.warn("[team-config] failed to parse flags JSON", { teamId });
            }
        }
        return {
            id: row.id,
            name: row.name,
            hmacSecret: row.hmac_secret,
            rateLimits: {
                scrape: row.rate_limit_scrape ?? DEFAULT_CONFIG.rateLimits.scrape,
                crawl: row.rate_limit_crawl ?? DEFAULT_CONFIG.rateLimits.crawl,
                search: row.rate_limit_search ?? DEFAULT_CONFIG.rateLimits.search,
                extract: row.rate_limit_extract ?? DEFAULT_CONFIG.rateLimits.extract,
                map: row.rate_limit_map ?? DEFAULT_CONFIG.rateLimits.map,
            },
            maxConcurrency: row.max_concurrency ?? DEFAULT_CONFIG.maxConcurrency,
            crawlTtlHours: row.crawl_ttl_hours ?? DEFAULT_CONFIG.crawlTtlHours,
            flags,
            autoRecharge: Boolean(row.auto_recharge),
            autoRechargeThreshold: row.auto_recharge_threshold,
        };
    }
    catch (e) {
        // Table may not exist yet; return defaults
        logger.warn("[team-config] failed to load team config", {
            teamId,
            error: e instanceof Error ? e.message : String(e),
        });
        return { ...DEFAULT_CONFIG, id: teamId };
    }
}
//# sourceMappingURL=team-config.js.map