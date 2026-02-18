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
import { parseBrowserCreateRequest, parseBrowserDeleteRequest, parseBrowserExecuteRequest, } from "../contracts/browser";
import { logger } from "../lib/logger";
/** Validate auth + browser service availability. Returns null + sends error response on failure. */
function getControllerContext(c) {
    const auth = c.get("auth");
    if (!auth) {
        c.json({ success: false, error: "Unauthorized" }, 401);
        return null;
    }
    const browserService = c.env.BROWSER_SERVICE;
    if (!browserService) {
        c.json({
            success: false,
            error: "Browser API is not available (BROWSER_SERVICE not configured)",
        }, 503);
        return null;
    }
    return { auth, browserService };
}
async function parseBody(c) {
    try {
        return await c.req.json();
    }
    catch {
        return {};
    }
}
/** Map common session errors to HTTP status codes. */
function sessionErrorStatus(error) {
    if (error === "Session not found")
        return 404;
    if (error === "Forbidden")
        return 403;
    if (error === "Session has been destroyed" || error === "Session has expired")
        return 410;
    return 500;
}
// ============================================================================
// POST /v2/browser - Create a browser session
// ============================================================================
export async function browserCreateController(c) {
    const ctx = getControllerContext(c);
    if (!ctx)
        return c.res;
    const rawBody = await parseBody(c);
    const parsed = parseBrowserCreateRequest(rawBody);
    if (!parsed.ok) {
        return c.json({ success: false, error: parsed.error }, 400);
    }
    const { ttlTotal, ttlWithoutActivity, streamWebView } = parsed.data;
    const { auth, browserService } = ctx;
    logger.info("[browser] creating session", {
        teamId: auth.team_id,
        ttlTotal,
        streamWebView,
    });
    try {
        const result = await browserService.createSession({
            teamId: auth.team_id,
            ttlTotal,
            ttlIdle: ttlWithoutActivity,
            streamWebView,
        });
        if (!result.success) {
            logger.warn("[browser] session creation failed", {
                teamId: auth.team_id,
                error: result.error,
            });
            return c.json({ success: false, error: result.error }, 500);
        }
        logger.info("[browser] session created", {
            teamId: auth.team_id,
            browserId: result.session.id,
        });
        return c.json({
            success: true,
            browserId: result.session.id,
            ttlRemaining: result.session.ttlTotal,
        });
    }
    catch (err) {
        logger.error("[browser] session creation error", {
            teamId: auth.team_id,
            error: err,
        });
        return c.json({
            success: false,
            error: err instanceof Error
                ? err.message
                : "Failed to create browser session",
        }, 500);
    }
}
// ============================================================================
// POST /v2/browser/execute - Execute actions in a session
// ============================================================================
export async function browserExecuteController(c) {
    const ctx = getControllerContext(c);
    if (!ctx)
        return c.res;
    let rawBody;
    try {
        rawBody = await c.req.json();
    }
    catch {
        return c.json({
            success: false,
            error: "Invalid JSON body; expected browser execute request object",
        }, 400);
    }
    const parsed = parseBrowserExecuteRequest(rawBody);
    if (!parsed.ok) {
        return c.json({ success: false, error: parsed.error }, 400);
    }
    const { browserId, code, language, actions } = parsed.data;
    const { auth, browserService } = ctx;
    // Build actions array
    let actionsToExecute = [];
    if (code) {
        if (language === "python") {
            return c.json({
                success: false,
                error: "Python code execution requires sandbox environment. Use 'actions' with 'evaluate' for JavaScript.",
            }, 400);
        }
        actionsToExecute = [{ action: "evaluate", code }];
    }
    else if (actions && actions.length > 0) {
        actionsToExecute = actions;
    }
    logger.info("[browser] executing actions", {
        teamId: auth.team_id,
        browserId,
        actionCount: actionsToExecute.length,
        actionTypes: actionsToExecute.map((a) => a.action),
    });
    try {
        const result = await browserService.executeSessionActions({
            sessionId: browserId,
            teamId: auth.team_id,
            actions: actionsToExecute,
        });
        if (!result.success) {
            const status = sessionErrorStatus(result.error);
            return c.json({
                success: false,
                error: result.error,
                results: result.results,
                stoppedAtIndex: result.stoppedAtIndex,
            }, status);
        }
        // Firecrawl compatibility: single evaluate → return result as string
        if (code && result.results.length === 1) {
            const singleResult = result.results[0];
            return c.json({
                success: true,
                result: typeof singleResult.data === "string"
                    ? singleResult.data
                    : JSON.stringify(singleResult.data),
                url: result.url,
                title: result.title,
            });
        }
        return c.json({
            success: true,
            results: result.results,
            url: result.url,
            title: result.title,
        });
    }
    catch (err) {
        logger.error("[browser] action execution error", {
            teamId: auth.team_id,
            browserId,
            error: err,
        });
        return c.json({
            success: false,
            error: err instanceof Error ? err.message : "Failed to execute actions",
        }, 500);
    }
}
// ============================================================================
// DELETE /v2/browser - Delete a browser session
// ============================================================================
export async function browserDeleteController(c) {
    const ctx = getControllerContext(c);
    if (!ctx)
        return c.res;
    let rawBody;
    try {
        rawBody = await c.req.json();
    }
    catch {
        return c.json({
            success: false,
            error: "Invalid JSON body; expected browser delete request object",
        }, 400);
    }
    const parsed = parseBrowserDeleteRequest(rawBody);
    if (!parsed.ok) {
        return c.json({ success: false, error: parsed.error }, 400);
    }
    const { browserId } = parsed.data;
    const { auth, browserService } = ctx;
    logger.info("[browser] deleting session", {
        teamId: auth.team_id,
        browserId,
    });
    try {
        const result = await browserService.destroySessionById({
            sessionId: browserId,
            teamId: auth.team_id,
        });
        if (!result.success) {
            const status = result.error === "Forbidden" ? 403 : 500;
            return c.json({ success: false, error: result.error }, status);
        }
        logger.info("[browser] session deleted", {
            teamId: auth.team_id,
            browserId,
        });
        return c.json({ success: true });
    }
    catch (err) {
        logger.error("[browser] session deletion error", {
            teamId: auth.team_id,
            browserId,
            error: err,
        });
        return c.json({
            success: false,
            error: err instanceof Error
                ? err.message
                : "Failed to delete browser session",
        }, 500);
    }
}
// ============================================================================
// GET /v2/browser/:browserId - Get session status
// ============================================================================
export async function browserStatusController(c) {
    const ctx = getControllerContext(c);
    if (!ctx)
        return c.res;
    const browserId = c.req.param("browserId");
    if (!browserId) {
        return c.json({ success: false, error: "Missing browserId parameter" }, 400);
    }
    const { auth, browserService } = ctx;
    try {
        const result = await browserService.getSessionInfo({
            sessionId: browserId,
            teamId: auth.team_id,
        });
        if (!result.success) {
            return c.json({ success: false, error: result.error }, sessionErrorStatus(result.error));
        }
        const session = result.session;
        const now = Date.now();
        const ttlRemaining = Math.max(0, session.ttlTotal - Math.floor((now - session.createdAt) / 1000));
        return c.json({
            success: true,
            browserId: session.id,
            status: session.destroyed
                ? "destroyed"
                : ttlRemaining > 0
                    ? "active"
                    : "expired",
            createdAt: new Date(session.createdAt).toISOString(),
            lastActivity: new Date(session.lastActivity).toISOString(),
            ttlTotal: session.ttlTotal,
            ttlRemaining,
            streamWebView: session.streamWebView,
        });
    }
    catch (err) {
        logger.error("[browser] get session error", {
            teamId: auth.team_id,
            browserId,
            error: err,
        });
        return c.json({
            success: false,
            error: err instanceof Error ? err.message : "Failed to get browser session",
        }, 500);
    }
}
//# sourceMappingURL=browser.js.map