/**
 * Team config service: loads team configuration from D1 `teams` table.
 * Adapted from Firecrawl's auth.ts getACUC() + TeamFlags pattern.
 *
 * Falls back to defaults if team row doesn't exist (backwards-compatible
 * with existing API key-only auth).
 */
export interface TeamFlags {
    /** Override zero-data-retention cost. */
    zdrCost?: number;
    /** Skip credit checks for this team. */
    bypassCreditChecks?: boolean;
    /** Allow specific blocked domains for this team. */
    unblockedDomains?: string[];
    /** Override robots.txt checking on scrape. */
    ignoreRobots?: boolean;
    /** Custom crawl TTL override. */
    crawlTtlHours?: number;
}
export interface TeamConfig {
    id: string;
    name: string | null;
    hmacSecret: string | null;
    rateLimits: {
        scrape: number;
        crawl: number;
        search: number;
        extract: number;
        map: number;
    };
    maxConcurrency: number;
    crawlTtlHours: number;
    flags: TeamFlags;
    autoRecharge: boolean;
    autoRechargeThreshold: number | null;
}
/**
 * Get team configuration from D1. Returns defaults if team row doesn't exist.
 * Firecrawl caches this in Redis for 10 minutes; we rely on D1's fast reads
 * (same region as the worker).
 */
export declare function getTeamConfig(db: D1Database, teamId: string): Promise<TeamConfig>;
//# sourceMappingURL=team-config.d.ts.map