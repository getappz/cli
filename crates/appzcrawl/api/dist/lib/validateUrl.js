export const protocolIncluded = (url) => /^([^.:]+:\/\/)/i.test(url);
const getURLobj = (s) => {
    let error = false;
    let urlObj = {};
    try {
        urlObj = new URL(s);
    }
    catch {
        error = true;
    }
    return { error, urlObj: urlObj };
};
export function checkUrl(url) {
    const { error, urlObj } = getURLobj(url);
    if (error)
        throw new Error("Invalid URL");
    if (urlObj.protocol !== "http:" && urlObj.protocol !== "https:") {
        throw new Error("Invalid URL");
    }
}
/**
 * Same domain check (base domain, allows subdomains).
 * Adopted from Firecrawl validateUrl for map endpoint.
 */
export function isSameDomain(url, baseUrl) {
    const { error: error1, urlObj: urlObj1 } = getURLobj(url);
    const { error: error2, urlObj: urlObj2 } = getURLobj(baseUrl);
    if (error1 || error2)
        return false;
    const cleanHostname = (hostname) => hostname.startsWith("www.") ? hostname.slice(4) : hostname;
    const domain1 = cleanHostname(urlObj1.hostname)
        .split(".")
        .slice(-2)
        .join(".");
    const domain2 = cleanHostname(urlObj2.hostname)
        .split(".")
        .slice(-2)
        .join(".");
    return domain1 === domain2;
}
/**
 * Same subdomain check (exact hostname after stripping www).
 * Adopted from Firecrawl validateUrl for map endpoint.
 */
export function isSameSubdomain(url, baseUrl) {
    const { error: error1, urlObj: urlObj1 } = getURLobj(url);
    const { error: error2, urlObj: urlObj2 } = getURLobj(baseUrl);
    if (error1 || error2)
        return false;
    const cleanHostname = (hostname) => hostname.startsWith("www.") ? hostname.slice(4) : hostname;
    const domain1 = cleanHostname(urlObj1.hostname)
        .split(".")
        .slice(-2)
        .join(".");
    const domain2 = cleanHostname(urlObj2.hostname)
        .split(".")
        .slice(-2)
        .join(".");
    const subdomain1 = cleanHostname(urlObj1.hostname)
        .split(".")
        .slice(0, -2)
        .join(".");
    const subdomain2 = cleanHostname(urlObj2.hostname)
        .split(".")
        .slice(0, -2)
        .join(".");
    return domain1 === domain2 && subdomain1 === subdomain2;
}
/**
 * Normalize URL for map: add protocol, strip trailing slash, optionally strip query.
 * Adopted from Firecrawl checkAndUpdateURLForMap.
 */
export function checkAndUpdateURLForMap(url, ignoreQueryParameters = false) {
    if (!protocolIncluded(url))
        url = `http://${url}`;
    if (url.endsWith("/"))
        url = url.slice(0, -1);
    const { error, urlObj } = getURLobj(url);
    if (error)
        throw new Error("Invalid URL");
    if (urlObj.protocol !== "http:" && urlObj.protocol !== "https:") {
        throw new Error("Invalid URL");
    }
    if (ignoreQueryParameters)
        url = url.split("?")[0].trim();
    return { urlObj, url };
}
/**
 * Deduplicate URLs (normalize www/protocol); prefer https and non-www.
 * Adopted from Firecrawl removeDuplicateUrls.
 */
export function removeDuplicateUrls(urls) {
    const urlMap = new Map();
    for (const url of urls) {
        try {
            const parsedUrl = new URL(url);
            const protocol = parsedUrl.protocol;
            const hostname = parsedUrl.hostname.replace(/^www\./, "");
            const path = parsedUrl.pathname + parsedUrl.search + parsedUrl.hash;
            const key = `${hostname}${path}`;
            const existing = urlMap.get(key);
            if (!existing) {
                urlMap.set(key, url);
            }
            else {
                const existingUrl = new URL(existing);
                if (protocol === "https:" && existingUrl.protocol === "http:") {
                    urlMap.set(key, url);
                }
                else if (protocol === existingUrl.protocol &&
                    !parsedUrl.hostname.startsWith("www.") &&
                    existingUrl.hostname.startsWith("www.")) {
                    urlMap.set(key, url);
                }
            }
        }
        catch {
            // skip invalid URLs
        }
    }
    return [...urlMap.values()];
}
//# sourceMappingURL=validateUrl.js.map