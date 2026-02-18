/**
 * Cloudflare Browser Rendering API client.
 *
 * Provides a CloudflareBrowserEngine class with methods for:
 * - content: Fetch rendered HTML
 * - snapshot: HTML + base64 screenshot
 * - markdown: Extract page as Markdown
 * - scrape: Extract elements by CSS selectors (text, html, attributes, dimensions)
 * - json: AI-powered structured extraction (prompt + optional JSON schema)
 *
 * API: https://developers.cloudflare.com/browser-rendering/
 *      https://developers.cloudflare.com/api/resources/browser_rendering/
 */
const CLOUDFLARE_API_BASE = "https://api.cloudflare.com/client/v4/accounts";
// ---------------------------------------------------------------------------
// CloudflareBrowserEngine class
// ---------------------------------------------------------------------------
function buildGotoOptions(options) {
    return {
        waitUntil: options.waitUntil ?? "networkidle0",
        timeout: options.timeout ?? 30_000,
    };
}
function parseCloudflareResponse(res, data, resultValidator) {
    if (!res.ok) {
        const errMsg = (Array.isArray(data.errors) &&
            data.errors.length > 0 &&
            data.errors[0].message) ||
            "Cloudflare Browser Rendering request failed";
        return {
            success: false,
            error: typeof errMsg === "string" ? errMsg : String(data.errors),
        };
    }
    if (data.success === false) {
        const errMsg = (Array.isArray(data.errors) &&
            data.errors.length > 0 &&
            data.errors[0].message) ||
            "Cloudflare returned success: false";
        return {
            success: false,
            error: typeof errMsg === "string" ? errMsg : "Unknown error",
        };
    }
    const result = data.result;
    const validated = resultValidator(result);
    if (validated === null) {
        return {
            success: false,
            error: "Cloudflare returned invalid result shape",
        };
    }
    return { success: true, result: validated };
}
async function cfRequest(accountId, apiToken, endpoint, body) {
    let res;
    try {
        res = await fetch(`${CLOUDFLARE_API_BASE}/${accountId}/browser-rendering/${endpoint}`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                Authorization: `Bearer ${apiToken}`,
            },
            body: JSON.stringify(body),
        });
    }
    catch (e) {
        return {
            ok: false,
            error: e instanceof Error ? e.message : "Cloudflare request failed",
        };
    }
    let data;
    try {
        data = (await res.json());
    }
    catch {
        return {
            ok: false,
            error: "Invalid JSON from Cloudflare Browser Rendering",
        };
    }
    return { ok: true, data, res };
}
/**
 * Cloudflare Browser Rendering Engine.
 * Use accountId and apiToken (from env CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_BROWSER_RENDERING_API_TOKEN).
 */
