/**
 * Team config service: loads team configuration from D1 `teams` table.
 * Adapted from Firecrawl's auth.ts getACUC() + TeamFlags pattern.
 *
 * Falls back to defaults if team row doesn't exist (backwards-compatible
 * with existing API key-only auth).
 */

import { logger } from "../lib/logger";

// ---------------------------------------------------------------------------
// Types (adapted from Firecrawl's TeamFlags)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_CONFIG: TeamConfig = {
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

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

interface TeamRow {
  id: string;
  name: string | null;
  hmac_secret: string | null;
  rate_limit_scrape: number | null;
  rate_limit_crawl: number | null;
  rate_limit_search: number | null;
  rate_limit_extract: number | null;
  rate_limit_map: number | null;
  max_concurrency: number | null;
  crawl_ttl_hours: number | null;
  flags: string | null;
  auto_recharge: number;
  auto_recharge_threshold: number | null;
}

/**
 * Get team configuration from D1. Returns defaults if team row doesn't exist.
 * Firecrawl caches this in Redis for 10 minutes; we rely on D1's fast reads
 * (same region as the worker).
 */
export async function getTeamConfig(
  db: D1Database,
  teamId: string,
): Promise<TeamConfig> {
  try {
    const row = await db
      .prepare("SELECT * FROM teams WHERE id = ?")
      .bind(teamId)
      .first<TeamRow>();

    if (!row) {
      return { ...DEFAULT_CONFIG, id: teamId };
    }

    let flags: TeamFlags = {};
    if (row.flags) {
      try {
        flags = JSON.parse(row.flags);
      } catch {
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
  } catch (e) {
    // Table may not exist yet; return defaults
    logger.warn("[team-config] failed to load team config", {
      teamId,
      error: e instanceof Error ? e.message : String(e),
    });
    return { ...DEFAULT_CONFIG, id: teamId };
  }
}
