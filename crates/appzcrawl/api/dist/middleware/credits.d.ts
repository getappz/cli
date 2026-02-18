import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
/**
 * Credit-check middleware.
 *
 * Uses the ACUC chunk already set by authMiddleware (no extra D1 query).
 * Equivalent to Firecrawl's checkTeamCredits().
 *
 * @param _minimum  Reserved for future per-endpoint minimum credit checks.
 */
export declare function checkCreditsMiddleware(_minimum?: number): (c: Context<AppEnv>, next: Next) => Promise<void | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 402, "json">)>;
//# sourceMappingURL=credits.d.ts.map