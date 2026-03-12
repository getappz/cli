/**
 * Authentication middleware — enterprise-level API key validation.
 *
 * Adapted from Firecrawl's auth.ts + authMiddleware in routes/shared.ts.
 *
 * Flow:
 * 1. Extract token from Authorization header or query param
 * 2. Dev bypass: if env.API_KEY matches, authenticate as default-team
 * 3. Full auth: call getAuthContext() which does a JOIN across
 *    api_keys + teams + team_credits (equivalent to Firecrawl's ACUC RPC)
 * 4. Set auth context, account info, and ACUC chunk on the request
 */
import type { Context, Next } from "hono";
import type { AppEnv, RateLimiterMode } from "../types";
/**
 * Auth middleware factory (mirrors Firecrawl's authMiddleware pattern).
 *
 * Sets on context:
 * - `auth`    → { team_id: string }
 * - `acuc`    → AuthCreditUsageChunk (full context with credits, rate limits, flags)
 * - `account` → { remainingCredits: number }
 */
export declare function authMiddleware(rateLimiterMode: RateLimiterMode): (c: Context<AppEnv>, next: Next) => Promise<void | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 403 | 401 | 429, "json">)>;
//# sourceMappingURL=auth.d.ts.map