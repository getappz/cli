import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
export declare function requestTimingMiddleware(version: string): (c: Context<AppEnv>, next: Next) => Promise<void>;
//# sourceMappingURL=timing.d.ts.map