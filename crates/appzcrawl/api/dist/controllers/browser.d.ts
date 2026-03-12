/**
 * Browser API controllers.
 * Implements Firecrawl-compatible browser session management endpoints.
 *
 * Endpoints:
 * - POST /v2/browser - Create a browser session
 * - POST /v2/browser/execute - Execute actions in a session
 * - DELETE /v2/browser - Delete a browser session
 * - GET /v2/browser/:browserId - Get session status
 */
import type { Context } from "hono";
import type { AppEnv } from "../types";
export declare function browserCreateController(c: Context<AppEnv>): Promise<Response>;
export declare function browserExecuteController(c: Context<AppEnv>): Promise<Response>;
export declare function browserDeleteController(c: Context<AppEnv>): Promise<Response>;
export declare function browserStatusController(c: Context<AppEnv>): Promise<Response>;
//# sourceMappingURL=browser.d.ts.map