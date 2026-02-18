/**
 * Branding runner using Cloudflare Browser Rendering (Playwright).
 * Loads URL in headless Chrome, executes branding script, returns raw branding data.
 */
import type { BrandingScriptReturn } from "../lib/branding/types";
/** Fetcher binding for Browser Rendering */
export interface BrowserBinding {
    fetch: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
}
export interface BrandingRunnerOptions {
    /** Browser binding (Cloudflare) or custom fetcher (local emulator) */
    browserBinding: BrowserBinding;
    /** URL to load */
    url: string;
    /** Timeout in ms */
    timeout?: number;
}
/**
 * Create a fetcher that proxies requests to a local Browser Rendering emulator.
 * Use with runDevBrowser.mjs (pnpm dev:browser).
 */
export declare function createLocalBrowserFetcher(origin: string): BrowserBinding;
export interface BrandingRunnerSuccess {
    success: true;
    rawBranding: BrandingScriptReturn;
}
export interface BrandingRunnerError {
    success: false;
    error: string;
}
export type BrandingRunnerResult = BrandingRunnerSuccess | BrandingRunnerError;
/**
 * Run branding extraction via Cloudflare Browser Rendering.
 * Launches headless Chrome, navigates to URL, runs branding script, returns result.
 */
export declare function runBrandingExtraction(options: BrandingRunnerOptions): Promise<BrandingRunnerResult>;
//# sourceMappingURL=branding-runner.d.ts.map