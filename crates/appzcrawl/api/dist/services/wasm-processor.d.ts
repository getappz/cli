/**
 * WASM-based HTML/crawl processing using the compiled firecrawl_rs Rust library.
 *
 * Build: `cargo build --target wasm32-unknown-unknown --release`
 * then:  `wasm-bindgen --target web --no-typescript --out-dir pkg ...`
 *
 * Loading uses Workers-native WASM import (`import wasm from "...wasm"`)
 * which gives a WebAssembly.Module directly.  All wrapper functions + initSync
 * come from the single generated `firecrawl_wasm.js` (--target web puts
 * everything in one file with one `let wasm;` variable).
 *
 * @see https://developers.cloudflare.com/workers/runtime-apis/webassembly/
 */
import type { ExtractAttributesOption, ExtractedAssetsResult, FilterLinksParams, FilterLinksResult, TransformHtmlParams } from "./native-container";
/** Returns `true` when the WASM backend is loaded and functional. */
export declare function isAvailable(): boolean;
/** Returns the init error message if WASM failed to load, or undefined. */
export declare function getInitError(): string | undefined;
/** Returns diagnostic info about the WASM module import resolution. */
export declare function getWasmDiagnostics(): Record<string, unknown>;
export declare function extractLinks(html: string | null): {
    links: string[];
};
export declare function extractBaseHref(html: string, baseUrl: string): {
    baseHref: string;
};
export declare function extractMetadata(html: string | null): {
    metadata: unknown;
};
export declare function transformHtml(params: TransformHtmlParams): {
    html: string;
};
export declare function getInnerJson(html: string): {
    content: string;
};
export declare function extractAttributes(html: string, options: {
    selectors: ExtractAttributesOption[];
}): {
    results: unknown;
};
export declare function extractImages(html: string, baseUrl: string): {
    images: string[];
};
export declare function extractAssets(html: string, baseUrl: string, formats?: string[]): ExtractedAssetsResult;
export declare function postProcessMarkdown(markdown: string, _options?: {
    baseUrl?: string;
    citations?: boolean;
}): {
    markdown: string;
};
export declare function filterLinks(params: FilterLinksParams): FilterLinksResult;
export declare function filterUrl(params: {
    href: string;
    url: string;
    baseUrl: string;
    excludes?: string[];
    ignoreRobotsTxt?: boolean;
    robotsTxt?: string;
    allowExternalContentLinks?: boolean;
    allowSubdomains?: boolean;
}): {
    allowed: boolean;
    url: string | null;
    denialReason: string | null;
};
export declare function parseSitemap(xml: string): {
    urls: string[];
    sitemapUrls: string[];
};
export declare function parseSitemapXml(xml: string): unknown;
export declare function convertDocumentToHtml(params: {
    data: Uint8Array;
    url?: string;
    contentType?: string | null;
}): {
    html: string;
};
export declare function computeEngpickerVerdict(params: {
    results: unknown[];
    similarityThreshold: number;
    successRateThreshold: number;
    cdpFailureThreshold: number;
}): unknown;
//# sourceMappingURL=wasm-processor.d.ts.map