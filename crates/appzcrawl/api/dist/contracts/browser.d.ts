/**
 * Firecrawl-compatible Browser API contracts.
 * Request/response aligned to Firecrawl v2 Browser endpoints.
 */
import { z } from "zod";
export declare const browserCreateRequestSchema: z.ZodObject<{
    /** Total TTL in seconds (default: 300, min: 30, max: 3600) */
    ttlTotal: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    /** Idle TTL in seconds (optional, min: 10, max: 3600) */
    ttlWithoutActivity: z.ZodOptional<z.ZodNumber>;
    /** Whether to enable web view streaming (future feature) */
    streamWebView: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
}, "strip", z.ZodTypeAny, {
    ttlTotal: number;
    streamWebView: boolean;
    ttlWithoutActivity?: number | undefined;
}, {
    ttlTotal?: number | undefined;
    streamWebView?: boolean | undefined;
    ttlWithoutActivity?: number | undefined;
}>;
export type BrowserCreateRequest = z.infer<typeof browserCreateRequestSchema>;
export type BrowserCreateRequestInput = z.input<typeof browserCreateRequestSchema>;
export interface BrowserCreateResponseSuccess {
    success: true;
    /** Session ID (use for execute/delete) */
    browserId: string;
    /** Remaining TTL in seconds */
    ttlRemaining?: number;
}
export interface BrowserCreateResponseError {
    success: false;
    error: string;
}
export type BrowserCreateResponse = BrowserCreateResponseSuccess | BrowserCreateResponseError;
/**
 * Browser action schema.
 * Note: In Firecrawl, this supports Python/JS code execution via sandbox.
 * In appzcrawl, we support predefined actions instead.
 */
