import type { Context, Next } from "hono";
import type { AuthCreditUsageChunk } from "../lib/auth-context";
import type { AppEnv } from "../types";

/**
 * Credit-check middleware.
 *
 * Uses the ACUC chunk already set by authMiddleware (no extra D1 query).
 * Equivalent to Firecrawl's checkTeamCredits().
 *
 * @param _minimum  Reserved for future per-endpoint minimum credit checks.
 */
export function checkCreditsMiddleware(_minimum?: number) {
  return async (c: Context<AppEnv>, next: Next) => {
    const acuc = c.get("acuc") as AuthCreditUsageChunk | undefined;

    // No ACUC → dev mode or auth bypass; skip check
    if (!acuc) return next();

    // Team has bypassCreditChecks flag → skip
    if (acuc.flags?.bypassCreditChecks) return next();

    if (acuc.remaining_credits <= 0) {
      return c.json(
        {
          success: false,
          error:
            "Insufficient credits. Please upgrade your plan or add more credits.",
        },
        402,
      );
    }

    return next();
  };
}
