import { ScrapeRetryLimitError, } from "./error";
export class ScrapeRetryTracker {
    config;
    logger;
    stats = {
        totalAttempts: 0,
        addFeatureAttempts: 0,
        removeFeatureAttempts: 0,
        pdfAntibotAttempts: 0,
        documentAntibotAttempts: 0,
    };
    constructor(config, logger) {
        this.config = config;
        this.logger = logger;
    }
    getSnapshot() {
        return { ...this.stats };
    }
    record(reason, lastError) {
        this.stats.totalAttempts += 1;
        if (this.stats.totalAttempts > this.config.maxAttempts) {
            this.throwLimit("global", lastError);
        }
        switch (reason) {
            case "feature_toggle":
                this.stats.addFeatureAttempts += 1;
                if (this.stats.addFeatureAttempts > this.config.maxFeatureToggles) {
                    this.throwLimit(reason, lastError);
                }
                break;
            case "feature_removal":
                this.stats.removeFeatureAttempts += 1;
                if (this.stats.removeFeatureAttempts > this.config.maxFeatureRemovals) {
                    this.throwLimit(reason, lastError);
                }
                break;
            case "pdf_antibot":
                this.stats.pdfAntibotAttempts += 1;
                if (this.stats.pdfAntibotAttempts > this.config.maxPdfPrefetches) {
                    this.throwLimit(reason, lastError);
                }
                break;
            case "document_antibot":
                this.stats.documentAntibotAttempts += 1;
                if (this.stats.documentAntibotAttempts > this.config.maxDocumentPrefetches) {
                    this.throwLimit(reason, lastError);
                }
                break;
        }
        this.logger.warn("scrapeURL retrying after handled error", {
            reason,
            retryStats: this.getSnapshot(),
            lastError: lastError instanceof Error ? lastError.message : (lastError ?? null),
        });
    }
    throwLimit(reason, lastError) {
        const snapshot = this.getSnapshot();
        this.logger.error("scrapeURL retry limit reached", {
            reason,
            retryStats: snapshot,
            lastError: lastError instanceof Error ? lastError.message : (lastError ?? null),
        });
        throw new ScrapeRetryLimitError(reason, snapshot);
    }
}
//# sourceMappingURL=retryTracker.js.map