/**
 * Authentication Context (equivalent to Firecrawl's AuthCreditUsageChunk / getACUC).
 *
 * A single function that validates an API key and returns all auth context
 * in one shot: team info, credits, rate limits, flags, and scopes.
 * This mirrors Firecrawl's `auth_credit_usage_chunk_42` Supabase RPC but
 * uses D1 JOINs instead.
 */
import { hashKey, isValidUuid, parseApiKey } from "./api-key";
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
export async function getAuthContext(db, token, _mode) {
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
    const scopes = row.scopes ? JSON.parse(row.scopes) : null;
    // 5. Update last_used_at (fire-and-forget; don't block auth)
    updateLastUsed(db, row.id).catch(() => {
        /* ignore errors */
    });
    // 6. Build the AuthCreditUsageChunk
    const credits = row.credits ?? DEFAULT_CREDITS;
    const flags = row.flags ? JSON.parse(row.flags) : null;
    const chunk = {
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
async function lookupByHash(db, keyHash) {
    return db
        .prepare(`${JOIN_SQL} WHERE k.key_hash = ? AND k.deleted_at IS NULL LIMIT 1`)
        .bind(keyHash)
        .first();
}
async function updateLastUsed(db, keyId) {
    await db
        .prepare("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
        .bind(new Date().toISOString(), keyId)
        .run();
}
//# sourceMappingURL=auth-context.js.map