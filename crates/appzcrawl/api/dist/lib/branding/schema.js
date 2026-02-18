import { z } from "zod";
const baseBrandingEnhancementSchema = z.object({
    buttonClassification: z
        .object({
        primaryButtonIndex: z.number(),
        primaryButtonReasoning: z.string(),
        secondaryButtonIndex: z.number(),
        secondaryButtonReasoning: z.string(),
        confidence: z.number().min(0).max(1),
    })
        .default({
        primaryButtonIndex: -1,
        primaryButtonReasoning: "LLM did not return button classification",
        secondaryButtonIndex: -1,
        secondaryButtonReasoning: "LLM did not return button classification",
        confidence: 0,
    }),
    colorRoles: z.object({
        primaryColor: z.string(),
        accentColor: z.string(),
        backgroundColor: z.string(),
        textPrimary: z.string(),
        confidence: z.number().min(0).max(1),
    }),
    personality: z.object({
        tone: z.enum([
            "professional",
            "playful",
            "modern",
            "traditional",
            "minimalist",
            "bold",
        ]),
        energy: z.enum(["low", "medium", "high"]),
        targetAudience: z.string(),
    }),
    designSystem: z.object({
        framework: z.enum([
            "tailwind",
            "bootstrap",
            "material",
            "chakra",
            "custom",
            "unknown",
        ]),
        componentLibrary: z.string(),
    }),
    cleanedFonts: z
        .array(z.object({
        family: z.string(),
        role: z.enum(["heading", "body", "monospace", "display", "unknown"]),
    }))
        .max(5)
        .default([]),
});
const brandingEnhancementSchemaWithLogo = baseBrandingEnhancementSchema.extend({
    logoSelection: z.object({
        selectedLogoIndex: z.number(),
        selectedLogoReasoning: z.string(),
        confidence: z.number().min(0).max(1),
    }),
});
export function getBrandingEnhancementSchema(hasLogoCandidates) {
    return hasLogoCandidates
        ? brandingEnhancementSchemaWithLogo
        : baseBrandingEnhancementSchema;
}
//# sourceMappingURL=schema.js.map