declare const browserActionSchema: z.ZodDiscriminatedUnion<"action", [z.ZodObject<{
    action: z.ZodLiteral<"navigate">;
    url: z.ZodString;
    waitUntil: z.ZodOptional<z.ZodEnum<["load", "domcontentloaded", "networkidle"]>>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    url: string;
    action: "navigate";
    timeout?: number | undefined;
    waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
}, {
    url: string;
    action: "navigate";
    timeout?: number | undefined;
    waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"click">;
    selector: z.ZodString;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    selector: string;
    action: "click";
    timeout?: number | undefined;
}, {
    selector: string;
    action: "click";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"type">;
    selector: z.ZodString;
    text: z.ZodString;
    delay: z.ZodOptional<z.ZodNumber>;
    clear: z.ZodOptional<z.ZodBoolean>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    text: string;
    selector: string;
    action: "type";
    timeout?: number | undefined;
    clear?: boolean | undefined;
    delay?: number | undefined;
}, {
    text: string;
    selector: string;
    action: "type";
    timeout?: number | undefined;
    clear?: boolean | undefined;
    delay?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"screenshot">;
    fullPage: z.ZodOptional<z.ZodBoolean>;
    selector: z.ZodOptional<z.ZodString>;
    format: z.ZodOptional<z.ZodEnum<["png", "jpeg", "webp"]>>;
    quality: z.ZodOptional<z.ZodNumber>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "screenshot";
    quality?: number | undefined;
    timeout?: number | undefined;
    format?: "png" | "jpeg" | "webp" | undefined;
    selector?: string | undefined;
    fullPage?: boolean | undefined;
}, {
    action: "screenshot";
    quality?: number | undefined;
    timeout?: number | undefined;
    format?: "png" | "jpeg" | "webp" | undefined;
    selector?: string | undefined;
    fullPage?: boolean | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"evaluate">;
    code: z.ZodString;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    code: string;
    action: "evaluate";
    timeout?: number | undefined;
}, {
    code: string;
    action: "evaluate";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"waitForSelector">;
    selector: z.ZodString;
    state: z.ZodOptional<z.ZodEnum<["visible", "hidden", "attached", "detached"]>>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    selector: string;
    action: "waitForSelector";
    timeout?: number | undefined;
    state?: "visible" | "hidden" | "attached" | "detached" | undefined;
}, {
    selector: string;
    action: "waitForSelector";
    timeout?: number | undefined;
    state?: "visible" | "hidden" | "attached" | "detached" | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"scroll">;
    direction: z.ZodOptional<z.ZodEnum<["up", "down", "top", "bottom"]>>;
    amount: z.ZodOptional<z.ZodNumber>;
    selector: z.ZodOptional<z.ZodString>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "scroll";
    timeout?: number | undefined;
    selector?: string | undefined;
    direction?: "up" | "down" | "top" | "bottom" | undefined;
    amount?: number | undefined;
}, {
    action: "scroll";
    timeout?: number | undefined;
    selector?: string | undefined;
    direction?: "up" | "down" | "top" | "bottom" | undefined;
    amount?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"extract">;
    type: z.ZodEnum<["text", "html", "attribute", "list"]>;
    selector: z.ZodString;
    attribute: z.ZodOptional<z.ZodString>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    type: "html" | "text" | "attribute" | "list";
    selector: string;
    action: "extract";
    timeout?: number | undefined;
    attribute?: string | undefined;
}, {
    type: "html" | "text" | "attribute" | "list";
    selector: string;
    action: "extract";
    timeout?: number | undefined;
    attribute?: string | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"goBack">;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "goBack";
    timeout?: number | undefined;
}, {
    action: "goBack";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"goForward">;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "goForward";
    timeout?: number | undefined;
}, {
    action: "goForward";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"reload">;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "reload";
    timeout?: number | undefined;
}, {
    action: "reload";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"getContent">;
    type: z.ZodOptional<z.ZodEnum<["html", "text"]>>;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "getContent";
    timeout?: number | undefined;
    type?: "html" | "text" | undefined;
}, {
    action: "getContent";
    timeout?: number | undefined;
    type?: "html" | "text" | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"getUrl">;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "getUrl";
    timeout?: number | undefined;
}, {
    action: "getUrl";
    timeout?: number | undefined;
}>, z.ZodObject<{
    action: z.ZodLiteral<"getTitle">;
    timeout: z.ZodOptional<z.ZodNumber>;
}, "strip", z.ZodTypeAny, {
    action: "getTitle";
    timeout?: number | undefined;
}, {
    action: "getTitle";
    timeout?: number | undefined;
}>]>;
export type BrowserAction = z.infer<typeof browserActionSchema>;
/**
 * Firecrawl-compatible execute request.
 * Firecrawl uses code + language; we support both code and actions.
 */
