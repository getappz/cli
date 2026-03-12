/**
 * Client for the fire-engine remote service (browser / TLS client scraping).
 * When FIRE_ENGINE_URL is set, the scrape runner can use this to fetch HTML
 * via fire-engine instead of Worker fetch (for JS-heavy or bot-protected sites).
 *
 * API: POST /scrape → { jobId, processing } or result; poll GET /scrape/:jobId; DELETE /scrape/:jobId.
 */
export interface FireEngineFetchResult {
    success: true;
    html: string;
    statusCode: number;
    finalUrl?: string;
}
export interface FireEngineFetchError {
    success: false;
    error: string;
    statusCode?: number;
}
export type FireEngineFetchOutput = FireEngineFetchResult | FireEngineFetchError;
/**
 * Fetch HTML for a URL using the fire-engine service (tlsclient engine).
 * Returns HTML and status code, or an error. Call only when baseUrl is non-empty.
 */
export declare function fireEngineFetchHtml(baseUrl: string, url: string, options?: {
    timeout?: number;
    skipTlsVerification?: boolean;
}): Promise<FireEngineFetchOutput>;
export declare function isFireEngineEnabled(env: {
    FIRE_ENGINE_URL?: string;
}): boolean;
//# sourceMappingURL=fire-engine-client.d.ts.map