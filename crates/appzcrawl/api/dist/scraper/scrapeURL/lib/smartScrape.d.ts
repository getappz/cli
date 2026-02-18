import { z } from "zod";
import { CostTracking } from "../../../lib/cost-tracking";
declare const smartScrapeResultSchema: z.ZodObject<{
    sessionId: z.ZodString;
    success: z.ZodBoolean;
    scrapedPages: z.ZodArray<z.ZodObject<{
        html: z.ZodString;
        reason: z.ZodString;
        page: z.ZodUnion<[z.ZodString, z.ZodNumber]>;
    }, "strip", z.ZodTypeAny, {
        html: string;
        reason: string;
        page: string | number;
    }, {
        html: string;
        reason: string;
        page: string | number;
    }>, "many">;
    tokenUsage: z.ZodNumber;
}, "strip", z.ZodTypeAny, {
    success: boolean;
    sessionId: string;
    scrapedPages: {
        html: string;
        reason: string;
        page: string | number;
    }[];
    tokenUsage: number;
}, {
    success: boolean;
    sessionId: string;
    scrapedPages: {
        html: string;
        reason: string;
        page: string | number;
    }[];
    tokenUsage: number;
}>;
export type SmartScrapeResult = z.infer<typeof smartScrapeResultSchema>;
/**
 * Sends a POST request to the internal /smart-scrape endpoint to extract
 * structured data from a URL based on a prompt.
 *
 * @param url The URL of the page to scrape.
 * @param prompt The prompt guiding the data extraction.
 * @returns A promise that resolves to an object matching the SmartScrapeResult type.
 * @throws Throws an error if the request fails or the response is invalid.
 */
export declare function smartScrape({ url, prompt, sessionId, extractId, scrapeId, beforeSubmission, costTracking, }: {
    url: string;
    prompt: string;
    sessionId?: string;
    extractId?: string;
    scrapeId?: string;
    beforeSubmission?: () => unknown;
    costTracking: CostTracking;
}): Promise<SmartScrapeResult>;
export {};
//# sourceMappingURL=smartScrape.d.ts.map