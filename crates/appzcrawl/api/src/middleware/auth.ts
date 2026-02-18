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
import { type AuthCreditUsageChunk, getAuthContext } from "../lib/auth-context";
import type { AppEnv, RateLimiterMode } from "../types";

// ---------------------------------------------------------------------------
// Token extraction
// ---------------------------------------------------------------------------

/** Get API key from Authorization header or query param. */
function getApiKey(c: Context<AppEnv>): string | undefined {
  const auth = c.req.header("Authorization");
  if (auth?.startsWith("Bearer ")) return auth.slice(7).trim();
  // Also support WebSocket protocol header (like Firecrawl)
  const wsProto = c.req.header("Sec-WebSocket-Protocol");
  if (wsProto) return wsProto.trim();
  return c.req.query("api_key") ?? undefined;
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/**
 * Auth middleware factory (mirrors Firecrawl's authMiddleware pattern).
 *
 * Sets on context:
 * - `auth`    → { team_id: string }
 * - `acuc`    → AuthCreditUsageChunk (full context with credits, rate limits, flags)
 * - `account` → { remainingCredits: number }
 */
export function authMiddleware(rateLimiterMode: RateLimiterMode) {
  return async (c: Context<AppEnv>, next: Next) => {
    const apiKey = getApiKey(c);
    if (!apiKey) {
      return c.json(
        { success: false, error: "Unauthorized: Missing API key" },
        401,
      );
    }

    const env = c.env;

    // -------------------------------------------------------------------
    // Dev bypass: env.API_KEY for local / single-team development
    // (equivalent to Firecrawl's USE_DB_AUTHENTICATION=false bypass)
    // -------------------------------------------------------------------
    if (env.API_KEY && apiKey === env.API_KEY) {
      const devChunk = buildDevChunk();
      c.set("auth", { team_id: "default-team" });
      c.set("acuc", devChunk);
      c.set("account", { remainingCredits: devChunk.remaining_credits });
      return next();
    }

    // -------------------------------------------------------------------
    // Full authentication via D1 (equivalent to Firecrawl's getACUC)
    // -------------------------------------------------------------------
    const result = await getAuthContext(env.DB, apiKey, rateLimiterMode);

    if (!result.success) {
      return c.json(
        { success: false, error: result.error },
        result.status as 401 | 403 | 429,
      );
    }

    const { team_id, chunk } = result;

    // Check scope authorization for the current mode
    if (chunk.scopes) {
      const modeScope = rateLimiterModeToScope(rateLimiterMode);
      if (modeScope && !chunk.scopes.includes(modeScope)) {
        return c.json(
          {
            success: false,
            error: `Unauthorized: API key does not have the "${modeScope}" scope`,
          },
          403,
        );
      }
    }

    c.set("auth", { team_id });
    c.set("acuc", chunk);
    c.set("account", { remainingCredits: chunk.remaining_credits });

    return next();
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Map RateLimiterMode to a scope string for API key scope checks. */
function rateLimiterModeToScope(mode: RateLimiterMode): string | null {
  const map: Record<string, string> = {
    crawl: "crawl",
    crawlStatus: "crawl",
    scrape: "scrape",
    search: "search",
    map: "map",
    extract: "extract",
    extractStatus: "extract",
    preview: "scrape",
  };
  return map[mode] ?? null;
}

/** Build a dev-mode ACUC chunk with unlimited access (like Firecrawl's mockACUC). */
function buildDevChunk(): AuthCreditUsageChunk {
  return {
    api_key_id: 0,
    team_id: "default-team",
    key_name: "Dev Key",
    scopes: null,
    price_credits: 999_999_999,
    credits_used: 0,
    remaining_credits: 999_999_999,
    rate_limits: {
      crawl: 999_999,
      scrape: 999_999,
      search: 999_999,
      map: 999_999,
      extract: 999_999,
      preview: 999_999,
      crawlStatus: 999_999,
      extractStatus: 999_999,
    },
    concurrency: 999_999,
    flags: null,
  };
}
