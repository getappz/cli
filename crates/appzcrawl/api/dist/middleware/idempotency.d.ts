import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
/**
 * Idempotency middleware (Firecrawl-compatible).
 * When x-idempotency-key header is present:
 * - Validates key is a UUID; if not, returns 409.
 * - Checks D1 idempotency_keys table; if key exists, returns 409.
 * - Inserts key before running controller so duplicate requests are rejected.
 */
export declare function idempotencyMiddleware(c: Context<AppEnv>, next: Next): Promise<void | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 409, "json">)>;
//# sourceMappingURL=idempotency.d.ts.map