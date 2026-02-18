/**
 * Transform raw branding script output into BrandingProfile.
 * Adapted from Firecrawl for Workers (heuristic + optional LLM).
 */
import type { BrandingProfile, BrandingScriptReturn } from "./types";
export interface BrandingTransformerInput {
    url: string;
    html?: string;
    rawBranding: BrandingScriptReturn;
    debugBranding?: boolean;
    /** Cloudflare Workers AI binding for LLM enhancement; when omitted, uses heuristics only */
    aiBinding?: {
        run: (model: string, options: object) => Promise<unknown>;
    };
}
/**
 * Transform raw branding script output into a BrandingProfile.
 * Uses heuristics for logo/button selection; LLM stub returns heuristic-only enhancement.
 */
export declare function brandingTransformer(input: BrandingTransformerInput): Promise<BrandingProfile>;
//# sourceMappingURL=transformer.d.ts.map