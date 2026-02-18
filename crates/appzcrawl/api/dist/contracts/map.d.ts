/**
 * Firecrawl-compatible map API contracts.
 * Request/response aligned to Firecrawl v2 POST /map (OpenAPI).
 */
import { z } from "zod";
export declare const MAX_MAP_LIMIT = 100000;
/** Map request body schema (Firecrawl v2 compatible). */
export declare const mapRequestSchema: z.ZodObject<{
    url: z.ZodEffects<z.ZodString, string, string>;
    search: z.ZodOptional<z.ZodString>;
    sitemap: z.ZodDefault<z.ZodOptional<z.ZodEnum<["skip", "include", "only"]>>>;
    limit: z.ZodDefault<z.ZodOptional<z.ZodNumber>>;
    timeout: z.ZodOptional<z.ZodNumber>;
    includeSubdomains: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    allowSubdomains: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    allowExternalLinks: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    ignoreQueryParameters: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    filterByPath: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    includePaths: z.ZodDefault<z.ZodOptional<z.ZodArray<z.ZodString, "many">>>;
    excludePaths: z.ZodDefault<z.ZodOptional<z.ZodArray<z.ZodString, "many">>>;
    regexOnFullURL: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    ignoreCache: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    ignoreRobotsTxt: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    deduplicateSimilarURLs: z.ZodDefault<z.ZodOptional<z.ZodBoolean>>;
    location: z.ZodOptional<z.ZodOptional<z.ZodObject<{
        country: z.ZodOptional<z.ZodString>;
        languages: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    }, "strip", z.ZodTypeAny, {
        country?: string | undefined;
        languages?: string[] | undefined;
    }, {
        country?: string | undefined;
        languages?: string[] | undefined;
    }>>>;
    headers: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodString>>;
    engine: z.ZodOptional<z.ZodEnum<["native", "cloudflare", "auto"]>>;
}, "strip", z.ZodTypeAny, {
    url: string;
    includePaths: string[];
    excludePaths: string[];
    limit: number;
    allowExternalLinks: boolean;
    allowSubdomains: boolean;
    ignoreRobotsTxt: boolean;
    sitemap: "skip" | "include" | "only";
    deduplicateSimilarURLs: boolean;
    ignoreQueryParameters: boolean;
    regexOnFullURL: boolean;
    includeSubdomains: boolean;
    filterByPath: boolean;
    ignoreCache: boolean;
    timeout?: number | undefined;
    headers?: Record<string, string> | undefined;
    location?: {
        country?: string | undefined;
        languages?: string[] | undefined;
    } | undefined;
    engine?: "auto" | "native" | "cloudflare" | undefined;
    search?: string | undefined;
}, {
    url: string;
    timeout?: number | undefined;
    headers?: Record<string, string> | undefined;
    location?: {
        country?: string | undefined;
        languages?: string[] | undefined;
    } | undefined;
    engine?: "auto" | "native" | "cloudflare" | undefined;
    search?: string | undefined;
    includePaths?: string[] | undefined;
    excludePaths?: string[] | undefined;
    limit?: number | undefined;
    allowExternalLinks?: boolean | undefined;
    allowSubdomains?: boolean | undefined;
    ignoreRobotsTxt?: boolean | undefined;
    sitemap?: "skip" | "include" | "only" | undefined;
    deduplicateSimilarURLs?: boolean | undefined;
    ignoreQueryParameters?: boolean | undefined;
    regexOnFullURL?: boolean | undefined;
    includeSubdomains?: boolean | undefined;
    filterByPath?: boolean | undefined;
    ignoreCache?: boolean | undefined;
}>;
export type MapRequest = z.infer<typeof mapRequestSchema>;
export type MapRequestInput = z.input<typeof mapRequestSchema>;
/** Single link in map response (Firecrawl MapDocument). */
export interface MapDocument {
    url: string;
    title?: string;
    description?: string;
}
/** Successful map response (Firecrawl MapResponse). */
export interface MapResponseSuccess {
    success: true;
    links: MapDocument[];
    warning?: string;
}
/** Error map response. */
export interface MapResponseError {
    success: false;
    error: string;
    code?: string;
}
export type MapResponse = MapResponseSuccess | MapResponseError;
export declare function parseMapRequestBody(raw: unknown): {
    ok: true;
    data: MapRequest;
} | {
    ok: false;
    error: string;
};
//# sourceMappingURL=map.d.ts.map