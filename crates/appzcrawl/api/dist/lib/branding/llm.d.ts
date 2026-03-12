/**
 * Branding LLM enhancement using Cloudflare Workers AI (env.AI).
 * Uses the same prompt as Firecrawl; falls back to heuristics when AI is unavailable or fails.
 */
import { type BrandingEnhancement } from "./schema";
import type { BrandingLLMInput } from "./types";
/** Cloudflare Workers AI binding - run(modelId, options) returns { response?: unknown }. */
type AiBinding = {
    run: (model: string, options: {
        messages?: Array<{
            role: string;
            content: string;
        }>;
        response_format?: {
            type: string;
            json_schema: object;
        };
        max_tokens?: number;
    }) => Promise<unknown>;
};
/**
 * Enhance branding using Cloudflare Workers AI.
 * When aiBinding is undefined, or when the AI call fails, falls back to heuristics.
 */
export declare function enhanceBrandingWithLLM(input: BrandingLLMInput, aiBinding?: AiBinding): Promise<BrandingEnhancement>;
export {};
//# sourceMappingURL=llm.d.ts.map