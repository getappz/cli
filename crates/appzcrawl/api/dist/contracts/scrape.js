/**
 * Firecrawl-compatible scrape API contracts.
 * Request/response types aligned to Firecrawl v2 scrape endpoint for drop-in replacement.
 */
/** Output formats supported by Firecrawl scrape API. */
export const SCRAPE_FORMATS = [
    "markdown",
    "html",
    "rawHtml",
    "links",
    "images",
    "assets",
    "css",
    "js",
    "fonts",
    "videos",
    "audio",
    "iframes",
    "screenshot",
    "screenshot@fullPage",
    "json",
    "changeTracking",
    "branding",
];
/** Asset format types that trigger extract-assets. "assets" = all, others = specific. */
export const ASSET_FORMAT_TYPES = [
    "assets",
    "images",
    "css",
    "js",
    "fonts",
    "videos",
    "audio",
    "iframes",
];
/** Formats that map to native extract-assets (excludes "assets" which means "all"). */
export const ASSET_TYPE_FORMATS = [
    "images",
    "css",
    "js",
    "fonts",
    "videos",
    "audio",
    "iframes",
];
/**
 * Returns true if formats requests any asset extraction.
 */
export function wantsAnyAssets(formats) {
    return formats.some((f) => ASSET_FORMAT_TYPES.includes(f));
}
/**
 * Returns the asset formats to pass to native extract-assets.
 * - ["assets"] when user requested "assets" (all types)
 * - Otherwise the list of specific asset types requested (e.g. ["css", "js"])
 */
export function getAssetFormatsToExtract(formats) {
    if (formats.includes("assets"))
        return ["assets"];
    return formats.filter((f) => ASSET_TYPE_FORMATS.includes(f));
}
/** Resolve screenshot options from request. */
export function resolveScreenshotOptions(formats, screenshotOptions) {
    const hasScreenshot = formats.includes("screenshot") || formats.includes("screenshot@fullPage");
    if (!hasScreenshot)
        return null;
    return {
        fullPage: screenshotOptions?.fullPage ?? formats.includes("screenshot@fullPage"),
        viewport: screenshotOptions?.viewport,
        quality: screenshotOptions?.quality,
    };
}
/** Default request values matching Firecrawl. */
export const SCRAPE_DEFAULTS = {
    formats: ["markdown"],
    onlyMainContent: true,
    includeTags: [],
    excludeTags: [],
    /** 2 days — enables cache by default so branding comes from cache (Firecrawl uses 4h–2d). */
    maxAge: 2 * 24 * 60 * 60 * 1000,
    waitFor: 0,
    mobile: false,
    skipTlsVerification: true,
    timeout: 30000,
    removeBase64Images: true,
    blockAds: true,
    proxy: "auto",
    storeInCache: true,
    zeroDataRetention: false,
};
/** Build response envelope for cache miss (e.g. crawl, extract, agent, search, map). */
export function responseEnvelope(creditsUsed = 1) {
    return {
        cacheState: "miss",
        cachedAt: new Date().toISOString(),
        creditsUsed,
        concurrencyLimited: false,
    };
}
/** Parse and normalize request body with Firecrawl defaults. Accepts unknown keys without failing. */
export function parseScrapeRequestBody(body) {
    if (body === null || typeof body !== "object") {
        return { ok: false, error: "Invalid JSON body; expected object" };
    }
    const raw = body;
    const url = raw.url;
    if (typeof url !== "string" || !url.trim()) {
        return { ok: false, error: "Missing or invalid url in body" };
    }
    const formats = raw.formats;
    const formatsArray = Array.isArray(formats)
        ? formats.filter((f) => typeof f === "string" && SCRAPE_FORMATS.includes(f))
        : SCRAPE_DEFAULTS.formats;
    return {
        ok: true,
        data: {
            url: url.trim(),
            formats: formatsArray.length > 0 ? formatsArray : SCRAPE_DEFAULTS.formats,
            onlyMainContent: typeof raw.onlyMainContent === "boolean"
                ? raw.onlyMainContent
                : SCRAPE_DEFAULTS.onlyMainContent,
            includeTags: Array.isArray(raw.includeTags)
                ? raw.includeTags.filter((t) => typeof t === "string")
                : SCRAPE_DEFAULTS.includeTags,
            excludeTags: Array.isArray(raw.excludeTags)
                ? raw.excludeTags.filter((t) => typeof t === "string")
                : SCRAPE_DEFAULTS.excludeTags,
            maxAge: typeof raw.maxAge === "number" && raw.maxAge >= 0
                ? raw.maxAge
                : SCRAPE_DEFAULTS.maxAge,
            headers: raw.headers &&
                typeof raw.headers === "object" &&
                !Array.isArray(raw.headers)
                ? raw.headers
                : undefined,
            waitFor: typeof raw.waitFor === "number" && raw.waitFor >= 0
                ? raw.waitFor
                : SCRAPE_DEFAULTS.waitFor,
            mobile: typeof raw.mobile === "boolean" ? raw.mobile : SCRAPE_DEFAULTS.mobile,
            skipTlsVerification: typeof raw.skipTlsVerification === "boolean"
                ? raw.skipTlsVerification
                : SCRAPE_DEFAULTS.skipTlsVerification,
            timeout: typeof raw.timeout === "number" && raw.timeout > 0
                ? Math.min(raw.timeout, 300_000)
                : SCRAPE_DEFAULTS.timeout,
            actions: Array.isArray(raw.actions) ? raw.actions : undefined,
            location: raw.location && typeof raw.location === "object"
                ? raw.location
                : undefined,
            removeBase64Images: typeof raw.removeBase64Images === "boolean"
                ? raw.removeBase64Images
                : SCRAPE_DEFAULTS.removeBase64Images,
            blockAds: typeof raw.blockAds === "boolean"
                ? raw.blockAds
                : SCRAPE_DEFAULTS.blockAds,
            proxy: raw.proxy === "basic" ||
                raw.proxy === "enhanced" ||
                raw.proxy === "auto"
                ? raw.proxy
                : SCRAPE_DEFAULTS.proxy,
            storeInCache: typeof raw.storeInCache === "boolean"
                ? raw.storeInCache
                : SCRAPE_DEFAULTS.storeInCache,
            zeroDataRetention: typeof raw.zeroDataRetention === "boolean"
                ? raw.zeroDataRetention
                : SCRAPE_DEFAULTS.zeroDataRetention,
            useFireEngine: Boolean(raw.useFireEngine),
            engine: raw.engine === "native" ||
                raw.engine === "cloudflare" ||
                raw.engine === "auto"
                ? raw.engine
                : undefined,
            citations: typeof raw.citations === "boolean" ? raw.citations : false,
            screenshotOptions: raw.screenshotOptions &&
                typeof raw.screenshotOptions === "object" &&
                !Array.isArray(raw.screenshotOptions)
                ? raw.screenshotOptions
                : undefined,
            jsonOptions: raw.jsonOptions &&
                typeof raw.jsonOptions === "object" &&
                !Array.isArray(raw.jsonOptions)
                ? (() => {
                    const jo = raw.jsonOptions;
                    return {
                        prompt: typeof jo.prompt === "string" ? jo.prompt : undefined,
                        schema: jo.schema && typeof jo.schema === "object"
                            ? jo.schema
                            : undefined,
                    };
                })()
                : undefined,
        },
    };
}
//# sourceMappingURL=scrape.js.map