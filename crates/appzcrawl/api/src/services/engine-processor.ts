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

import type { AppEnv, AppzcrawlEngineRpc } from "../types";
import type {
  ExtractAttributesOption,
  ExtractedAssetsResult,
  FilterLinksParams,
  FilterLinksResult,
  NativeSearchOptions,
  NativeSearchResponse,
  TransformHtmlParams,
} from "./native-container";

// ---------------------------------------------------------------------------
// Availability check
// ---------------------------------------------------------------------------

/** Returns `true` when the APPZCRAWL_ENGINE binding is present. */
export function isAvailable(env: AppEnv["Bindings"]): boolean {
  return !!(env as Record<string, unknown>).APPZCRAWL_ENGINE;
}

function getEngine(env: AppEnv["Bindings"]): AppzcrawlEngineRpc {
  return (env as Record<string, unknown>)
    .APPZCRAWL_ENGINE as AppzcrawlEngineRpc;
}

// ---------------------------------------------------------------------------
// Helper to parse RPC result (JSON string) and check for errors
// ---------------------------------------------------------------------------

function parseResult<T>(json: string): T {
  const parsed = JSON.parse(json);
  if (parsed && typeof parsed === "object" && "error" in parsed) {
    throw new Error(parsed.error as string);
  }
  return parsed as T;
}

// ---------------------------------------------------------------------------
// Public API — matches wasm-processor.ts / native-container.ts signatures
// ---------------------------------------------------------------------------

export async function extractLinks(
  env: AppEnv["Bindings"],
  html: string | null,
): Promise<{ links: string[] }> {
  const result = await getEngine(env).rpc_extract_links(html ?? "");
  return { links: parseResult<string[]>(result) };
}

export async function extractBaseHref(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
): Promise<{ baseHref: string }> {
  const result = await getEngine(env).rpc_extract_base_href(html, baseUrl);
  return { baseHref: parseResult<string>(result) };
}

export async function extractMetadata(
  env: AppEnv["Bindings"],
  html: string | null,
): Promise<{ metadata: unknown }> {
  const result = await getEngine(env).rpc_extract_metadata(html ?? "");
  return { metadata: parseResult<unknown>(result) };
}

export async function transformHtml(
  env: AppEnv["Bindings"],
  params: TransformHtmlParams,
): Promise<{ html: string }> {
  const opts = JSON.stringify({
    html: params.html,
    url: params.url,
    include_tags: params.include_tags ?? [],
    exclude_tags: params.exclude_tags ?? [],
    only_main_content: params.only_main_content ?? false,
    omce_signatures: params.omce_signatures ?? null,
  });
  const result = await getEngine(env).rpc_transform_html(opts);
  return { html: parseResult<string>(result) };
}

export async function getInnerJson(
  env: AppEnv["Bindings"],
  html: string,
): Promise<{ content: string }> {
  const result = await getEngine(env).rpc_get_inner_json(html);
  return { content: parseResult<string>(result) };
}

export async function extractAttributes(
  env: AppEnv["Bindings"],
  html: string,
  options: { selectors: ExtractAttributesOption[] },
): Promise<{ results: unknown }> {
  const result = await getEngine(env).rpc_extract_attributes(
    html,
    JSON.stringify({ selectors: options.selectors }),
  );
  return { results: parseResult<unknown>(result) };
}

export async function extractImages(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
): Promise<{ images: string[] }> {
  const result = await getEngine(env).rpc_extract_images(html, baseUrl);
  return { images: parseResult<string[]>(result) };
}

export async function extractAssets(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
  formats?: string[],
): Promise<ExtractedAssetsResult> {
  const result = await getEngine(env).rpc_extract_assets(
    html,
    baseUrl,
    JSON.stringify(formats ?? ["assets"]),
  );
  return parseResult<ExtractedAssetsResult>(result);
}

export async function postProcessMarkdown(
  env: AppEnv["Bindings"],
  markdown: string,
  options?: { baseUrl?: string; citations?: boolean },
): Promise<{ markdown: string }> {
  const result = await getEngine(env).rpc_post_process_markdown(
    markdown,
    options?.baseUrl ?? "",
    options?.citations ?? false,
  );
  return { markdown: parseResult<string>(result) };
}

export async function htmlToMarkdown(
  env: AppEnv["Bindings"],
  html: string,
): Promise<{ markdown: string }> {
  const result = await getEngine(env).rpc_html_to_markdown(html);
  return { markdown: parseResult<string>(result) };
}

export async function filterLinks(
  env: AppEnv["Bindings"],
  params: FilterLinksParams,
): Promise<FilterLinksResult> {
  const body = {
    links: params.links,
    limit: params.limit ?? null,
    max_depth: params.maxDepth,
    base_url: params.baseUrl,
    initial_url: params.initialUrl,
    regex_on_full_url: params.regexOnFullUrl ?? false,
    excludes: params.excludes ?? [],
    includes: params.includes ?? [],
    allow_backward_crawling: params.allowBackwardCrawling ?? false,
    ignore_robots_txt: params.ignoreRobotsTxt ?? false,
    robots_txt: params.robotsTxt ?? "",
    allow_external_content_links: params.allowExternalContentLinks ?? false,
    allow_subdomains: params.allowSubdomains ?? false,
  };
  const result = await getEngine(env).rpc_filter_links(JSON.stringify(body));
  const parsed = parseResult<{
    links: string[];
    denial_reasons: Record<string, string>;
  }>(result);
  return { links: parsed.links, denialReasons: parsed.denial_reasons ?? {} };
}

export async function parseSitemap(
  env: AppEnv["Bindings"],
  xml: string,
): Promise<{ urls: string[]; sitemapUrls: string[] }> {
  const result = await getEngine(env).rpc_parse_sitemap(xml);
  return parseResult<{ urls: string[]; sitemapUrls: string[] }>(result);
}

export async function nativeSearch(
  env: AppEnv["Bindings"],
  options: NativeSearchOptions,
): Promise<NativeSearchResponse> {
  const opts = JSON.stringify({
    num_results: options.num_results ?? 5,
    tbs: options.tbs,
    filter: options.filter,
    lang: options.lang ?? "en",
    country: options.country ?? "us",
    location: options.location,
    timeout_ms: options.timeout_ms ?? 5000,
  });
  const result = await getEngine(env).rpc_search(options.query, opts);
  return parseResult<NativeSearchResponse>(result);
}

export async function convertDocument(
  env: AppEnv["Bindings"],
  params: { data: Uint8Array; url?: string; contentType?: string | null },
): Promise<{ html: string }> {
  // Encode data to base64
  const bytes = new Uint8Array(params.data);
  let binary = "";
  const chunk = 8192;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  const dataBase64 = btoa(binary);

  const body = JSON.stringify({
    dataBase64,
    url: params.url,
    contentType: params.contentType,
  });
  const result = await getEngine(env).rpc_convert_document(body);
  return { html: parseResult<string>(result) };
}
