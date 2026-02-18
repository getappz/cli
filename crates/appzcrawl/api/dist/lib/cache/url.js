/**
 * URL normalization and hashing for scrape cache.
 * Adopts Firecrawl rules for canonical URLs so variants map to the same cache key.
 */
/** Normalize URL for cache lookup. Variants like www, trailing slash, index.html map to same result. */
export function normalizeUrlForCache(url) {
    const urlObj = new URL(url);
    if (!urlObj.hash ||
        urlObj.hash.length <= 2 ||
        (!urlObj.hash.startsWith("#/") && !urlObj.hash.startsWith("#!/"))) {
        urlObj.hash = "";
    }
    urlObj.protocol = "https";
    if (urlObj.port === "80" || urlObj.port === "443") {
        urlObj.port = "";
    }
    if (urlObj.hostname.startsWith("www.")) {
        urlObj.hostname = urlObj.hostname.slice(4);
    }
    if (urlObj.pathname.endsWith("/index.html")) {
        urlObj.pathname = urlObj.pathname.slice(0, -10);
    }
    else if (urlObj.pathname.endsWith("/index.php")) {
        urlObj.pathname = urlObj.pathname.slice(0, -9);
    }
    else if (urlObj.pathname.endsWith("/index.htm")) {
        urlObj.pathname = urlObj.pathname.slice(0, -9);
    }
    else if (urlObj.pathname.endsWith("/index.shtml")) {
        urlObj.pathname = urlObj.pathname.slice(0, -11);
    }
    else if (urlObj.pathname.endsWith("/index.xml")) {
        urlObj.pathname = urlObj.pathname.slice(0, -9);
    }
    if (urlObj.pathname.endsWith("/")) {
        urlObj.pathname = urlObj.pathname.slice(0, -1);
    }
    return urlObj.toString();
}
/** SHA-256 hash of string using Web Crypto. Returns hex string. */
export async function hashString(value) {
    const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
    return Array.from(new Uint8Array(buf))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
}
/** Hash normalized URL; returns full 32-char hex. */
export async function hashUrl(url) {
    return hashString(url);
}
//# sourceMappingURL=url.js.map