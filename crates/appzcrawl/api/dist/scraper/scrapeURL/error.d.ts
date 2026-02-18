import { ErrorCodes, TransportableError } from "../../lib/error";
import { Meta } from ".";
import { Engine, FeatureFlag } from "./engines";
export declare class EngineError extends Error {
    constructor(message?: string, options?: ErrorOptions);
}
export declare class NoEnginesLeftError extends TransportableError {
    fallbackList: Engine[];
    constructor(fallbackList: Engine[]);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): NoEnginesLeftError;
}
export declare class AddFeatureError extends Error {
    featureFlags: FeatureFlag[];
    pdfPrefetch: Meta["pdfPrefetch"];
    documentPrefetch: Meta["documentPrefetch"];
    constructor(featureFlags: FeatureFlag[], pdfPrefetch?: Meta["pdfPrefetch"], documentPrefetch?: Meta["documentPrefetch"]);
}
export declare class RemoveFeatureError extends Error {
    featureFlags: FeatureFlag[];
    constructor(featureFlags: FeatureFlag[]);
}
export declare class SSLError extends TransportableError {
    skipTlsVerification: boolean;
    constructor(skipTlsVerification: boolean);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): SSLError;
}
export declare class SiteError extends TransportableError {
    errorCode: string;
    constructor(errorCode: string);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): SiteError;
}
export declare class ProxySelectionError extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): ProxySelectionError;
}
export declare class ActionError extends TransportableError {
    errorCode: string;
    constructor(errorCode: string);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): ActionError;
}
export declare class UnsupportedFileError extends TransportableError {
    reason: string;
    constructor(reason: string);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): UnsupportedFileError;
}
export declare class PDFAntibotError extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): PDFAntibotError;
}
export declare class PDFInsufficientTimeError extends TransportableError {
    pageCount: number;
    minTimeout: number;
    constructor(pageCount: number, minTimeout: number);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): PDFInsufficientTimeError;
}
export declare class DNSResolutionError extends TransportableError {
    hostname: string;
    constructor(hostname: string);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): DNSResolutionError;
}
export declare class IndexMissError extends Error {
    constructor();
}
export declare class NoCachedDataError extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): NoCachedDataError;
}
export declare class ZDRViolationError extends TransportableError {
    feature: string;
    constructor(feature: string);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): ZDRViolationError;
}
export declare class PDFPrefetchFailed extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): PDFPrefetchFailed;
}
export declare class DocumentAntibotError extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): DocumentAntibotError;
}
export declare class DocumentPrefetchFailed extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): DocumentPrefetchFailed;
}
export declare class FEPageLoadFailed extends Error {
    constructor();
}
export declare class EngineSnipedError extends Error {
    name: string;
    constructor();
}
export declare class EngineUnsuccessfulError extends Error {
    name: string;
    constructor(engine: Engine);
}
export declare class WaterfallNextEngineSignal extends Error {
    name: string;
    constructor();
}
export declare class ScrapeJobCancelledError extends TransportableError {
    constructor();
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): ScrapeJobCancelledError;
}
export type ScrapeRetryLimitReason = "global" | "feature_toggle" | "feature_removal" | "pdf_antibot" | "document_antibot";
export type ScrapeRetryStats = {
    totalAttempts: number;
    addFeatureAttempts: number;
    removeFeatureAttempts: number;
    pdfAntibotAttempts: number;
    documentAntibotAttempts: number;
};
export declare class ScrapeRetryLimitError extends TransportableError {
    reason: ScrapeRetryLimitReason;
    stats: ScrapeRetryStats;
    constructor(reason: ScrapeRetryLimitReason, stats: ScrapeRetryStats);
    serialize(): any;
    static deserialize(_: ErrorCodes, data: ReturnType<typeof this.prototype.serialize>): ScrapeRetryLimitError;
}
//# sourceMappingURL=error.d.ts.map