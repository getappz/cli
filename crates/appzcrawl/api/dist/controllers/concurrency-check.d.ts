import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function concurrencyCheckController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    canRun: true;
    current: number;
    limit: number;
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
//# sourceMappingURL=concurrency-check.d.ts.map