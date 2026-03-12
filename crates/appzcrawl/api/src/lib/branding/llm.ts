/**
 * Branding LLM enhancement using Cloudflare Workers AI (env.AI).
 * Uses the same prompt as Firecrawl; falls back to heuristics when AI is unavailable or fails.
 */

import { selectLogoWithConfidence } from "./logo-selector";
import { buildBrandingPrompt } from "./prompt";
import {
  type BrandingEnhancement,
  getBrandingEnhancementSchema,
} from "./schema";
import type { BrandingLLMInput } from "./types";

const BRANDING_AI_MODEL = "@cf/meta/llama-3.1-8b-instruct";

/** Default enhancement when LLM fails or AI binding is not available. */
const DEFAULT_ENHANCEMENT: BrandingEnhancement = {
  buttonClassification: {
    primaryButtonIndex: -1,
    primaryButtonReasoning: "LLM not configured or failed; heuristic-only mode",
    secondaryButtonIndex: -1,
    secondaryButtonReasoning:
      "LLM not configured or failed; heuristic-only mode",
    confidence: 0,
  },
  colorRoles: {
    primaryColor: "#000000",
    accentColor: "#000000",
    backgroundColor: "#FFFFFF",
    textPrimary: "#111111",
    confidence: 0,
  },
  personality: {
    tone: "professional",
    energy: "medium",
    targetAudience: "General audience",
  },
  designSystem: {
    framework: "unknown",
    componentLibrary: "",
  },
  cleanedFonts: [],
};

/** JSON Schema for Cloudflare Workers AI response_format (base, no logo). */
function getBrandingJsonSchema(hasLogoCandidates: boolean): object {
  const base: Record<string, unknown> = {
    type: "object",
    properties: {
      buttonClassification: {
        type: "object",
        properties: {
          primaryButtonIndex: { type: "number" },
          primaryButtonReasoning: { type: "string" },
          secondaryButtonIndex: { type: "number" },
          secondaryButtonReasoning: { type: "string" },
          confidence: { type: "number" },
        },
        required: [
          "primaryButtonIndex",
          "primaryButtonReasoning",
          "secondaryButtonIndex",
          "secondaryButtonReasoning",
          "confidence",
        ],
      },
      colorRoles: {
        type: "object",
        properties: {
          primaryColor: { type: "string" },
          accentColor: { type: "string" },
          backgroundColor: { type: "string" },
          textPrimary: { type: "string" },
          confidence: { type: "number" },
        },
        required: [
          "primaryColor",
          "accentColor",
          "backgroundColor",
          "textPrimary",
          "confidence",
        ],
      },
      personality: {
        type: "object",
        properties: {
          tone: {
            type: "string",
            enum: [
              "professional",
              "playful",
              "modern",
              "traditional",
              "minimalist",
              "bold",
            ],
          },
          energy: { type: "string", enum: ["low", "medium", "high"] },
          targetAudience: { type: "string" },
        },
        required: ["tone", "energy", "targetAudience"],
      },
      designSystem: {
        type: "object",
        properties: {
          framework: {
            type: "string",
            enum: [
              "tailwind",
              "bootstrap",
              "material",
              "chakra",
              "custom",
              "unknown",
            ],
          },
          componentLibrary: { type: "string" },
        },
        required: ["framework", "componentLibrary"],
      },
      cleanedFonts: {
        type: "array",
        items: {
          type: "object",
          properties: {
            family: { type: "string" },
            role: {
              type: "string",
              enum: ["heading", "body", "monospace", "display", "unknown"],
            },
          },
          required: ["family", "role"],
        },
      },
    },
    required: [
      "buttonClassification",
      "colorRoles",
      "personality",
      "designSystem",
      "cleanedFonts",
    ],
  };

  if (hasLogoCandidates) {
    (base.properties as Record<string, unknown>).logoSelection = {
      type: "object",
      properties: {
        selectedLogoIndex: { type: "number" },
        selectedLogoReasoning: { type: "string" },
        confidence: { type: "number" },
      },
      required: ["selectedLogoIndex", "selectedLogoReasoning", "confidence"],
    };
    (base.required as string[]).push("logoSelection");
  }

  return base;
}

/** Cloudflare Workers AI binding - run(modelId, options) returns { response?: unknown }. */
type AiBinding = {
  run: (
    model: string,
    options: {
      messages?: Array<{ role: string; content: string }>;
      response_format?: { type: string; json_schema: object };
      max_tokens?: number;
    },
  ) => Promise<unknown>;
};

/**
 * Enhance branding using Cloudflare Workers AI.
 * When aiBinding is undefined, or when the AI call fails, falls back to heuristics.
 */
export async function enhanceBrandingWithLLM(
  input: BrandingLLMInput,
  aiBinding?: AiBinding,
): Promise<BrandingEnhancement> {
  const logoCandidates = input.logoCandidates ?? [];
  const heuristicResult =
    logoCandidates.length > 0
      ? selectLogoWithConfidence(logoCandidates, input.brandName)
      : null;

  if (!aiBinding) {
    return {
      ...DEFAULT_ENHANCEMENT,
      logoSelection:
        logoCandidates.length > 0 && heuristicResult
          ? {
              selectedLogoIndex: heuristicResult.selectedIndex,
              selectedLogoReasoning: `Heuristic fallback (AI not configured): ${heuristicResult.reasoning}`,
              confidence: heuristicResult.confidence,
            }
          : undefined,
    };
  }

  const prompt = buildBrandingPrompt(input);
  const hasLogoCandidates = logoCandidates.length > 0;
  const jsonSchema = getBrandingJsonSchema(hasLogoCandidates);

  try {
    const response = await aiBinding.run(BRANDING_AI_MODEL, {
      messages: [
        {
          role: "system",
          content:
            "You are a brand design expert analyzing websites to extract accurate branding information. Respond ONLY with valid JSON matching the schema.",
        },
        { role: "user", content: prompt },
      ],
      response_format: {
        type: "json_schema",
        json_schema: jsonSchema,
      },
      max_tokens: 2048,
    });

    const raw = response as { response?: unknown };
    const parsed = raw?.response;

    if (parsed == null || typeof parsed !== "object") {
      throw new Error("AI returned invalid response shape");
    }

    const zodSchema = getBrandingEnhancementSchema(hasLogoCandidates);
    const result = zodSchema.parse(parsed) as BrandingEnhancement;

    if (!hasLogoCandidates && result.logoSelection != null) {
      const { logoSelection: _, ...rest } = result;
      return rest as BrandingEnhancement;
    }

    return result;
  } catch {
    return {
      ...DEFAULT_ENHANCEMENT,
      logoSelection:
        logoCandidates.length > 0 && heuristicResult
          ? {
              selectedLogoIndex: heuristicResult.selectedIndex,
              selectedLogoReasoning: `Heuristic fallback (LLM failed): ${heuristicResult.reasoning}`,
              confidence: heuristicResult.confidence,
            }
          : undefined,
    };
  }
}
