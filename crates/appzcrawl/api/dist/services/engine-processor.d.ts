/**
 * Engine processor — calls appzcrawl-engine (workers-rs) via RPC Service Binding.
 *
 * Each function calls the corresponding `rpc_*` export on the engine Worker.
 * All data crosses the binding as JSON strings — no HTTP overhead.
 *
 * The engine Worker compiles firecrawl_rs + htmd to WASM and runs as a
 * standalone Worker, giving it its own CPU/memory budget separate from
 * the main appzcrawl API Worker.
 */
import type { AppEnv } from "../types";
import type { ExtractAttributesOption, ExtractedAssetsResult, FilterLinksParams, FilterLinksResult, NativeSearchOptions, NativeSearchResponse, TransformHtmlParams } from "./native-container";
/** Returns `true` when the APPZCRAWL_ENGINE binding is present. */
export declare function isAvailable(env: AppEnv["Bindings"]): boolean;
export declare function extractLinks(env: AppEnv["Bindings"], html: string | null): Promise<{
    links: string[];
}>;
export declare function extractBaseHref(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    baseHref: string;
}>;
export declare function extractMetadata(env: AppEnv["Bindings"], html: string | null): Promise<{
    metadata: unknown;
}>;
export declare function transformHtml(env: AppEnv["Bindings"], params: TransformHtmlParams): Promise<{
    html: string;
}>;
export declare function getInnerJson(env: AppEnv["Bindings"], html: string): Promise<{
    content: string;
}>;
export declare function extractAttributes(env: AppEnv["Bindings"], html: string, options: {
    selectors: ExtractAttributesOption[];
}): Promise<{
    results: unknown;
}>;
export declare function extractImages(env: AppEnv["Bindings"], html: string, baseUrl: string): Promise<{
    images: string[];
}>;
export declare function extractAssets(env: AppEnv["Bindings"], html: string, baseUrl: string, formats?: string[]): Promise<ExtractedAssetsResult>;
export declare function postProcessMarkdown(env: AppEnv["Bindings"], markdown: string, options?: {
    baseUrl?: string;
    citations?: boolean;
}): Promise<{
    markdown: string;
}>;
export declare function htmlToMarkdown(env: AppEnv["Bindings"], html: string): Promise<{
    markdown: string;
}>;
export declare function filterLinks(env: AppEnv["Bindings"], params: FilterLinksParams): Promise<FilterLinksResult>;
export declare function parseSitemap(env: AppEnv["Bindings"], xml: string): Promise<{
    urls: string[];
    sitemapUrls: string[];
}>;
export declare function nativeSearch(env: AppEnv["Bindings"], options: NativeSearchOptions): Promise<NativeSearchResponse>;
export declare function convertDocument(env: AppEnv["Bindings"], params: {
    data: Uint8Array;
    url?: string;
    contentType?: string | null;
}): Promise<{
    html: string;
}>;
//# sourceMappingURL=engine-processor.d.ts.map