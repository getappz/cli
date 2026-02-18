/**
 * Resolve raw link hrefs to absolute URLs (Firecrawl-compatible).
 * Uses baseUrl and optional baseHref from <base> for relative resolution.
 */
export declare function resolveUrlWithBaseHref(href: string, baseUrl: string, baseHref: string): string;
/** Resolve raw hrefs to absolute URLs and dedupe (Firecrawl links format). */
export declare function resolveLinks(rawHrefs: string[], baseUrl: string, baseHref: string): string[];
//# sourceMappingURL=resolveLinks.d.ts.map