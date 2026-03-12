import { Logger } from "winston";
import { type Document, type ScrapeOptions, type TeamFlags } from "../../controllers/v2/types";
import { ScrapeOptions as ScrapeOptionsV1 } from "../../controllers/v1/types";
import { Engine, FeatureFlag } from "./engines";
import { MockState } from "./lib/mock";
import { CostTracking } from "../../lib/cost-tracking";
import { AbortInstance, AbortManager } from "./lib/abortManager";
export type ScrapeUrlResponse = {
    success: true;
    document: Document;
    unsupportedFeatures?: Set<FeatureFlag>;
} | {
    success: false;
    error: any;
};
export type Meta = {
    id: string;
    url: string;
    rewrittenUrl?: string;
    options: ScrapeOptions & {
        skipTlsVerification: boolean;
    };
    internalOptions: InternalOptions;
    logger: Logger;
    abort: AbortManager;
    featureFlags: Set<FeatureFlag>;
    mock: MockState | null;
    pdfPrefetch: {
        filePath: string;
        url?: string;
        status: number;
        proxyUsed: "basic" | "stealth";
        contentType?: string;
    } | null | undefined;
    documentPrefetch: {
        filePath: string;
        url?: string;
        status: number;
        proxyUsed: "basic" | "stealth";
        contentType?: string;
    } | null | undefined;
    costTracking: CostTracking;
    winnerEngine?: Engine;
    abortHandle?: NodeJS.Timeout;
};
export type InternalOptions = {
    teamId: string;
    crawlId?: string;
    priority?: number;
    forceEngine?: Engine | Engine[];
    atsv?: boolean;
    v0CrawlOnlyUrls?: boolean;
    v0DisableJsDom?: boolean;
    disableSmartWaitCache?: boolean;
    isBackgroundIndex?: boolean;
    externalAbort?: AbortInstance;
    urlInvisibleInCurrentCrawl?: boolean;
    unnormalizedSourceURL?: string;
    saveScrapeResultToGCS?: boolean;
    bypassBilling?: boolean;
    zeroDataRetention?: boolean;
    teamFlags?: TeamFlags;
    v1Agent?: ScrapeOptionsV1["agent"];
    v1JSONAgent?: Exclude<ScrapeOptionsV1["jsonOptions"], undefined>["agent"];
    v1JSONSystemPrompt?: string;
    v1OriginalFormat?: "extract" | "json";
    isPreCrawl?: boolean;
};
export declare function scrapeURL(id: string, url: string, options: ScrapeOptions, internalOptions: InternalOptions, costTracking: CostTracking): Promise<ScrapeUrlResponse>;
//# sourceMappingURL=index.d.ts.map