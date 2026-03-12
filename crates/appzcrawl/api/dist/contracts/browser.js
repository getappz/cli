/**
 * Firecrawl-compatible Browser API contracts.
 * Request/response aligned to Firecrawl v2 Browser endpoints.
 */
import { z } from "zod";
// ============================================================================
// POST /v2/browser - Create a browser session
// ============================================================================
export const browserCreateRequestSchema = z.object({
    /** Total TTL in seconds (default: 300, min: 30, max: 3600) */
    ttlTotal: z.number().min(30).max(3600).optional().default(300),
    /** Idle TTL in seconds (optional, min: 10, max: 3600) */
    ttlWithoutActivity: z.number().min(10).max(3600).optional(),
    /** Whether to enable web view streaming (future feature) */
    streamWebView: z.boolean().optional().default(false),
});
// ============================================================================
// POST /v2/browser/execute - Execute code/actions in a browser session
// ============================================================================
/**
 * Browser action schema.
 * Note: In Firecrawl, this supports Python/JS code execution via sandbox.
 * In appzcrawl, we support predefined actions instead.
 */
const browserActionSchema = z.discriminatedUnion("action", [
    // Navigate
    z.object({
        action: z.literal("navigate"),
        url: z.string().url(),
        waitUntil: z.enum(["load", "domcontentloaded", "networkidle"]).optional(),
        timeout: z.number().positive().optional(),
    }),
    // Click
    z.object({
        action: z.literal("click"),
        selector: z.string().min(1),
        timeout: z.number().positive().optional(),
    }),
    // Type
    z.object({
        action: z.literal("type"),
        selector: z.string().min(1),
        text: z.string(),
        delay: z.number().positive().optional(),
        clear: z.boolean().optional(),
        timeout: z.number().positive().optional(),
    }),
    // Screenshot
    z.object({
        action: z.literal("screenshot"),
        fullPage: z.boolean().optional(),
        selector: z.string().optional(),
        format: z.enum(["png", "jpeg", "webp"]).optional(),
        quality: z.number().min(0).max(100).optional(),
        timeout: z.number().positive().optional(),
    }),
    // Evaluate JavaScript
    z.object({
        action: z.literal("evaluate"),
        code: z.string().min(1).max(100000),
        timeout: z.number().positive().optional(),
    }),
    // Wait for selector
    z.object({
        action: z.literal("waitForSelector"),
        selector: z.string().min(1),
        state: z.enum(["visible", "hidden", "attached", "detached"]).optional(),
        timeout: z.number().positive().optional(),
    }),
    // Scroll
    z.object({
        action: z.literal("scroll"),
        direction: z.enum(["up", "down", "top", "bottom"]).optional(),
        amount: z.number().positive().optional(),
        selector: z.string().optional(),
        timeout: z.number().positive().optional(),
    }),
    // Extract
    z.object({
        action: z.literal("extract"),
        type: z.enum(["text", "html", "attribute", "list"]),
        selector: z.string().min(1),
        attribute: z.string().optional(),
        timeout: z.number().positive().optional(),
    }),
    // Navigation actions
    z.object({
        action: z.literal("goBack"),
        timeout: z.number().positive().optional(),
    }),
    z.object({
        action: z.literal("goForward"),
        timeout: z.number().positive().optional(),
    }),
    z.object({
        action: z.literal("reload"),
        timeout: z.number().positive().optional(),
    }),
    // Get content
    z.object({
        action: z.literal("getContent"),
        type: z.enum(["html", "text"]).optional(),
        timeout: z.number().positive().optional(),
    }),
    // Get URL/title
    z.object({
        action: z.literal("getUrl"),
        timeout: z.number().positive().optional(),
    }),
    z.object({
        action: z.literal("getTitle"),
        timeout: z.number().positive().optional(),
    }),
]);
/**
 * Firecrawl-compatible execute request.
 * Firecrawl uses code + language; we support both code and actions.
 */
export const browserExecuteRequestSchema = z
    .object({
    /** Session ID */
    browserId: z.string().min(1),
    /** Code to execute (Firecrawl compatibility - converted to evaluate action) */
    code: z.string().min(1).max(100000).optional(),
    /** Language for code execution (Firecrawl compatibility) */
    language: z.enum(["python", "js"]).optional().default("js"),
    /** Actions to execute (appzcrawl extension) */
    actions: z.array(browserActionSchema).optional(),
})
    .refine((data) => data.code !== undefined || (data.actions && data.actions.length > 0), {
    message: "Either 'code' or 'actions' must be provided",
});
// ============================================================================
// DELETE /v2/browser - Delete a browser session
// ============================================================================
export const browserDeleteRequestSchema = z.object({
    /** Session ID to delete */
    browserId: z.string().min(1),
});
// ============================================================================
// Helper functions
// ============================================================================
export function parseBrowserCreateRequest(raw) {
    const parsed = browserCreateRequestSchema.safeParse(raw);
    if (!parsed.success) {
        const first = parsed.error.flatten().formErrors[0] ?? parsed.error.message;
        return { ok: false, error: first };
    }
    return { ok: true, data: parsed.data };
}
export function parseBrowserExecuteRequest(raw) {
    const parsed = browserExecuteRequestSchema.safeParse(raw);
    if (!parsed.success) {
        const first = parsed.error.flatten().formErrors[0] ?? parsed.error.message;
        return { ok: false, error: first };
    }
    return { ok: true, data: parsed.data };
}
export function parseBrowserDeleteRequest(raw) {
    const parsed = browserDeleteRequestSchema.safeParse(raw);
    if (!parsed.success) {
        const first = parsed.error.flatten().formErrors[0] ?? parsed.error.message;
        return { ok: false, error: first };
    }
    return { ok: true, data: parsed.data };
}
//# sourceMappingURL=browser.js.map