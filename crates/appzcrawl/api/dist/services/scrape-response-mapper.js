/**
 * Maps scrape runner output to Firecrawl-compatible response shape.
 */
import { wantsAnyAssets, } from "../contracts/scrape";
const UNSUPPORTED_WARNINGS = {
    changeTracking: "changeTracking format is not yet supported",
};
function collectWarnings(formats, request) {
    const warnings = [];
    for (const f of formats) {
        const msg = UNSUPPORTED_WARNINGS[f];
        if (msg)
            warnings.push(msg);
    }
    if (request.actions && request.actions.length > 0) {
        warnings.push("browser actions are partially supported (fire-engine only)");
    }
    if (request.location) {
        warnings.push("location/proxy settings are not yet applied");
    }
    if (request.waitFor && request.waitFor > 0) {
        warnings.push("waitFor delay is not yet applied");
    }
    return warnings;
}
/**
 * Map runner document to Firecrawl-compatible response data.
 * Only includes fields for requested formats; adds warning for unsupported features.
 */
export function mapToFirecrawlResponse(document, request) {
    const formats = request.formats ?? ["markdown"];
    let warnings = collectWarnings(formats, request);
    if (formats.includes("json") &&
        !document.documentJson &&
        !warnings.some((w) => w.includes("json"))) {
        warnings = [
            ...warnings,
            "json format requires Workers AI (use pnpm dev:remote for local dev); optionally provide jsonOptions.prompt or jsonOptions.schema for structured extraction",
        ];
    }
    if (formats.includes("branding") && !document.branding) {
        warnings.push(document.brandingError
            ? `branding failed: ${document.brandingError}`
            : "branding requires BROWSER_SERVICE binding (appz-browser)");
    }
    if ((formats.includes("screenshot") ||
        formats.includes("screenshot@fullPage")) &&
        !document.screenshot) {
        warnings.push("screenshot requires BROWSER_SERVICE binding (appz-browser) and valid screenshotBaseUrl");
    }
    const warning = warnings.length > 0 ? warnings.join("; ") : null;
    const data = {
        metadata: {
            ...document.metadata,
            sourceURL: document.url,
            statusCode: document.statusCode,
        },
        warning,
    };
    if (formats.includes("markdown") && document.markdown) {
        data.markdown = document.markdown;
    }
    if (formats.includes("html")) {
        data.html = document.html;
    }
    if (formats.includes("rawHtml")) {
        data.rawHtml = document.rawHtml;
    }
    if (formats.includes("links")) {
        data.links = document.links;
    }
    if (formats.includes("images")) {
        data.images = document.images ?? [];
    }
    if (wantsAnyAssets(formats)) {
        data.assets = document.assets ?? [];
    }
    if ((formats.includes("screenshot") ||
        formats.includes("screenshot@fullPage")) &&
        document.screenshot !== undefined) {
        data.screenshot = document.screenshot;
    }
    // Screenshot: null when requested but not captured (e.g. no BROWSER_SERVICE binding)
    if ((formats.includes("screenshot") ||
        formats.includes("screenshot@fullPage")) &&
        data.screenshot === undefined) {
        data.screenshot = null;
    }
    if (formats.includes("changeTracking")) {
        data.changeTracking = null;
    }
    if (formats.includes("branding")) {
        data.branding = document.branding ?? null;
    }
    if (formats.includes("json")) {
        data.llm_extraction = document.documentJson ?? null;
    }
    const cachedAt = document.metadata.cachedAt ??
        new Date().toISOString();
    const cacheState = typeof document.metadata.cachedAt === "string" ? "hit" : "miss";
    return {
        success: true,
        data,
        cacheState,
        cachedAt,
        creditsUsed: 1,
        concurrencyLimited: false,
    };
}
//# sourceMappingURL=scrape-response-mapper.js.map