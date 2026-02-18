/**
 * Format processing for the scrape pipeline.
 * Extracts assets, images, and markdown from HTML based on requested formats.
 * Shared between cache-derived documents and fresh scrapes (DRY).
 */

import {
  getAssetFormatsToExtract,
  type ScrapeFormat,
} from "../contracts/scrape";
import { removeBase64ImagesFromMarkdown } from "../lib/removeBase64Images";
import type { AppEnv } from "../types";
import {
  type ExtractedAssetsResult,
  extractAssets,
  extractImages,
  htmlToMarkdown,
  postProcessMarkdown,
} from "./html-processor";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FormatProcessingResult {
  markdown: string;
  assets: string[];
  images: string[];
  /** Whether any asset extraction was performed (used for conditional spreading). */
  wantsAnyAssetExtraction: boolean;
  /** Whether images format was requested. */
  wantsImages: boolean;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flatten an ExtractedAssetsResult into a single string array. */
export function flattenAssets(result: ExtractedAssetsResult): string[] {
  return [
    ...(result.images ?? []),
    ...(result.css ?? []),
    ...(result.js ?? []),
    ...(result.fonts ?? []),
    ...(result.videos ?? []),
    ...(result.audio ?? []),
    ...(result.iframes ?? []),
  ];
}

// ---------------------------------------------------------------------------
// Unified format processing
// ---------------------------------------------------------------------------

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
export async function processFormats(
  env: AppEnv["Bindings"],
  html: string,
  rawHtml: string,
  url: string,
  formats: ScrapeFormat[],
  citations: boolean,
  removeBase64Images: boolean,
): Promise<FormatProcessingResult> {
  const assetFormatsToExtract = getAssetFormatsToExtract(formats);
  const wantsAnyAssetExtraction = assetFormatsToExtract.length > 0;
  const wantsImages = formats.includes("images");
  const wantsMarkdown =
    formats.includes("markdown") || formats.includes("json");

  // Extract assets (use rawHtml when available — <head> has CSS/JS links)
  let assets: string[] = [];
  let assetsResult: ExtractedAssetsResult | null = null;
  if (wantsAnyAssetExtraction) {
    try {
      const htmlForExtract = rawHtml || html;
      assetsResult = await extractAssets(
        env,
        htmlForExtract,
        url,
        assetFormatsToExtract,
      );
      assets = flattenAssets(assetsResult);
    } catch {
      assets = [];
    }
  }

  // Extract images (reuse from assets if already extracted)
  let images: string[] = [];
  if (wantsImages) {
    if (assetsResult) {
      images = assetsResult.images ?? [];
    } else {
      try {
        const imgResult = await extractImages(env, html, url);
        images = imgResult.images ?? [];
      } catch {
        images = [];
      }
    }
  }

  // Convert to markdown
  let markdown = "";
  if (wantsMarkdown) {
    try {
      const mdResult = await htmlToMarkdown(env, html);
      let mdRaw = mdResult.markdown ?? "";
      const pp = await postProcessMarkdown(env, mdRaw, {
        baseUrl: url,
        citations,
      });
      mdRaw = pp.markdown ?? mdRaw;
      markdown =
        removeBase64Images && mdRaw
          ? removeBase64ImagesFromMarkdown(mdRaw)
          : mdRaw;
    } catch {
      markdown = "";
    }
  }

  return {
    markdown,
    assets,
    images,
    wantsAnyAssetExtraction,
    wantsImages,
  };
}