export class CloudflareBrowserEngine {
    accountId;
    apiToken;
    constructor(accountId, apiToken) {
        this.accountId = accountId;
        this.apiToken = apiToken;
    }
    /**
     * Fetch rendered HTML from /content.
     * https://developers.cloudflare.com/browser-rendering/rest-api/content-endpoint/
     */
    async content(url, options = {}) {
        const body = {
            url,
            gotoOptions: buildGotoOptions(options),
        };
        if (options.viewport)
            body.viewport = options.viewport;
        const req = await cfRequest(this.accountId, this.apiToken, "content", body);
        if (!req.ok)
            return { success: false, error: req.error };
        const parsed = parseCloudflareResponse(req.res, req.data, (r) => typeof r === "string" ? r : null);
        if (!parsed.success)
            return parsed;
        return {
            success: true,
            html: parsed.result,
            statusCode: 200,
        };
    }
    /**
     * Fetch HTML + base64 screenshot from /snapshot.
     * https://developers.cloudflare.com/browser-rendering/rest-api/snapshot/
     */
    async snapshot(url, options = {}) {
        const body = {
            url,
            gotoOptions: buildGotoOptions(options),
            screenshotOptions: {
                fullPage: options.fullPage ?? false,
                ...(options.screenshotViewport && {
                    viewport: options.screenshotViewport,
                }),
            },
        };
        if (options.viewport)
            body.viewport = options.viewport;
        const req = await cfRequest(this.accountId, this.apiToken, "snapshot", body);
        if (!req.ok)
            return { success: false, error: req.error };
        const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
            if (r &&
                typeof r === "object" &&
                "content" in r &&
                "screenshot" in r &&
                typeof r.content === "string" &&
                typeof r.screenshot === "string") {
                return r;
            }
            return null;
        });
        if (!parsed.success)
            return parsed;
        return {
            success: true,
            html: parsed.result.content,
            statusCode: 200,
            screenshotBase64: parsed.result.screenshot,
        };
    }
    /**
     * Extract page as Markdown from /markdown.
     * https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/
     */
    async markdown(url, options = {}) {
        const body = {
            url,
            gotoOptions: buildGotoOptions(options),
        };
        if (options.viewport)
            body.viewport = options.viewport;
        if (options.rejectRequestPattern?.length)
            body.rejectRequestPattern = options.rejectRequestPattern;
        const req = await cfRequest(this.accountId, this.apiToken, "markdown", body);
        if (!req.ok)
            return { success: false, error: req.error };
        const parsed = parseCloudflareResponse(req.res, req.data, (r) => typeof r === "string" ? r : null);
        if (!parsed.success)
            return parsed;
        return {
            success: true,
            markdown: parsed.result,
            statusCode: 200,
        };
    }
    /**
     * Scrape elements by CSS selectors from /scrape.
     * Returns text, html, attributes, dimensions per element.
     * https://developers.cloudflare.com/browser-rendering/rest-api/scrape-endpoint/
     */
    async scrape(url, options) {
        const { elements, ...pageOpts } = options;
        if (!elements?.length) {
            return {
                success: false,
                error: "scrape requires at least one element with selector",
            };
        }
        const body = {
            url,
            elements: elements.map((e) => ({ selector: e.selector })),
            gotoOptions: buildGotoOptions(pageOpts),
        };
        if (pageOpts.viewport)
            body.viewport = pageOpts.viewport;
        const req = await cfRequest(this.accountId, this.apiToken, "scrape", body);
        if (!req.ok)
            return { success: false, error: req.error };
        const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
            if (Array.isArray(r)) {
                return r;
            }
            return null;
        });
        if (!parsed.success)
            return parsed;
        return {
            success: true,
            data: parsed.result,
            statusCode: 200,
        };
    }
    /**
     * AI-powered structured JSON extraction from /json.
     * Requires prompt and/or responseFormat. Uses Workers AI by default; use customAi for BYO model.
     * https://developers.cloudflare.com/browser-rendering/rest-api/json-endpoint/
     */
    async json(url, options) {
        const { prompt, responseFormat, customAi, ...pageOpts } = options;
        if (!prompt && !responseFormat) {
            return {
                success: false,
                error: "json requires prompt and/or responseFormat",
            };
        }
        const body = {
            url,
            gotoOptions: buildGotoOptions(pageOpts),
        };
        if (pageOpts.viewport)
            body.viewport = pageOpts.viewport;
        if (prompt)
            body.prompt = prompt;
        if (responseFormat)
            body.response_format = {
                type: "json_schema",
                schema: responseFormat.schema,
            };
        if (customAi?.length)
            body.custom_ai = customAi;
        const req = await cfRequest(this.accountId, this.apiToken, "json", body);
        if (!req.ok)
            return { success: false, error: req.error };
        const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
            if (r && typeof r === "object" && !Array.isArray(r)) {
                return r;
            }
            return null;
        });
        if (!parsed.success)
            return parsed;
        return {
            success: true,
            data: parsed.result,
            statusCode: 200,
        };
    }
}
// ---------------------------------------------------------------------------
// Standalone functions (backward compatibility)
// ---------------------------------------------------------------------------
export async function cloudflareFetchContent(accountId, apiToken, url, options = {}) {
    const engine = new CloudflareBrowserEngine(accountId, apiToken);
    return engine.content(url, options);
}
export async function cloudflareFetchSnapshot(accountId, apiToken, url, options = {}) {
    const engine = new CloudflareBrowserEngine(accountId, apiToken);
    return engine.snapshot(url, options);
}
export function isCloudflareBrowserEnabled(env) {
    const accountId = env.CLOUDFLARE_ACCOUNT_ID;
    const token = env.CLOUDFLARE_BROWSER_RENDERING_API_TOKEN;
    return (typeof accountId === "string" &&
        accountId.trim().length > 0 &&
        typeof token === "string" &&
        token.trim().length > 0);
}
/**
 * Create a CloudflareBrowserEngine from env bindings.
 * Returns null if credentials are not configured.
 */
export function createCloudflareBrowserEngine(env) {
    if (!isCloudflareBrowserEnabled(env))
        return null;
    return new CloudflareBrowserEngine(env.CLOUDFLARE_ACCOUNT_ID, env.CLOUDFLARE_BROWSER_RENDERING_API_TOKEN);
}
//# sourceMappingURL=cloudflare-browser-client.js.map