import { ScrapeActionContent } from "../../../lib/entities";
import { Meta } from "..";
import { PdfMetadata } from "@mendable/firecrawl-rs";
import { BrandingProfile } from "../../../types/branding";
export type Engine = "fire-engine;chrome-cdp" | "fire-engine(retry);chrome-cdp" | "fire-engine;chrome-cdp;stealth" | "fire-engine(retry);chrome-cdp;stealth" | "fire-engine;playwright" | "fire-engine;playwright;stealth" | "fire-engine;tlsclient" | "fire-engine;tlsclient;stealth" | "playwright" | "fetch" | "pdf" | "document" | "index" | "index;documents";
declare const featureFlags: readonly ["actions", "waitFor", "screenshot", "screenshot@fullScreen", "pdf", "document", "atsv", "location", "mobile", "skipTlsVerification", "useFastMode", "stealthProxy", "branding", "disableAdblock"];
export type FeatureFlag = (typeof featureFlags)[number];
export type EngineScrapeResult = {
    url: string;
    html: string;
    markdown?: string;
    statusCode: number;
    error?: string;
    screenshot?: string;
    actions?: {
        screenshots: string[];
        scrapes: ScrapeActionContent[];
        javascriptReturns: {
            type: string;
            value: unknown;
        }[];
        pdfs: string[];
    };
    branding?: BrandingProfile;
    pdfMetadata?: PdfMetadata;
    cacheInfo?: {
        created_at: Date;
    };
    contentType?: string;
    youtubeTranscriptContent?: any;
    postprocessorsUsed?: string[];
    proxyUsed: "basic" | "stealth";
    timezone?: string;
};
export declare function shouldUseIndex(meta: Meta): any;
export declare function buildFallbackList(meta: Meta): Promise<{
    engine: Engine;
    unsupportedFeatures: Set<FeatureFlag>;
}[]>;
export declare function scrapeURLWithEngine(meta: Meta, engine: Engine): Promise<EngineScrapeResult>;
export declare function getEngineMaxReasonableTime(meta: Meta, engine: Engine): number;
export {};
//# sourceMappingURL=index.d.ts.map