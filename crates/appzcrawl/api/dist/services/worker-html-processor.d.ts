/**
 * Worker-native HTML processing using Cloudflare's HTMLRewriter.
 *
 * Zero external dependencies — uses only APIs available in the Workers runtime.
 * Each function matches the signature/return shape of its native-container.ts
 * counterpart so the two backends are interchangeable.
 *
 * HTMLRewriter is powered by lol_html (the same Rust library the container
 * uses), so behaviour should be nearly identical for tag/attribute extraction.
 */
import type { ExtractedAssetsResult, TransformHtmlParams } from "./native-container";
export declare function extractLinks(html: string | null): Promise<{
    links: string[];
}>;
export declare function extractBaseHref(html: string, baseUrl: string): Promise<{
    baseHref: string;
}>;
export declare function extractMetadata(html: string | null): Promise<{
    metadata: Record<string, unknown>;
}>;
export declare function getInnerJson(html: string): Promise<{
    content: string;
}>;
export declare function extractImages(html: string, baseUrl: string): Promise<{
    images: string[];
}>;
export declare function extractAssets(html: string, baseUrl: string, formats?: string[]): Promise<ExtractedAssetsResult>;
export declare function postProcessMarkdown(markdown: string, _options?: {
    baseUrl?: string;
    citations?: boolean;
}): {
    markdown: string;
};
export declare function parseSitemap(xml: string): {
    urls: string[];
    sitemapUrls: string[];
};
/**
 * Returns `true` when the Worker-native implementation can handle the
 * given params.  Falls back to the container for features that require
 * DOM-tree operations (include_tags, OMCE signatures).
 */
export declare function canHandleTransformHtml(params: TransformHtmlParams): boolean;
/**
 * Transform and clean HTML using HTMLRewriter.
 *
 * What it does (matching the Rust implementation):
 * 1. Strip `<head>`, `<meta>`, `<script>`, `<style>`, `<noscript>`
 * 2. Apply `exclude_tags` removals
 * 3. If `only_main_content`: remove 42 non-main selectors (nav, header,
 *    footer, sidebar, ads, social, cookie banners, etc.)
 * 4. Resolve relative `<img src>` and `<a href>` to absolute URLs
 * 5. Pick largest srcset image as the `src`
 *
 * Does NOT handle (falls back to container via `canHandleTransformHtml`):
 * - `include_tags` (needs DOM tree to "keep only matching elements")
 * - OMCE signature matching (needs tree traversal)
 */
export declare function transformHtml(params: TransformHtmlParams): Promise<{
    html: string;
}>;
//# sourceMappingURL=worker-html-processor.d.ts.map