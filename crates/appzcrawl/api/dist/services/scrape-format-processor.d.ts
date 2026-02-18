/**
 * Format processing for the scrape pipeline.
 * Extracts assets, images, and markdown from HTML based on requested formats.
 * Shared between cache-derived documents and fresh scrapes (DRY).
 */
import { type ScrapeFormat } from "../contracts/scrape";
import type { AppEnv } from "../types";
import { type ExtractedAssetsResult } from "./html-processor";
export interface FormatProcessingResult {
    markdown: string;
    assets: string[];
    images: string[];
    /** Whether any asset extraction was performed (used for conditional spreading). */
    wantsAnyAssetExtraction: boolean;
    /** Whether images format was requested. */
    wantsImages: boolean;
}
/** Flatten an ExtractedAssetsResult into a single string array. */
export declare function flattenAssets(result: ExtractedAssetsResult): string[];
/**
 * Process formats (assets, images, markdown) from HTML.
 * Used by both deriveDocumentFromHtml (cache path) and runScrapeUrl (fresh path).
 *
 * @param env - Worker bindings
 * @param html - Transformed HTML (main content extracted)
 * @param rawHtml - Full raw HTML (for asset extraction from <head>)
 * @param url - Page URL for link resolution
 * @param formats - Requested output formats
 * @param citations - Whether to convert links to citations
 * @param removeBase64Images - Whether to strip base64 images from markdown
 */
export declare function processFormats(env: AppEnv["Bindings"], html: string, rawHtml: string, url: string, formats: ScrapeFormat[], citations: boolean, removeBase64Images: boolean): Promise<FormatProcessingResult>;
//# sourceMappingURL=scrape-format-processor.d.ts.map