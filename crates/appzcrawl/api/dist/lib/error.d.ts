export type ErrorCodes = "SCRAPE_TIMEOUT" | "MAP_TIMEOUT" | "UNKNOWN_ERROR" | "SCRAPE_ALL_ENGINES_FAILED" | "SCRAPE_SSL_ERROR" | "SCRAPE_SITE_ERROR" | "SCRAPE_JOB_CANCELLED" | "BAD_REQUEST_INVALID_JSON" | "BAD_REQUEST";
export declare class TransportableError extends Error {
    readonly code: ErrorCodes;
    constructor(code: ErrorCodes, message?: string, options?: ErrorOptions);
}
//# sourceMappingURL=error.d.ts.map