/**
 * Client for the appzcrawl native addon running in a Cloudflare Container.
 * Uses getRandom() for stateless load balancing across container instances.
 */

import { getRandom } from "@cloudflare/containers";
import type { AppEnv } from "../types";

const INSTANCE_COUNT = 1;

function getBinding(env: AppEnv["Bindings"]) {
  return env.APPZCRAWL_CONTAINER;
}

/** Base URL for container requests; host is ignored, path is forwarded. */
const CONTAINER_ORIGIN = "http://native";

async function fetchNative(
  env: AppEnv["Bindings"],
  path: string,
  options:
    | { method: "GET"; body?: undefined }
    | { method: "POST"; body: string },
): Promise<Response> {
  const binding = getBinding(env);
  const container = await getRandom(binding, INSTANCE_COUNT);
  const url = `${CONTAINER_ORIGIN}${path}`;
  const request = new Request(url, {
    method: options.method,
    headers: { "Content-Type": "application/json" },
    body: options.method === "POST" ? options.body : undefined,
  });
  return container.fetch(request);
}

async function postJson<T>(
  env: AppEnv["Bindings"],
  path: string,
  body: unknown,
): Promise<T> {
  const res = await fetchNative(env, path, {
    method: "POST",
    body: JSON.stringify(body),
  });

  // Read body as text first to handle empty/non-JSON responses gracefully
  const text = await res.text();
  if (!text) {
    throw new Error(
      `Native container ${path}: empty response (HTTP ${res.status}). Container may need rebuild.`,
    );
  }

  let data: T | { error?: string };
  try {
    data = JSON.parse(text) as T | { error?: string };
  } catch {
    throw new Error(
      `Native container ${path}: invalid JSON (HTTP ${res.status}): ${text.slice(0, 200)}`,
    );
  }

  if (!res.ok) {
    const err =
      data && typeof data === "object" && "error" in data
        ? (data as { error: string }).error
        : res.statusText;
    throw new Error(err || `Native container ${path}: ${res.status}`);
  }
  return data as T;
}

export async function nativeHealth(
  env: AppEnv["Bindings"],
): Promise<{ ok: boolean }> {
  const binding = getBinding(env);
  const container = await getRandom(binding, 1);
  const res = await container.fetch(
    new Request(`${CONTAINER_ORIGIN}/health`, { method: "GET" }),
  );
  const data = await res.json();
  if (!res.ok)
    throw new Error(
      String((data as { error?: string })?.error ?? res.statusText),
    );
  return data as { ok: boolean };
}

export async function extractLinks(
  env: AppEnv["Bindings"],
  html: string | null,
): Promise<{ links: string[] }> {
  return postJson(env, "/extract-links", { html });
}

// ---------------------------------------------------------------------------
// filterLinks: Firecrawl-compatible link filtering via native Rust (crawler.rs).
// Adapted from firecrawl/apps/api/src/scraper/WebScraper/crawler.ts (filterLinks).
// ---------------------------------------------------------------------------

export interface FilterLinksParams {
  links: string[];
  limit?: number;
  maxDepth: number;
  baseUrl: string;
  initialUrl: string;
  regexOnFullUrl?: boolean;
  excludes?: string[];
  includes?: string[];
  allowBackwardCrawling?: boolean;
  ignoreRobotsTxt?: boolean;
  robotsTxt?: string;
  allowExternalContentLinks?: boolean;
  allowSubdomains?: boolean;
}

export interface FilterLinksResult {
  links: string[];
  denialReasons: Record<string, string>;
}

/** Filter discovered links using the native Rust crawler (includePaths, excludePaths, robots, depth, etc.). */
export async function filterLinks(
  env: AppEnv["Bindings"],
  params: FilterLinksParams,
): Promise<FilterLinksResult> {
  return postJson(env, "/filter-links", {
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
  });
}

/** Extract base href from HTML (firecrawl_rs). Used to resolve relative links. */
export async function extractBaseHref(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
): Promise<{ baseHref: string }> {
  return postJson(env, "/extract-base-href", { html, url: baseUrl });
}

export interface TransformHtmlParams {
  html: string;
  url: string;
  include_tags?: string[];
  exclude_tags?: string[];
  only_main_content?: boolean;
  omce_signatures?: string[];
}

export async function transformHtml(
  env: AppEnv["Bindings"],
  params: TransformHtmlParams,
): Promise<{ html: string }> {
  const body = {
    html: params.html,
    url: params.url,
    include_tags: params.include_tags,
    exclude_tags: params.exclude_tags,
    only_main_content: params.only_main_content,
    omce_signatures: params.omce_signatures,
  };
  return postJson(env, "/transform-html", body);
}

export async function getInnerJson(
  env: AppEnv["Bindings"],
  html: string,
): Promise<{ content: string }> {
  return postJson(env, "/get-inner-json", { html });
}

export async function extractMetadata(
  env: AppEnv["Bindings"],
  html: string | null,
): Promise<{ metadata: unknown }> {
  return postJson(env, "/extract-metadata", { html });
}

export interface ExtractAttributesOption {
  selector: string;
  attribute: string;
}

export async function extractAttributes(
  env: AppEnv["Bindings"],
  html: string,
  options: { selectors: ExtractAttributesOption[] },
): Promise<{ results: unknown }> {
  return postJson(env, "/extract-attributes", {
    html,
    options: { selectors: options.selectors },
  });
}

