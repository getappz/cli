/**
 * LLM extraction: convert markdown/HTML to structured JSON using Cloudflare Workers AI.
 * Adapted from Firecrawl's llmExtract. Uses AI binding with JSON Mode when schema provided.
 */
export interface LlmExtractOptions {
    /** User prompt directing extraction (e.g. "Extract invoice fields"). */
    prompt?: string;
    /** JSON Schema for structured output. When provided, uses Workers AI JSON Mode. */
    schema?: Record<string, unknown>;
}
type AiBinding = {
    run: (model: string, options: Record<string, unknown>) => Promise<{
        response?: string;
        [key: string]: unknown;
    }>;
};
/**
 * Extract structured JSON from markdown using Cloudflare Workers AI.
 * When schema is provided, uses response_format for JSON Mode (structured output).
 */
export declare function extractWithLlm(ai: AiBinding, markdown: string, options?: LlmExtractOptions): Promise<{
    data: unknown;
    warning?: string;
}>;
export {};
//# sourceMappingURL=llm-extract.d.ts.map