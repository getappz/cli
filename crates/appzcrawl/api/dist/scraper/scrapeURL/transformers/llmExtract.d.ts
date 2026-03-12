import { Document, JsonFormatWithOptions, TokenUsage } from "../../../controllers/v2/types";
import { Logger } from "winston";
import { Meta } from "..";
import { LanguageModel } from "ai";
import { CostTracking } from "../../../lib/cost-tracking";
type LanguageModelV1ProviderMetadata = {
    anthropic?: {
        thinking?: {
            type: "enabled" | "disabled";
            budgetTokens?: number;
        };
        tool_choice?: "auto" | "none" | "required";
    };
};
export declare class LLMRefusalError extends Error {
    refusal: string;
    constructor(refusal: string);
}
interface TrimResult {
    text: string;
    numTokens: number;
    warning?: string;
}
export declare function trimToTokenLimit(text: string, maxTokens: number, modelId?: string, previousWarning?: string): TrimResult;
export declare function calculateCost(model: string, inputTokens: number, outputTokens: number): number;
export type GenerateCompletionsOptions = {
    model?: LanguageModel;
    logger: Logger;
    options: Omit<JsonFormatWithOptions, "type" | "schema"> & {
        systemPrompt?: string;
        temperature?: number;
        schema?: any;
    };
    markdown?: string;
    previousWarning?: string;
    isExtractEndpoint?: boolean;
    mode?: "object" | "no-object";
    providerOptions?: LanguageModelV1ProviderMetadata;
    retryModel?: LanguageModel;
    costTrackingOptions: {
        costTracking: CostTracking;
        metadata: Record<string, any>;
    };
    metadata: {
        teamId: string;
        functionId?: string;
        extractId?: string;
        scrapeId?: string;
        deepResearchId?: string;
        llmsTxtId?: string;
    };
};
export declare function generateCompletions({ logger, options, markdown, previousWarning, isExtractEndpoint, model, mode, providerOptions, retryModel, costTrackingOptions, metadata, }: GenerateCompletionsOptions): Promise<{
    extract: any;
    numTokens: number;
    warning: string | undefined;
    totalUsage: TokenUsage;
    model: string;
}>;
export declare function performLLMExtract(meta: Meta, document: Document): Promise<Document>;
export declare function performSummary(meta: Meta, document: Document): Promise<Document>;
export declare function removeDefaultProperty(schema: any): any;
export declare function generateSchemaFromPrompt(prompt: string, logger: Logger, costTracking: CostTracking, metadata: {
    teamId: string;
    functionId?: string;
    extractId?: string;
    scrapeId?: string;
}): Promise<{
    extract: any;
}>;
export declare function generateCrawlerOptionsFromPrompt(prompt: string, logger: Logger, costTracking: CostTracking, metadata: {
    teamId: string;
    crawlId?: string;
}): Promise<{
    extract: any;
}>;
export {};
//# sourceMappingURL=llmExtract.d.ts.map