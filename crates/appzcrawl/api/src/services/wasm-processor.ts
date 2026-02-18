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

// All wrapper functions + initSync from the single generated JS file.
// With --target web, firecrawl_wasm.js contains everything in one scope
// with a single `let wasm;` that initSync sets correctly.
import {
  compute_engpicker_verdict,
  convert_document,
  extract_assets,
  extract_attributes,
  extract_base_href,
  extract_images,
  extract_links,
  extract_metadata,
  filter_links,
  filter_url,
  get_inner_json,
  initSync,
  parse_sitemap_xml,
  post_process_markdown,
  process_sitemap,
  transform_html,
} from "../wasm/firecrawl_wasm.js";
// Workers-native WASM import → WebAssembly.Module
import wasmModule from "../wasm/firecrawl_wasm_bg.wasm";

import type {
  ExtractAttributesOption,
  ExtractedAssetsResult,
  FilterLinksParams,
  FilterLinksResult,
  TransformHtmlParams,
} from "./native-container";

// ---------------------------------------------------------------------------
// Helpers to detect document type from URL/contentType (mirrors native-container.ts logic)
// ---------------------------------------------------------------------------

function detectDocType(
  url?: string,
  contentType?: string | null,
): string | null {
  const ct = contentType?.toLowerCase() ?? "";
  if (
    ct.includes(
      "application/vnd.openxmlformats-officedocument.wordprocessingml",
    )
  )
    return "docx";
  if (
    ct.includes("application/vnd.openxmlformats-officedocument.spreadsheetml")
  )
    return "xlsx";
  if (ct.includes("application/vnd.oasis.opendocument.text")) return "odt";
  if (ct.includes("application/rtf") || ct.includes("text/rtf")) return "rtf";
  if (ct.includes("application/msword")) return "doc";
  if (ct.includes("application/vnd.ms-excel")) return "xlsx";

  // Fallback: extension-based detection
  if (url) {
    const lower = url.toLowerCase();
    if (lower.endsWith(".docx")) return "docx";
    if (lower.endsWith(".xlsx") || lower.endsWith(".xls")) return "xlsx";
    if (lower.endsWith(".odt")) return "odt";
    if (lower.endsWith(".rtf")) return "rtf";
    if (lower.endsWith(".doc")) return "doc";
  }
  return null;
}

// ---------------------------------------------------------------------------
// Public API — matches native-container.ts signatures (minus `env` param)
// ---------------------------------------------------------------------------

/** Whether the WASM module loaded successfully. */
let wasmAvailable = true;
/** Captures the init error message (if any) for diagnostics. */
let wasmInitError: string | undefined;

// Instantiate WASM once at import time (per-isolate, not per-request).
// Workers-native import gives us a WebAssembly.Module directly.
// initSync() instantiates it and sets the internal `wasm` variable that
// all wrapper functions in firecrawl_wasm.js reference.
try {
  initSync({ module: wasmModule });
  // Sanity-check: verify a trivial call works
  extract_links("");
  console.log("[wasm] initialized successfully");
} catch (e) {
  wasmAvailable = false;
  wasmInitError = e instanceof Error ? e.message : String(e);
  console.error("[wasm] init failed:", wasmInitError);
}

/** Returns `true` when the WASM backend is loaded and functional. */
export function isAvailable(): boolean {
  return wasmAvailable;
}

/** Returns the init error message if WASM failed to load, or undefined. */
export function getInitError(): string | undefined {
  return wasmInitError;
}

/** Returns diagnostic info about the WASM module import resolution. */
export function getWasmDiagnostics(): Record<string, unknown> {
  return {
    typeof_wasmModule: typeof wasmModule,
    is_wasm_module:
      typeof WebAssembly !== "undefined" &&
      wasmModule instanceof WebAssembly.Module,
    wasmAvailable,
    wasmInitError,
  };
}

export function extractLinks(html: string | null): { links: string[] } {
  const result: string[] = JSON.parse(extract_links(html ?? ""));
  return { links: result };
}

export function extractBaseHref(
  html: string,
  baseUrl: string,
): { baseHref: string } {
  const result: string = JSON.parse(extract_base_href(html, baseUrl));
  return { baseHref: result };
}

export function extractMetadata(html: string | null): { metadata: unknown } {
  const result = JSON.parse(extract_metadata(html ?? ""));
  return { metadata: result };
}

