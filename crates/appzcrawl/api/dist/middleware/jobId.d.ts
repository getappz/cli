import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
export declare function isValidJobId(jobId: string | undefined): jobId is string;
export declare function validateJobIdParam(c: Context<AppEnv>, next: Next): Promise<void> | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">);
//# sourceMappingURL=jobId.d.ts.map