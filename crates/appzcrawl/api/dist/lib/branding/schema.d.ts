import { z } from "zod";
declare const brandingEnhancementSchemaWithLogo: z.ZodObject<{
    buttonClassification: z.ZodDefault<z.ZodObject<{
        primaryButtonIndex: z.ZodNumber;
        primaryButtonReasoning: z.ZodString;
        secondaryButtonIndex: z.ZodNumber;
        secondaryButtonReasoning: z.ZodString;
        confidence: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    }, {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    }>>;
    colorRoles: z.ZodObject<{
        primaryColor: z.ZodString;
        accentColor: z.ZodString;
        backgroundColor: z.ZodString;
        textPrimary: z.ZodString;
        confidence: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    }, {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    }>;
    personality: z.ZodObject<{
        tone: z.ZodEnum<["professional", "playful", "modern", "traditional", "minimalist", "bold"]>;
        energy: z.ZodEnum<["low", "medium", "high"]>;
        targetAudience: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    }, {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    }>;
    designSystem: z.ZodObject<{
        framework: z.ZodEnum<["tailwind", "bootstrap", "material", "chakra", "custom", "unknown"]>;
        componentLibrary: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    }, {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    }>;
    cleanedFonts: z.ZodDefault<z.ZodArray<z.ZodObject<{
        family: z.ZodString;
        role: z.ZodEnum<["heading", "body", "monospace", "display", "unknown"]>;
    }, "strip", z.ZodTypeAny, {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }, {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }>, "many">>;
} & {
    logoSelection: z.ZodObject<{
        selectedLogoIndex: z.ZodNumber;
        selectedLogoReasoning: z.ZodString;
        confidence: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        confidence: number;
        selectedLogoIndex: number;
        selectedLogoReasoning: string;
    }, {
        confidence: number;
        selectedLogoIndex: number;
        selectedLogoReasoning: string;
    }>;
}, "strip", z.ZodTypeAny, {
    buttonClassification: {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    };
    colorRoles: {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    };
    personality: {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    };
    designSystem: {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    };
    cleanedFonts: {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }[];
    logoSelection: {
        confidence: number;
        selectedLogoIndex: number;
        selectedLogoReasoning: string;
    };
}, {
    colorRoles: {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    };
    personality: {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    };
    designSystem: {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    };
    logoSelection: {
        confidence: number;
        selectedLogoIndex: number;
        selectedLogoReasoning: string;
    };
    buttonClassification?: {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    } | undefined;
    cleanedFonts?: {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }[] | undefined;
}>;
export declare function getBrandingEnhancementSchema(hasLogoCandidates: boolean): z.ZodObject<{
    buttonClassification: z.ZodDefault<z.ZodObject<{
        primaryButtonIndex: z.ZodNumber;
        primaryButtonReasoning: z.ZodString;
        secondaryButtonIndex: z.ZodNumber;
        secondaryButtonReasoning: z.ZodString;
        confidence: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    }, {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    }>>;
    colorRoles: z.ZodObject<{
        primaryColor: z.ZodString;
        accentColor: z.ZodString;
        backgroundColor: z.ZodString;
        textPrimary: z.ZodString;
        confidence: z.ZodNumber;
    }, "strip", z.ZodTypeAny, {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    }, {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    }>;
    personality: z.ZodObject<{
        tone: z.ZodEnum<["professional", "playful", "modern", "traditional", "minimalist", "bold"]>;
        energy: z.ZodEnum<["low", "medium", "high"]>;
        targetAudience: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    }, {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    }>;
    designSystem: z.ZodObject<{
        framework: z.ZodEnum<["tailwind", "bootstrap", "material", "chakra", "custom", "unknown"]>;
        componentLibrary: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    }, {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    }>;
    cleanedFonts: z.ZodDefault<z.ZodArray<z.ZodObject<{
        family: z.ZodString;
        role: z.ZodEnum<["heading", "body", "monospace", "display", "unknown"]>;
    }, "strip", z.ZodTypeAny, {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }, {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }>, "many">>;
}, "strip", z.ZodTypeAny, {
    buttonClassification: {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    };
    colorRoles: {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    };
    personality: {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    };
    designSystem: {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    };
    cleanedFonts: {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }[];
}, {
    colorRoles: {
        textPrimary: string;
        confidence: number;
        primaryColor: string;
        accentColor: string;
        backgroundColor: string;
    };
    personality: {
        tone: "professional" | "playful" | "modern" | "traditional" | "minimalist" | "bold";
        energy: "medium" | "low" | "high";
        targetAudience: string;
    };
    designSystem: {
        framework: "unknown" | "custom" | "tailwind" | "bootstrap" | "material" | "chakra";
        componentLibrary: string;
    };
    buttonClassification?: {
        primaryButtonIndex: number;
        primaryButtonReasoning: string;
        secondaryButtonIndex: number;
        secondaryButtonReasoning: string;
        confidence: number;
    } | undefined;
    cleanedFonts?: {
        family: string;
        role: "body" | "heading" | "unknown" | "monospace" | "display";
    }[] | undefined;
}>;
export type BrandingEnhancement = Omit<z.infer<typeof brandingEnhancementSchemaWithLogo>, "logoSelection"> & {
    logoSelection?: z.infer<typeof brandingEnhancementSchemaWithLogo>["logoSelection"];
};
export {};
//# sourceMappingURL=schema.d.ts.map