export declare const browserExecuteRequestSchema: z.ZodEffects<z.ZodObject<{
    /** Session ID */
    browserId: z.ZodString;
    /** Code to execute (Firecrawl compatibility - converted to evaluate action) */
    code: z.ZodOptional<z.ZodString>;
    /** Language for code execution (Firecrawl compatibility) */
    language: z.ZodDefault<z.ZodOptional<z.ZodEnum<["python", "js"]>>>;
    /** Actions to execute (appzcrawl extension) */
    actions: z.ZodOptional<z.ZodArray<z.ZodDiscriminatedUnion<"action", [z.ZodObject<{
        action: z.ZodLiteral<"navigate">;
        url: z.ZodString;
        waitUntil: z.ZodOptional<z.ZodEnum<["load", "domcontentloaded", "networkidle"]>>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    }, {
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"click">;
        selector: z.ZodString;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    }, {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"type">;
        selector: z.ZodString;
        text: z.ZodString;
        delay: z.ZodOptional<z.ZodNumber>;
        clear: z.ZodOptional<z.ZodBoolean>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    }, {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"screenshot">;
        fullPage: z.ZodOptional<z.ZodBoolean>;
        selector: z.ZodOptional<z.ZodString>;
        format: z.ZodOptional<z.ZodEnum<["png", "jpeg", "webp"]>>;
        quality: z.ZodOptional<z.ZodNumber>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    }, {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"evaluate">;
        code: z.ZodString;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    }, {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"waitForSelector">;
        selector: z.ZodString;
        state: z.ZodOptional<z.ZodEnum<["visible", "hidden", "attached", "detached"]>>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    }, {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"scroll">;
        direction: z.ZodOptional<z.ZodEnum<["up", "down", "top", "bottom"]>>;
        amount: z.ZodOptional<z.ZodNumber>;
        selector: z.ZodOptional<z.ZodString>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    }, {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"extract">;
        type: z.ZodEnum<["text", "html", "attribute", "list"]>;
        selector: z.ZodString;
        attribute: z.ZodOptional<z.ZodString>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    }, {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"goBack">;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "goBack";
        timeout?: number | undefined;
    }, {
        action: "goBack";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"goForward">;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "goForward";
        timeout?: number | undefined;
    }, {
        action: "goForward";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"reload">;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "reload";
        timeout?: number | undefined;
    }, {
        action: "reload";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"getContent">;
        type: z.ZodOptional<z.ZodEnum<["html", "text"]>>;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    }, {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"getUrl">;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "getUrl";
        timeout?: number | undefined;
    }, {
        action: "getUrl";
        timeout?: number | undefined;
    }>, z.ZodObject<{
        action: z.ZodLiteral<"getTitle">;
        timeout: z.ZodOptional<z.ZodNumber>;
    }, "strip", z.ZodTypeAny, {
        action: "getTitle";
        timeout?: number | undefined;
    }, {
        action: "getTitle";
        timeout?: number | undefined;
    }>]>, "many">>;
}, "strip", z.ZodTypeAny, {
    language: "js" | "python";
    browserId: string;
    actions?: ({
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    } | {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    } | {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    } | {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    } | {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    } | {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    } | {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    } | {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    } | {
        action: "goBack";
        timeout?: number | undefined;
    } | {
        action: "goForward";
        timeout?: number | undefined;
    } | {
        action: "reload";
        timeout?: number | undefined;
    } | {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    } | {
        action: "getUrl";
        timeout?: number | undefined;
    } | {
        action: "getTitle";
        timeout?: number | undefined;
    })[] | undefined;
    code?: string | undefined;
}, {
    browserId: string;
    actions?: ({
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    } | {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    } | {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    } | {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    } | {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    } | {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    } | {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    } | {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    } | {
        action: "goBack";
        timeout?: number | undefined;
    } | {
        action: "goForward";
        timeout?: number | undefined;
    } | {
        action: "reload";
        timeout?: number | undefined;
    } | {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    } | {
        action: "getUrl";
        timeout?: number | undefined;
    } | {
        action: "getTitle";
        timeout?: number | undefined;
    })[] | undefined;
    language?: "js" | "python" | undefined;
    code?: string | undefined;
}>, {
    language: "js" | "python";
    browserId: string;
    actions?: ({
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    } | {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    } | {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    } | {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    } | {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    } | {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    } | {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    } | {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    } | {
        action: "goBack";
        timeout?: number | undefined;
    } | {
        action: "goForward";
        timeout?: number | undefined;
    } | {
        action: "reload";
        timeout?: number | undefined;
    } | {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    } | {
        action: "getUrl";
        timeout?: number | undefined;
    } | {
        action: "getTitle";
        timeout?: number | undefined;
    })[] | undefined;
    code?: string | undefined;
}, {
    browserId: string;
    actions?: ({
        url: string;
        action: "navigate";
        timeout?: number | undefined;
        waitUntil?: "load" | "domcontentloaded" | "networkidle" | undefined;
    } | {
        selector: string;
        action: "click";
        timeout?: number | undefined;
    } | {
        text: string;
        selector: string;
        action: "type";
        timeout?: number | undefined;
        clear?: boolean | undefined;
        delay?: number | undefined;
    } | {
        action: "screenshot";
        quality?: number | undefined;
        timeout?: number | undefined;
        format?: "png" | "jpeg" | "webp" | undefined;
        selector?: string | undefined;
        fullPage?: boolean | undefined;
    } | {
        code: string;
        action: "evaluate";
        timeout?: number | undefined;
    } | {
        selector: string;
        action: "waitForSelector";
        timeout?: number | undefined;
        state?: "visible" | "hidden" | "attached" | "detached" | undefined;
    } | {
        action: "scroll";
        timeout?: number | undefined;
        selector?: string | undefined;
        direction?: "up" | "down" | "top" | "bottom" | undefined;
        amount?: number | undefined;
    } | {
        type: "html" | "text" | "attribute" | "list";
        selector: string;
        action: "extract";
        timeout?: number | undefined;
        attribute?: string | undefined;
    } | {
        action: "goBack";
        timeout?: number | undefined;
    } | {
        action: "goForward";
        timeout?: number | undefined;
    } | {
        action: "reload";
        timeout?: number | undefined;
    } | {
        action: "getContent";
        timeout?: number | undefined;
        type?: "html" | "text" | undefined;
    } | {
        action: "getUrl";
        timeout?: number | undefined;
    } | {
        action: "getTitle";
        timeout?: number | undefined;
    })[] | undefined;
    language?: "js" | "python" | undefined;
    code?: string | undefined;
}>;
export type BrowserExecuteRequest = z.infer<typeof browserExecuteRequestSchema>;
export type BrowserExecuteRequestInput = z.input<typeof browserExecuteRequestSchema>;
export interface ActionResultData {
    success: boolean;
    data?: unknown;
    error?: string;
    screenshot?: string;
    url?: string;
    title?: string;
    executionTime?: number;
}
export interface BrowserExecuteResponseSuccess {
    success: true;
    /** Result string (for code execution - Firecrawl compatibility) */
    result?: string;
    /** Action results (appzcrawl extension) */
    results?: ActionResultData[];
    /** Current URL */
    url?: string;
    /** Current page title */
    title?: string;
}
export interface BrowserExecuteResponseError {
    success: false;
    error: string;
    /** Partial results if some actions succeeded */
    results?: ActionResultData[];
    /** Index where execution stopped */
    stoppedAtIndex?: number;
}
export type BrowserExecuteResponse = BrowserExecuteResponseSuccess | BrowserExecuteResponseError;
export declare const browserDeleteRequestSchema: z.ZodObject<{
    /** Session ID to delete */
    browserId: z.ZodString;
}, "strip", z.ZodTypeAny, {
    browserId: string;
}, {
    browserId: string;
}>;
export type BrowserDeleteRequest = z.infer<typeof browserDeleteRequestSchema>;
export type BrowserDeleteRequestInput = z.input<typeof browserDeleteRequestSchema>;
export interface BrowserDeleteResponseSuccess {
    success: true;
}
export interface BrowserDeleteResponseError {
    success: false;
    error: string;
}
export type BrowserDeleteResponse = BrowserDeleteResponseSuccess | BrowserDeleteResponseError;
export interface BrowserStatusResponseSuccess {
    success: true;
    browserId: string;
    status: "active" | "expired" | "destroyed";
    createdAt: string;
    lastActivity: string;
    ttlTotal: number;
    ttlRemaining: number;
    streamWebView: boolean;
}
export interface BrowserStatusResponseError {
    success: false;
    error: string;
}
export type BrowserStatusResponse = BrowserStatusResponseSuccess | BrowserStatusResponseError;
export declare function parseBrowserCreateRequest(raw: unknown): {
    ok: true;
    data: BrowserCreateRequest;
} | {
    ok: false;
    error: string;
};
export declare function parseBrowserExecuteRequest(raw: unknown): {
    ok: true;
    data: BrowserExecuteRequest;
} | {
    ok: false;
    error: string;
};
export declare function parseBrowserDeleteRequest(raw: unknown): {
    ok: true;
    data: BrowserDeleteRequest;
} | {
    ok: false;
    error: string;
};
export {};
//# sourceMappingURL=browser.d.ts.map