/**
 * URL fetching logic for the scrape pipeline.
 * Handles HTML fetch (Worker fetch, fire-engine, Cloudflare Browser Rendering)
 * and document fetch (docx, xlsx, etc.) with unified timeout/abort patterns.
 */
import { type ScrapeFormat } from "../contracts/scrape";
import type { AppEnv } from "../types";
import type { ScrapeRunnerError, ScrapeRunnerOptions } from "./scrape-runner";
export declare const USER_AGENT = "Mozilla/5.0 (compatible; Appzcrawl/1.0; +https://appz.dev)";
export declare function isDocumentUrl(url: string): boolean;
export declare function isPdfUrl(url: string): boolean;
/** Resolve fetch engine for web URLs. Documents and PDFs always use native. */
export declare function resolveEngine(env: AppEnv["Bindings"], options: Pick<ScrapeRunnerOptions, "engine">): "native" | "cloudflare";
export declare function fetchHtmlWithWorker(url: string, opts?: {
    timeout?: number;
}): Promise<{
    success: true;
    rawHtml: string;
    statusCode: number;
} | ScrapeRunnerError>;
export declare function fetchDocumentWithWorker(url: string, opts?: {
    timeout?: number;
}): Promise<{
    success: true;
    data: Uint8Array;
    contentType: string;
    statusCode: number;
} | ScrapeRunnerError>;
export declare function fetchPdfWithWorker(url: string, opts?: {
    timeout?: number;
}): Promise<{
    success: true;
    data: Uint8Array;
    contentType: string;
    statusCode: number;
} | ScrapeRunnerError>;
export declare function captureScreenshotAndUpload(env: AppEnv["Bindings"], url: string, opts: {
    formats: ScrapeFormat[];
    screenshotOptions?: ScrapeRunnerOptions["screenshotOptions"];
    screenshotBaseUrl?: string;
    mobile?: boolean;
    blockAds?: boolean;
    timeout?: number;
    waitFor?: number;
}): Promise<string | undefined>;
export type FetchResult = {
    success: true;
    rawHtml: string;
    statusCode: number;
    screenshotUrl?: string;
    documentContentType?: string;
    /** From Sarvam PDF: native markdown (not derived from HTML). */
    documentMarkdown?: string;
    /** LLM-extracted JSON when available (PDF: not supported by Sarvam/LlamaParse). */
    documentJson?: unknown;
    /** From LlamaParse PDF: extracted image URLs. */
    documentImages?: string[];
} | {
    success: false;
    error: string;
};
/** Fetch HTML or document content with engine fallback strategy. */
export declare function fetchContent(env: AppEnv["Bindings"], url: string, options: {
    resolvedEngine: "native" | "cloudflare";
    useFireEngine: boolean;
    effectiveTimeout: number;
    skipTlsVerification: boolean;
    mobile: boolean;
    wantsScreenshot: boolean;
    screenshotBaseUrl?: string;
    screenshotOptions?: ScrapeRunnerOptions["screenshotOptions"];
    formats: ScrapeFormat[];
}): Promise<FetchResult>;
//# sourceMappingURL=scrape-fetcher.d.ts.map