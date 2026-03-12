import type { Logger } from "winston";
import { ScrapeRetryLimitReason, ScrapeRetryStats } from "./error";
type RetryReason = Exclude<ScrapeRetryLimitReason, "global">;
interface ScrapeRetryTrackerConfig {
    maxAttempts: number;
    maxFeatureToggles: number;
    maxFeatureRemovals: number;
    maxPdfPrefetches: number;
    maxDocumentPrefetches: number;
}
export declare class ScrapeRetryTracker {
    private readonly config;
    private readonly logger;
    private stats;
    constructor(config: ScrapeRetryTrackerConfig, logger: Logger);
    getSnapshot(): ScrapeRetryStats;
    record(reason: RetryReason, lastError: unknown): void;
    private throwLimit;
}
export {};
//# sourceMappingURL=retryTracker.d.ts.map