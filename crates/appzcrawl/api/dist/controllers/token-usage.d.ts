import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function tokenUsageController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    tokensUsed: number;
    remainingTokens: number;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
export declare function tokenUsageHistoricalController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    data: never[];
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
//# sourceMappingURL=token-usage.d.ts.map