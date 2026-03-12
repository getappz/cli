/**
 * LLM extraction: convert markdown/HTML to structured JSON using Cloudflare Workers AI.
 * Adapted from Firecrawl's llmExtract. Uses AI binding with JSON Mode when schema provided.
 */
import { logger } from "../lib/logger";
/** @cf/meta/llama-3.1-8b-instruct-fast supports JSON Mode. 128k context. */
const DEFAULT_MODEL = "@cf/meta/llama-3.1-8b-instruct-fast";
/** Strip markdown code fences (```json ... ``` or ``` ... ```) before JSON.parse. */
function stripJsonFences(raw) {
    const trimmed = raw.trim();
    const fenceMatch = trimmed.match(/^```(?:json)?\s*\n?([\s\S]*?)\n?```$/);
    return fenceMatch ? fenceMatch[1].trim() : trimmed;
}
/** Max input chars (~6k tokens at ~4 chars/token) to leave room for prompt + output. */
const MAX_INPUT_CHARS = 24_000;
/**
 * Extract structured JSON from markdown using Cloudflare Workers AI.
 * When schema is provided, uses response_format for JSON Mode (structured output).
 */
export async function extractWithLlm(ai, markdown, options = {}) {
    const { prompt, schema } = options;
    const trimmed = markdown.length > MAX_INPUT_CHARS
        ? `${markdown.slice(0, MAX_INPUT_CHARS)}\n\n[... truncated]`
        : markdown;
    const hadTruncation = markdown.length > MAX_INPUT_CHARS;
    const warning = hadTruncation
        ? `Input trimmed from ${markdown.length} to ${MAX_INPUT_CHARS} chars for LLM context limits`
        : undefined;
    const userContent = prompt
        ? `Transform the following content into structured JSON output based on the provided schema and this user request: ${prompt}. If schema is provided, strictly follow it.\n\n---\n\n${trimmed}`
        : `Transform the following content into structured JSON output based on the provided schema if any. Extract key information as a JSON object.\n\n---\n\n${trimmed}`;
    const messages = [
        {
            role: "system",
            content: "You are a precise data extraction assistant. Output valid JSON only. Do not include markdown code fences or explanation.",
        },
        { role: "user", content: userContent },
    ];
    const runOptions = {
        messages,
        max_tokens: 4096,
        temperature: 0.1,
    };
    if (schema) {
        const jsonSchema = normalizeSchemaForWorkersAi(schema);
        runOptions.response_format = {
            type: "json_schema",
            json_schema: jsonSchema,
        };
    }
    try {
        const result = await ai.run(DEFAULT_MODEL, runOptions);
        const raw = result?.response;
        if (typeof raw !== "string") {
            throw new Error(`AI returned unexpected format: ${typeof raw}`);
        }
        const parsed = JSON.parse(stripJsonFences(raw));
        return { data: parsed, warning };
    }
    catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes("JSON Mode couldn't be met")) {
            logger.warn("[llm-extract] JSON schema not satisfied, retrying without schema");
            const fallback = await ai.run(DEFAULT_MODEL, {
                messages,
                max_tokens: 4096,
                temperature: 0.1,
            });
            const raw = fallback?.response;
            if (typeof raw !== "string") {
                throw e;
            }
            try {
                const parsed = JSON.parse(stripJsonFences(raw));
                return {
                    data: parsed,
                    warning: warning
                        ? `${warning}; schema strict mode failed, used free-form JSON`
                        : "Schema strict mode failed, used free-form JSON",
                };
            }
            catch {
                throw e;
            }
        }
        throw e;
    }
}
/** Normalize user schema for Workers AI json_schema format. */
function normalizeSchemaForWorkersAi(schema) {
    if (!schema || typeof schema !== "object")
        return schema;
    const s = { ...schema };
    if (s.type === "array" && !s.properties) {
        return {
            type: "object",
            properties: {
                items: s,
            },
            required: ["items"],
            additionalProperties: false,
        };
    }
    if (!s.type && s.properties) {
        return {
            type: "object",
            properties: s.properties,
            required: Array.isArray(s.required)
                ? s.required
                : Object.keys(s.properties || {}),
            additionalProperties: false,
        };
    }
    return s;
}
//# sourceMappingURL=llm-extract.js.map