/**
 * Resolve raw link hrefs to absolute URLs (Firecrawl-compatible).
 * Uses baseUrl and optional baseHref from <base> for relative resolution.
 */
export function resolveUrlWithBaseHref(href, baseUrl, baseHref) {
    let resolutionBase = baseUrl;
    if (baseHref) {
        try {
            new URL(baseHref);
            resolutionBase = baseHref;
        }
        catch {
            try {
                resolutionBase = new URL(baseHref, baseUrl).href;
            }
            catch {
                resolutionBase = baseUrl;
            }
        }
    }
    try {
        if (href.startsWith("http://") || href.startsWith("https://")) {
            return href;
        }
        if (href.startsWith("mailto:")) {
            return href;
        }
        if (href.startsWith("#")) {
            return "";
        }
        return new URL(href, resolutionBase).href;
    }
    catch {
        return "";
    }
}
/** Resolve raw hrefs to absolute URLs and dedupe (Firecrawl links format). */
export function resolveLinks(rawHrefs, baseUrl, baseHref) {
    const seen = new Set();
    const out = [];
    for (const href of rawHrefs) {
        const trimmed = href.trim();
        if (!trimmed)
            continue;
        const resolved = resolveUrlWithBaseHref(trimmed, baseUrl, baseHref);
        if (resolved && !seen.has(resolved)) {
            seen.add(resolved);
            out.push(resolved);
        }
    }
    return out;
}
//# sourceMappingURL=resolveLinks.js.map