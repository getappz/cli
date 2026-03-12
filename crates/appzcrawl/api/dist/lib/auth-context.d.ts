/**
 * Authentication Context (equivalent to Firecrawl's AuthCreditUsageChunk / getACUC).
 *
 * A single function that validates an API key and returns all auth context
 * in one shot: team info, credits, rate limits, flags, and scopes.
 * This mirrors Firecrawl's `auth_credit_usage_chunk_42` Supabase RPC but
 * uses D1 JOINs instead.
 */
import type { RateLimiterMode } from "../types";
export interface AuthCreditUsageChunk {
    /** The API key ID (api_keys.id). */
    api_key_id: number;
    /** Team ID that owns this key. */
    team_id: string;
    /** Human-readable key name. */
    key_name: string;
    /** JSON-parsed scopes, or null for "all". */
    scopes: string[] | null;
    /** Total credits available for the period. */
    price_credits: number;
    /** Credits already consumed. */
    credits_used: number;
    /** Remaining credits (price_credits - credits_used). */
    remaining_credits: number;
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
export declare function getAuthContext(db: D1Database, token: string, _mode?: RateLimiterMode): Promise<AuthResult>;
//# sourceMappingURL=auth-context.d.ts.map