import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function queueStatusController(c: Context<AppEnv>): Promise<Response & import("hono").TypedResponse<{
    success: true;
    queue: never[];
}, import("hono/utils/http-status").ContentfulStatusCode, "json">>;
//# sourceMappingURL=queue-status.d.ts.map