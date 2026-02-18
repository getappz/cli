/**
 * Authentication Context (equivalent to Firecrawl's AuthCreditUsageChunk / getACUC).
 *
 * A single function that validates an API key and returns all auth context
 * in one shot: team info, credits, rate limits, flags, and scopes.
 * This mirrors Firecrawl's `auth_credit_usage_chunk_42` Supabase RPC but
 * uses D1 JOINs instead.
 */

import type { RateLimiterMode } from "../types";
import { hashKey, isValidUuid, parseApiKey } from "./api-key";

// ---------------------------------------------------------------------------
// Types — mirrors Firecrawl's AuthCreditUsageChunk
// ---------------------------------------------------------------------------

export interface AuthCreditUsageChunk {
  /** The API key ID (api_keys.id). */
  api_key_id: number;
  /** Team ID that owns this key. */
  team_id: string;
  /** Human-readable key name. */
  key_name: string;
  /** JSON-parsed scopes, or null for "all". */
  scopes: string[] | null;

  // --- Credits ---
  /** Total credits available for the period. */
  price_credits: number;
  /** Credits already consumed. */
  credits_used: number;
  /** Remaining credits (price_credits - credits_used). */
  remaining_credits: number;

  // --- Rate limits (per minute) ---
  rate_limits: {
    crawl: number;
    scrape: number;
    search: number;
    map: number;
    extract: number;
    preview: number;
    crawlStatus: number;
    extractStatus: number;
  };

  /** Max concurrent jobs for this team. */
  concurrency: number;

  /** Team feature flags (parsed JSON). */
  flags: TeamFlags | null;
}

/** Team feature flags — mirrors Firecrawl's TeamFlags. */
export interface TeamFlags {
  ignoreRobots?: boolean;
  unblockedDomains?: string[];
  crawlTtlHours?: number;
  extractBeta?: boolean;
  agentBeta?: boolean;
  browserBeta?: boolean;
  bypassCreditChecks?: boolean;
}

export interface AuthResponse {
  success: true;
  team_id: string;
  chunk: AuthCreditUsageChunk;
}

export interface AuthError {
  success: false;
  error: string;
  status: number;
}

export type AuthResult = AuthResponse | AuthError;

// ---------------------------------------------------------------------------
// Default rate limits (used when team has no custom overrides)
// ---------------------------------------------------------------------------

const DEFAULT_RATE_LIMITS = {
  crawl: 15,
  scrape: 100,
  search: 100,
  map: 100,
  extract: 100,
  preview: 5,
  crawlStatus: 500,
  extractStatus: 500,
};

const DEFAULT_CONCURRENCY = 10;
const DEFAULT_CREDITS = 999_999;

// ---------------------------------------------------------------------------
// Main function: getAuthContext (equivalent to Firecrawl's getACUC)
// ---------------------------------------------------------------------------

/**
 * Validate an API key against D1 and return the full auth context.
 *
 * This performs the equivalent of Firecrawl's `auth_credit_usage_chunk_42`
 * Supabase RPC using a single D1 JOIN query across api_keys, teams, and
 * team_credits.
 *
 * @param db      D1Database binding
 * @param token   Raw Bearer token from the request
 * @param _mode   Rate limiter mode (reserved for per-mode rate limiting)
 * @returns       AuthResult with either the full context or an error
 */
export async function getAuthContext(
  db: D1Database,
  token: string,
  _mode?: RateLimiterMode,
): Promise<AuthResult> {
  // 1. Parse the key and compute its SHA-256 hash
  const parsed = parseApiKey(token);

  if (!isValidUuid(parsed.normalizedUuid)) {
    return {
      success: false,
      error: "Unauthorized: Invalid token",
      status: 401,
    };
  }

  // 2. Always look up by key_hash (no legacy plaintext fallback)
  const keyHash = await hashKey(parsed.fullKey);
  const row = await lookupByHash(db, keyHash);

  if (!row) {
    return {
      success: false,
      error: "Unauthorized: Invalid token",
      status: 401,
    };
  }

  // 3. Check if key is expired
  if (row.expires_at && new Date(row.expires_at) < new Date()) {
    return {
      success: false,
      error: "Unauthorized: API key has expired",
      status: 401,
    };
  }

  // 4. Parse scopes
  const scopes: string[] | null = row.scopes ? JSON.parse(row.scopes) : null;

  // 5. Update last_used_at (fire-and-forget; don't block auth)
  updateLastUsed(db, row.id).catch(() => {
    /* ignore errors */
  });

  // 6. Build the AuthCreditUsageChunk
  const credits = row.credits ?? DEFAULT_CREDITS;
  const flags: TeamFlags | null = row.flags ? JSON.parse(row.flags) : null;

  const chunk: AuthCreditUsageChunk = {
    api_key_id: row.id,
    team_id: row.team_id,
    key_name: row.name ?? "Default",
    scopes,

    price_credits: credits,
    credits_used: 0, // TODO: sum from billing_log for current period
    remaining_credits: credits,

    rate_limits: {
      crawl: row.rate_limit_crawl ?? DEFAULT_RATE_LIMITS.crawl,
      scrape: row.rate_limit_scrape ?? DEFAULT_RATE_LIMITS.scrape,
      search: row.rate_limit_search ?? DEFAULT_RATE_LIMITS.search,
      map: row.rate_limit_map ?? DEFAULT_RATE_LIMITS.map,
      extract: row.rate_limit_extract ?? DEFAULT_RATE_LIMITS.extract,
      preview: DEFAULT_RATE_LIMITS.preview,
      crawlStatus: DEFAULT_RATE_LIMITS.crawlStatus,
      extractStatus: DEFAULT_RATE_LIMITS.extractStatus,
    },
    concurrency: row.max_concurrency ?? DEFAULT_CONCURRENCY,
    flags,
  };

  return {
    success: true,
    team_id: row.team_id,
    chunk,
  };
}

// ---------------------------------------------------------------------------
// D1 queries
// ---------------------------------------------------------------------------

/** Shape of the JOIN result row. */
interface ApiKeyJoinRow {
  id: number;
  team_id: string;
  name: string | null;
  scopes: string | null;
  expires_at: string | null;

  // From team_credits
  credits: number | null;

  // From teams
  rate_limit_scrape: number | null;
  rate_limit_crawl: number | null;
  rate_limit_search: number | null;
  rate_limit_extract: number | null;
  rate_limit_map: number | null;
  max_concurrency: number | null;
  flags: string | null;
}

const JOIN_SQL = `
  SELECT
    k.id,
    k.team_id,
    k.name,
    k.scopes,
    k.expires_at,
    c.credits,
    t.rate_limit_scrape,
    t.rate_limit_crawl,
    t.rate_limit_search,
    t.rate_limit_extract,
    t.rate_limit_map,
    t.max_concurrency,
    t.flags
  FROM api_keys k
  LEFT JOIN team_credits c ON c.team_id = k.team_id
  LEFT JOIN teams t ON t.id = k.team_id
`;

async function lookupByHash(
  db: D1Database,
  keyHash: string,
): Promise<ApiKeyJoinRow | null> {
  return db
    .prepare(
      `${JOIN_SQL} WHERE k.key_hash = ? AND k.deleted_at IS NULL LIMIT 1`,
    )
    .bind(keyHash)
    .first<ApiKeyJoinRow>();
}

async function updateLastUsed(db: D1Database, keyId: number): Promise<void> {
  await db
    .prepare("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
    .bind(new Date().toISOString(), keyId)
    .run();
}
