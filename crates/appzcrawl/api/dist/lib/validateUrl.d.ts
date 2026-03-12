export declare const protocolIncluded: (url: string) => boolean;
export declare function checkUrl(url: string): void;
/**
 * Same domain check (base domain, allows subdomains).
 * Adopted from Firecrawl validateUrl for map endpoint.
 */
export declare function isSameDomain(url: string, baseUrl: string): boolean;
/**
 * Same subdomain check (exact hostname after stripping www).
 * Adopted from Firecrawl validateUrl for map endpoint.
 */
export declare function isSameSubdomain(url: string, baseUrl: string): boolean;
/**
 * Normalize URL for map: add protocol, strip trailing slash, optionally strip query.
 * Adopted from Firecrawl checkAndUpdateURLForMap.
 */
export declare function checkAndUpdateURLForMap(url: string, ignoreQueryParameters?: boolean): {
    url: string;
    urlObj: URL;
};
/**
 * Deduplicate URLs (normalize www/protocol); prefer https and non-www.
 * Adopted from Firecrawl removeDuplicateUrls.
 */
export declare function removeDuplicateUrls(urls: string[]): string[];
//# sourceMappingURL=validateUrl.d.ts.map