export function transformHtml(params: TransformHtmlParams): { html: string } {
  const opts = {
    html: params.html,
    url: params.url,
    include_tags: params.include_tags ?? [],
    exclude_tags: params.exclude_tags ?? [],
    only_main_content: params.only_main_content ?? false,
    omce_signatures: params.omce_signatures ?? null,
  };
  const result: string = JSON.parse(transform_html(JSON.stringify(opts)));
  return { html: result };
}

export function getInnerJson(html: string): { content: string } {
  const result: string = JSON.parse(get_inner_json(html));
  return { content: result };
}

export function extractAttributes(
  html: string,
  options: { selectors: ExtractAttributesOption[] },
): { results: unknown } {
  const results = JSON.parse(
    extract_attributes(html, JSON.stringify({ selectors: options.selectors })),
  );
  return { results };
}

export function extractImages(
  html: string,
  baseUrl: string,
): { images: string[] } {
  const result: string[] = JSON.parse(extract_images(html, baseUrl));
  return { images: result };
}

export function extractAssets(
  html: string,
  baseUrl: string,
  formats?: string[],
): ExtractedAssetsResult {
  const result = JSON.parse(
    extract_assets(html, baseUrl, JSON.stringify(formats ?? ["assets"])),
  );
  return result as ExtractedAssetsResult;
}

export function postProcessMarkdown(
  markdown: string,
  _options?: { baseUrl?: string; citations?: boolean },
): { markdown: string } {
  // Note: baseUrl/citations options are handled by the server crate's postprocess,
  // not firecrawl_rs. The WASM module only does the core post-processing (link escaping,
  // skip-to-content removal). For citations support, fall through to container.
  const result: string = JSON.parse(post_process_markdown(markdown));
  return { markdown: result };
}

export function filterLinks(params: FilterLinksParams): FilterLinksResult {
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
  const result = JSON.parse(filter_links(JSON.stringify(body)));
  // Map snake_case response to camelCase
  return {
    links: result.links,
    denialReasons: result.denial_reasons ?? {},
  };
}

export function filterUrl(params: {
  href: string;
  url: string;
  baseUrl: string;
  excludes?: string[];
  ignoreRobotsTxt?: boolean;
  robotsTxt?: string;
  allowExternalContentLinks?: boolean;
  allowSubdomains?: boolean;
}): { allowed: boolean; url: string | null; denialReason: string | null } {
  const body = {
    href: params.href,
    url: params.url,
    base_url: params.baseUrl,
    excludes: params.excludes ?? [],
    ignore_robots_txt: params.ignoreRobotsTxt ?? false,
    robots_txt: params.robotsTxt ?? "",
    allow_external_content_links: params.allowExternalContentLinks ?? false,
    allow_subdomains: params.allowSubdomains ?? false,
  };
  const result = JSON.parse(filter_url(JSON.stringify(body)));
  return {
    allowed: result.allowed,
    url: result.url ?? null,
    denialReason: result.denial_reason ?? null,
  };
}

export function parseSitemap(xml: string): {
  urls: string[];
  sitemapUrls: string[];
} {
  const result = JSON.parse(process_sitemap(xml));
  // Map from the Rust SitemapProcessingResult to the expected format
  const urls: string[] = [];
  const sitemapUrls: string[] = [];
  for (const instruction of result.instructions ?? []) {
    if (instruction.action === "process") {
      urls.push(...(instruction.urls ?? []));
    } else if (instruction.action === "recurse") {
      sitemapUrls.push(...(instruction.urls ?? []));
    }
  }
  return { urls, sitemapUrls };
}

export function parseSitemapXml(xml: string): unknown {
  return JSON.parse(parse_sitemap_xml(xml));
}

export function convertDocumentToHtml(params: {
  data: Uint8Array;
  url?: string;
  contentType?: string | null;
}): { html: string } {
  const docType = detectDocType(params.url, params.contentType);
  if (!docType) {
    throw new Error(
      `WASM: unable to detect document type from url=${params.url} contentType=${params.contentType}`,
    );
  }

  // Encode to base64
  const bytes = new Uint8Array(params.data);
  let binary = "";
  const chunk = 8192;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  const dataBase64 = btoa(binary);

  const result: string = JSON.parse(convert_document(dataBase64, docType));
  return { html: result };
}

export function computeEngpickerVerdict(params: {
  results: unknown[];
  similarityThreshold: number;
  successRateThreshold: number;
  cdpFailureThreshold: number;
}): unknown {
  const body = {
    results: params.results,
    similarity_threshold: params.similarityThreshold,
    success_rate_threshold: params.successRateThreshold,
    cdp_failure_threshold: params.cdpFailureThreshold,
  };
  return JSON.parse(compute_engpicker_verdict(JSON.stringify(body)));
}
