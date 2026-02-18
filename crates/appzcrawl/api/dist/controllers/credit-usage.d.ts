import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function creditUsageController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    creditsUsed: number;
    remainingCredits: number;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
export declare function creditUsageHistoricalController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    data: never[];
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
//# sourceMappingURL=credit-usage.d.ts.map