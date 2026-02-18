import { GenerateCompletionsOptions } from "../transformers/llmExtract";
export declare function extractData({ extractOptions, urls, useAgent, extractId, sessionId, scrapeId, metadata, }: {
    extractOptions: GenerateCompletionsOptions;
    urls: string[];
    useAgent: boolean;
    extractId?: string;
    sessionId?: string;
    scrapeId?: string;
    metadata: {
        teamId: string;
        functionId?: string;
    };
}): Promise<{
    extractedDataArray: any[];
    warning: any;
    costLimitExceededTokenUsage: number | null;
}>;
//# sourceMappingURL=extractSmartScrape.d.ts.map