export async function extractImages(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
): Promise<{ images: string[] }> {
  return postJson(env, "/extract-images", { html, base_url: baseUrl });
}

/** Native container extract-assets response (css, js, images, fonts, videos, audio, iframes). */
export interface ExtractedAssetsResult {
  images?: string[];
  css?: string[];
  js?: string[];
  fonts?: string[];
  videos?: string[];
  audio?: string[];
  iframes?: string[];
}

export async function extractAssets(
  env: AppEnv["Bindings"],
  html: string,
  baseUrl: string,
  formats?: string[],
): Promise<ExtractedAssetsResult> {
  return postJson(env, "/extract-assets", {
    html,
    base_url: baseUrl,
    formats: formats ?? ["assets"],
  });
}

export async function postProcessMarkdown(
  env: AppEnv["Bindings"],
  markdown: string,
  options?: { baseUrl?: string; citations?: boolean },
): Promise<{ markdown: string }> {
  const body: { markdown: string; baseUrl?: string; citations?: boolean } = {
    markdown,
  };
  if (options?.baseUrl) body.baseUrl = options.baseUrl;
  if (options?.citations !== undefined) body.citations = options.citations;
  return postJson(env, "/post-process-markdown", body);
}

/** Parse sitemap XML via native container (Rust quick-xml). Returns page URLs and child sitemap URLs. */
export async function parseSitemap(
  env: AppEnv["Bindings"],
  xml: string,
): Promise<{ urls: string[]; sitemapUrls: string[] }> {
  return postJson(env, "/parse-sitemap", { xml });
}

// ---------------------------------------------------------------------------
// Search: DuckDuckGo and SearXNG via native Rust
// ---------------------------------------------------------------------------

export interface NativeSearchOptions {
  query: string;
  num_results?: number;
  tbs?: string;
  filter?: string;
  lang?: string;
  country?: string;
  location?: string;
  timeout_ms?: number;
}

export interface WebSearchResult {
  url: string;
  title: string;
  description: string;
}

export interface NativeSearchResponse {
  web?: WebSearchResult[];
}

/**
 * Perform web search using native Rust implementation.
 * - Uses SearXNG if SEARXNG_ENDPOINT is configured (env var)
 * - Falls back to DuckDuckGo if SearXNG is not available or returns no results
 * - Adapted from firecrawl/apps/api/src/search/v2/index.ts
 */
export async function nativeSearch(
  env: AppEnv["Bindings"],
  options: NativeSearchOptions,
): Promise<NativeSearchResponse> {
  const body = {
    query: options.query,
    num_results: options.num_results ?? 5,
    tbs: options.tbs,
    filter: options.filter,
    lang: options.lang ?? "en",
    country: options.country ?? "us",
    location: options.location,
    timeout_ms: options.timeout_ms ?? 5000,
  };
  return postJson(env, "/search", body);
}

/** Convert HTML to markdown via native container (Go library). */
export async function htmlToMarkdown(
  env: AppEnv["Bindings"],
  html: string,
): Promise<{ markdown: string }> {
  return postJson(env, "/html-to-markdown", { html });
}

/**
 * Convert Office documents (docx, xlsx, odt, rtf, doc, xls) to HTML.
 * Firecrawl-compatible: same flow as firecrawl document engine.
 */
export async function convertDocument(
  env: AppEnv["Bindings"],
  params: {
    /** Document binary data. */
    data: Uint8Array;
    /** URL for extension-based type detection when contentType is absent. */
    url?: string;
    /** Content-Type header for type detection. */
    contentType?: string | null;
  },
): Promise<{ html: string }> {
  const { data, url, contentType } = params;
  const bytes = new Uint8Array(data);
  let binary = "";
  const chunk = 8192;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  const dataBase64 = btoa(binary);
  const body: {
    dataBase64: string;
    url?: string;
    contentType?: string;
  } = { dataBase64 };
  if (url) body.url = url;
  if (contentType) body.contentType = contentType;
  return postJson(env, "/convert-document", body);
}

/**
 * Convert PDF to HTML.
 * Firecrawl-compatible: same flow as Firecrawl PDF engine (text → HTML for pipeline).
 */
export async function convertPdf(
  env: AppEnv["Bindings"],
  params: { data: Uint8Array },
): Promise<{ html: string }> {
  const bytes = new Uint8Array(params.data);
  let binary = "";
  const chunk = 8192;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  const dataBase64 = btoa(binary);
  return postJson(env, "/convert-pdf", { dataBase64 });
}

export async function getPdfMetadata(
  env: AppEnv["Bindings"],
  input: { pdfBase64: string } | Uint8Array,
): Promise<{ num_pages: number; title: string | null }> {
  const binding = getBinding(env);
  const container = await getRandom(binding, 1);
  const body =
    typeof input === "object" && "pdfBase64" in input
      ? JSON.stringify({ pdfBase64: input.pdfBase64 })
      : new Uint8Array(input);
  const headers: Record<string, string> =
    typeof input === "object" && "pdfBase64" in input
      ? { "Content-Type": "application/json" }
      : {};
  const res = await container.fetch(
    new Request(`${CONTAINER_ORIGIN}/get-pdf-metadata`, {
      method: "POST",
      headers,
      body,
    }),
  );
  const data = await res.json();
  if (!res.ok)
    throw new Error(
      String((data as { error?: string })?.error ?? res.statusText),
    );
  return data as { num_pages: number; title: string | null };
}
