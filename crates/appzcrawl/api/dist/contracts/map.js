/**
 * Firecrawl-compatible map API contracts.
 * Request/response aligned to Firecrawl v2 POST /map (OpenAPI).
 */
import { z } from "zod";
const URL = z
    .string()
    .url()
    .refine((x) => /^https?:\/\//i.test(x), "URL must use http or https protocol");
/** ISO 3166-1 alpha-2 pattern; location is optional and may be ignored by implementation. */
const locationSchema = z
    .object({
    country: z
        .string()
        .regex(/^[A-Z]{2}$/i)
        .optional(),
    languages: z.array(z.string()).optional(),
})
    .optional();
export const MAX_MAP_LIMIT = 100_000;
/** Map request body schema (Firecrawl v2 compatible). */
export const mapRequestSchema = z.object({
    // Core params
    url: URL,
    search: z.string().optional(),
    sitemap: z.enum(["skip", "include", "only"]).optional().default("include"),
    limit: z.number().int().min(1).max(MAX_MAP_LIMIT).optional().default(5000),
    timeout: z.number().int().positive().finite().optional(),
    // URL filtering
    includeSubdomains: z.boolean().optional().default(true),
    allowSubdomains: z.boolean().optional().default(false),
    allowExternalLinks: z.boolean().optional().default(false),
    ignoreQueryParameters: z.boolean().optional().default(true),
    filterByPath: z.boolean().optional().default(true),
    // Path patterns (regex or glob)
    includePaths: z.array(z.string()).optional().default([]),
    excludePaths: z.array(z.string()).optional().default([]),
    regexOnFullURL: z.boolean().optional().default(false),
    // Cache and robots
    ignoreCache: z.boolean().optional().default(false),
    ignoreRobotsTxt: z.boolean().optional().default(false),
    // Deduplication
    deduplicateSimilarURLs: z.boolean().optional().default(true),
    // Request customization
    location: locationSchema.optional(),
    headers: z.record(z.string(), z.string()).optional(),
    // Fetch engine for web URLs: native (default), cloudflare, or auto
    engine: z.enum(["native", "cloudflare", "auto"]).optional(),
});
export function parseMapRequestBody(raw) {
    const parsed = mapRequestSchema.safeParse(raw);
    if (!parsed.success) {
        const first = parsed.error.flatten().formErrors[0] ?? parsed.error.message;
        return { ok: false, error: first };
    }
    return { ok: true, data: parsed.data };
}
//# sourceMappingURL=map.js.map