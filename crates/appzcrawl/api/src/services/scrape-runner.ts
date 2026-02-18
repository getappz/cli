/**
 * Scrape runner orchestrator.
 * Coordinates: fetch URL → transform HTML → extract metadata & links → format processing.
 * Delegates to scrape-fetcher (fetch), scrape-format-processor (assets/markdown), and cache layer.
 */

import type { ScrapeFormat } from "../contracts/scrape";
import { BRANDING_SCRIPT } from "../lib/branding/branding-script.inject";
import { brandingTransformer } from "../lib/branding/transformer";
import type { BrandingScriptReturn } from "../lib/branding/types";
import { getFromCache, putToCache } from "../lib/cache";
import { logger } from "../lib/logger";
import { resolveLinks } from "../lib/resolveLinks";
import type { AppEnv } from "../types";
import {
  extractBaseHref,
  extractLinks,
  extractMetadata,
  transformHtml,
} from "./html-processor";
import { extractWithLlm } from "./llm-extract";
import {
  captureScreenshotAndUpload,
  fetchContent,
  isDocumentUrl,
  isPdfUrl,
  resolveEngine,
} from "./scrape-fetcher";
import { processFormats } from "./scrape-format-processor";

// ---------------------------------------------------------------------------
// Types (exported for consumers)
// ---------------------------------------------------------------------------

export interface ScrapeRunnerOptions {
  /** Only main content (passed to transformHtml). */
  onlyMainContent?: boolean;
  /** Use fire-engine for fetch when FIRE_ENGINE_URL is set. Falls back to Worker fetch on error. */
  useFireEngine?: boolean;
  /** Fetch engine for web URLs: native (default), cloudflare, or auto. Documents always use native. */
  engine?: "native" | "cloudflare" | "auto";
  /** Output formats to produce. */
  formats?: ScrapeFormat[];
  /** Tags to include. */
  includeTags?: string[];
  /** Tags to exclude. */
  excludeTags?: string[];
  /** Cache max age in ms. 0 = always fresh. */
  maxAge?: number;
  /** Store result in cache after scrape. Default true. */
  storeInCache?: boolean;
  /** Zero data retention — do not store in cache. */
  zeroDataRetention?: boolean;
  /** Custom headers — bypasses cache (request-specific). */
  headers?: Record<string, string>;
  /** Convert links to citations. Default: false. */
  citations?: boolean;
  /** Emulate mobile viewport for screenshots. Default: false. */
  mobile?: boolean;
  /** Remove base64 images from markdown. Default: true. */
  removeBase64Images?: boolean;
  /** Block ads when capturing screenshots. Default: true. */
  blockAds?: boolean;
  /** Skip TLS cert verification (fire-engine only). Default: true. */
  skipTlsVerification?: boolean;
  /** Timeout in ms. Default: 30000, max 300000. */
  timeout?: number;
  /** Base URL for screenshot links. Required for screenshot format. */
  screenshotBaseUrl?: string;
  /** Screenshot options from request. */
  screenshotOptions?: {
    fullPage?: boolean;
    viewport?: { width: number; height: number };
    quality?: number;
  };
  /** Wait before screenshot capture (ms). */
  waitFor?: number;
  /** JSON (LLM extraction) options when formats includes "json". */
  jsonOptions?: {
    prompt?: string;
    schema?: Record<string, unknown>;
  };
}

/** Branding profile (Firecrawl-compatible). */
export type BrandingProfile = Record<string, unknown>;

/** Canonical document produced by the scrape runner. */
export interface ScrapeRunnerDocument {
  url: string;
  rawHtml: string;
  html: string;
  markdown: string;
  metadata: Record<string, unknown>;
  links: string[];
  images?: string[];
  assets?: string[];
  /** Native JSON from Sarvam PDF (for json format response). */
  documentJson?: unknown;
  statusCode?: number;
  branding?: BrandingProfile;
  brandingError?: string;
  screenshot?: string;
}

export interface ScrapeRunnerResult {
  success: true;
  document: ScrapeRunnerDocument;
}

export interface ScrapeRunnerError {
  success: false;
  url: string;
  error: string;
  statusCode?: number;
}

export type ScrapeRunnerOutput = ScrapeRunnerResult | ScrapeRunnerError;

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

function shouldUseCache(
  options: ScrapeRunnerOptions,
  env: AppEnv["Bindings"],
): boolean {
  if (options.maxAge === undefined || options.maxAge <= 0) return false;
  if (!env.DB || !env.BUCKET) return false;
  const formats = options.formats ?? ["markdown"];
  if (formats.includes("changeTracking")) return false;
  if (options.headers && Object.keys(options.headers).length > 0) return false;
  return true;
}

function shouldStoreInCache(
  options: ScrapeRunnerOptions,
  env: AppEnv["Bindings"],
  statusCode: number,
  document: ScrapeRunnerDocument,
): boolean {
  if (options.storeInCache === false) return false;
  if (options.zeroDataRetention === true) return false;
  if (statusCode !== 200) return false;
  if (!env.DB || !env.BUCKET) return false;
  const formats = options.formats ?? ["markdown"];
  if (formats.includes("changeTracking")) return false;
  if (options.headers && Object.keys(options.headers).length > 0) return false;
  if (!document.html && !document.rawHtml) return false;
  return true;
}

// ---------------------------------------------------------------------------
// Cache-derived document
// ---------------------------------------------------------------------------

/** Derive full document from cached HTML. */
async function deriveDocumentFromHtml(
  env: AppEnv["Bindings"],
  params: {
    html: string;
    rawHtml?: string;
    url: string;
    statusCode?: number;
    formats: ScrapeFormat[];
    citations: boolean;
    removeBase64Images: boolean;
    cachedAt: string;
    /** When present (e.g. from Sarvam PDF cache), use instead of deriving. */
    markdown?: string;
    documentJson?: unknown;
  },
): Promise<ScrapeRunnerDocument> {
  const {
    html,
    rawHtml: htmlForAssets,
    url,
    statusCode = 200,
    formats,
    citations,
    removeBase64Images: shouldRemove,
    cachedAt,
    markdown: cachedMarkdown,
    documentJson: cachedDocumentJson,
  } = params;

  const [metadataResult, linksResult, baseHrefResult] = await Promise.all([
    extractMetadata(env, html),
    extractLinks(env, html),
    extractBaseHref(env, html, url),
  ]);

  const metadata = (metadataResult.metadata as Record<string, unknown>) ?? {};
  const baseHref = baseHrefResult?.baseHref ?? "";
  const links = resolveLinks(linksResult.links ?? [], url, baseHref);

  const formatResult = await processFormats(
    env,
    html,
    htmlForAssets || html,
    url,
    formats,
    citations,
    shouldRemove,
  );

  return {
    url,
    rawHtml: html,
    html,
    markdown: cachedMarkdown ?? formatResult.markdown,
    metadata: { ...metadata, statusCode, sourceURL: url, cachedAt },
    links,
    ...(formatResult.wantsImages ? { images: formatResult.images } : {}),
    ...(formatResult.wantsAnyAssetExtraction
      ? { assets: formatResult.assets }
      : {}),
    ...(cachedDocumentJson !== undefined
      ? { documentJson: cachedDocumentJson }
      : {}),
    statusCode,
  };
}

// ---------------------------------------------------------------------------
// Main scrape orchestrator
// ---------------------------------------------------------------------------

export async function runScrapeUrl(
  env: AppEnv["Bindings"],
  url: string,
  options: ScrapeRunnerOptions = {},
): Promise<ScrapeRunnerOutput> {
  const t0 = Date.now();
  const {
    onlyMainContent = true,
    useFireEngine = false,
    formats = ["markdown"],
    includeTags = [],
    excludeTags = [],
    maxAge = 0,
    citations = false,
    mobile = false,
    removeBase64Images = true,
    blockAds = true,
    skipTlsVerification = true,
    timeout = 30000,
    screenshotBaseUrl,
    screenshotOptions,
    jsonOptions,
  } = options;

  const effectiveTimeout = Math.min(300_000, Math.max(1000, timeout));
  const wantsScreenshot =
    formats.includes("screenshot") || formats.includes("screenshot@fullPage");
  const wantsBranding = formats.includes("branding");

  // --- Cache lookup ---
  const useCache = shouldUseCache(options, env);
  if (useCache) {
    const cached = await lookupCache(env, url, {
      onlyMainContent,
      includeTags,
      excludeTags,
      mobile,
      maxAge,
    });
    if (cached) {
      let doc = await deriveDocumentFromHtml(env, {
        html: cached.document.html ?? cached.document.rawHtml ?? "",
        rawHtml: cached.document.rawHtml,
        url: cached.document.url,
        statusCode: cached.document.statusCode,
        formats,
        citations,
        removeBase64Images,
        cachedAt: cached.cachedAt.toISOString(),
        markdown: cached.document.markdown,
        documentJson: cached.document.documentJson,
      });
      if (
        formats.includes("json") &&
        env.AI &&
        !options.zeroDataRetention &&
        (doc.documentJson === undefined || doc.documentJson === null) &&
        doc.markdown?.trim()
      ) {
        try {
          const extracted = await extractWithLlm(
            env.AI as Parameters<typeof extractWithLlm>[0],
            doc.markdown,
            {
              prompt: jsonOptions?.prompt,
              schema: jsonOptions?.schema,
            },
          );
          doc = { ...doc, documentJson: extracted.data };
        } catch (e) {
          logger.warn("[scrape] llm-extract failed (cache path)", {
            url,
            error: e instanceof Error ? e.message : String(e),
          });
        }
      }
      if (wantsScreenshot && screenshotBaseUrl) {
        const screenshotUrl = await captureScreenshotAndUpload(
          env,
          cached.document.url,
          {
            formats,
            screenshotOptions,
            screenshotBaseUrl,
            mobile,
            blockAds,
            timeout: effectiveTimeout,
            waitFor: options.waitFor ?? 0,
          },
        );
        if (screenshotUrl) doc = { ...doc, screenshot: screenshotUrl };
      }
      logger.info("[scrape] cache HIT", { url, ms: Date.now() - t0 });
      return { success: true, document: doc };
    }
  }

  // --- Fetch content ---
  const isDoc = isDocumentUrl(url) || isPdfUrl(url);
  const resolvedEngine = resolveEngine(env, options);

  // Start parallel tasks (screenshot + branding) for non-document URLs with native engine
  const screenshotPromise =
    wantsScreenshot &&
    screenshotBaseUrl &&
    env.BROWSER_SERVICE &&
    !isDoc &&
    resolvedEngine === "native"
      ? captureScreenshotAndUpload(env, url, {
          formats,
          screenshotOptions,
          screenshotBaseUrl,
          mobile,
          blockAds,
          timeout: effectiveTimeout,
          waitFor: options.waitFor ?? 0,
        })
      : null;

  const brandingPromise =
    wantsBranding && env.BROWSER_SERVICE && !isDoc
      ? env.BROWSER_SERVICE.extractBranding({
          url,
          script: BRANDING_SCRIPT,
          timeout: Math.min(effectiveTimeout, 30_000),
        })
      : null;

  const fetchResult = await fetchContent(env, url, {
    resolvedEngine,
    useFireEngine,
    effectiveTimeout,
    skipTlsVerification,
    mobile,
    wantsScreenshot,
    screenshotBaseUrl,
    screenshotOptions,
    formats,
  });

  if (!fetchResult.success) {
    return {
      success: false,
      url,
      error: fetchResult.error,
      statusCode: undefined,
    };
  }

  const {
    rawHtml,
    statusCode,
    screenshotUrl: screenshotFromFetch,
    documentContentType,
    documentImages,
    documentMarkdown,
    documentJson,
  } = fetchResult;

  // Fallback screenshot for Cloudflare engine when snapshot didn't capture one
  let screenshotFromCloudflare = screenshotFromFetch;
  if (
    resolvedEngine === "cloudflare" &&
    !screenshotFromCloudflare &&
    wantsScreenshot &&
    screenshotBaseUrl &&
    env.BROWSER_SERVICE
  ) {
    screenshotFromCloudflare = await captureScreenshotAndUpload(env, url, {
      formats,
      screenshotOptions,
      screenshotBaseUrl,
      mobile,
      blockAds,
      timeout: effectiveTimeout,
      waitFor: options.waitFor ?? 0,
    });
  }

  // --- Transform + extract ---
  try {
    const [transformed, metadataResult, linksResult, baseHrefResult] =
      await Promise.all([
        transformHtml(env, {
          html: rawHtml,
          url,
          only_main_content: onlyMainContent,
          include_tags: includeTags.length > 0 ? includeTags : undefined,
          exclude_tags: excludeTags.length > 0 ? excludeTags : undefined,
        }),
        extractMetadata(env, rawHtml),
        extractLinks(env, rawHtml),
        extractBaseHref(env, rawHtml, url),
      ]);

    const rawHtmlOut = transformed.html;
    const metadata = (metadataResult.metadata as Record<string, unknown>) ?? {};
    const baseHref = baseHrefResult?.baseHref ?? "";
    const links = resolveLinks(linksResult.links ?? [], url, baseHref);

    // --- Format processing (DRY via shared module) ---
    const formatResult = await processFormats(
      env,
      rawHtmlOut,
      rawHtml,
      url,
      formats,
      citations,
      removeBase64Images,
    );

    // --- Branding ---
    let branding: BrandingProfile | undefined;
    let brandingError: string | undefined;
    if (brandingPromise) {
      const brandingResult = await brandingPromise;
      if (brandingResult.success) {
        try {
          branding = await brandingTransformer({
            url,
            html: rawHtmlOut,
            rawBranding:
              brandingResult.rawBranding as unknown as BrandingScriptReturn,
            aiBinding: env.AI as
              | { run: (m: string, o: object) => Promise<unknown> }
              | undefined,
          });
        } catch (e) {
          brandingError = e instanceof Error ? e.message : String(e);
        }
      } else {
        brandingError = brandingResult.error;
      }
    }

    // --- Screenshot ---
    let screenshot: string | undefined = screenshotFromCloudflare;
    if (!screenshot && screenshotPromise) {
      screenshot = await screenshotPromise;
    }

    // --- LLM extraction (json format) ---
    let finalDocumentJson = documentJson;
    if (
      formats.includes("json") &&
      env.AI &&
      !options.zeroDataRetention &&
      (finalDocumentJson === undefined || finalDocumentJson === null)
    ) {
      const markdownForExtract =
        documentMarkdown ?? formatResult.markdown ?? "";
      if (markdownForExtract.trim().length > 0) {
        try {
          const extracted = await extractWithLlm(
            env.AI as Parameters<typeof extractWithLlm>[0],
            markdownForExtract,
            {
              prompt: jsonOptions?.prompt,
              schema: jsonOptions?.schema,
            },
          );
          finalDocumentJson = extracted.data;
          if (extracted.warning) {
            logger.warn("[scrape] llm-extract warning", {
              url,
              warning: extracted.warning,
            });
          }
        } catch (e) {
          logger.warn("[scrape] llm-extract failed", {
            url,
            error: e instanceof Error ? e.message : String(e),
          });
        }
      }
    }

    // --- Build document ---
    const document: ScrapeRunnerDocument = {
      url,
      rawHtml: rawHtmlOut,
      html: rawHtmlOut,
      markdown: documentMarkdown ?? formatResult.markdown,
      metadata: {
        ...metadata,
        statusCode,
        sourceURL: url,
        ...(documentContentType ? { contentType: documentContentType } : {}),
      },
      links,
      ...(formatResult.wantsImages
        ? {
            images:
              documentImages && documentImages.length > 0
                ? documentImages
                : formatResult.images,
          }
        : {}),
      ...(formatResult.wantsAnyAssetExtraction
        ? { assets: formatResult.assets }
        : {}),
      ...(finalDocumentJson !== undefined
        ? { documentJson: finalDocumentJson }
        : {}),
      ...(screenshot ? { screenshot } : {}),
      statusCode,
      branding,
      brandingError,
    };

    // --- Cache store ---
    await storeInCacheIfNeeded(env, options, maxAge, statusCode, document, {
      url,
      onlyMainContent,
      includeTags,
      excludeTags,
      mobile,
      formats,
      rawHtmlOut,
      rawHtml,
      wantsAnyAssetExtraction: formatResult.wantsAnyAssetExtraction,
    });

    logger.info("[scrape] complete", { url, ms: Date.now() - t0 });
    return { success: true, document };
  } catch (e) {
    return {
      success: false,
      url,
      error: e instanceof Error ? e.message : "Native processing failed",
      statusCode,
    };
  }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async function lookupCache(
  env: AppEnv["Bindings"],
  url: string,
  params: {
    onlyMainContent: boolean;
    includeTags: string[];
    excludeTags: string[];
    mobile: boolean;
    maxAge: number;
  },
) {
  try {
    return await getFromCache(
      { DB: env.DB, BUCKET: env.BUCKET },
      { url, ...params },
    );
  } catch (e) {
    logger.warn("[scrape] cache lookup failed", {
      error: e instanceof Error ? e.message : String(e),
    });
    return null;
  }
}

async function storeInCacheIfNeeded(
  env: AppEnv["Bindings"],
  options: ScrapeRunnerOptions,
  maxAge: number,
  statusCode: number,
  document: ScrapeRunnerDocument,
  params: {
    url: string;
    onlyMainContent: boolean;
    includeTags: string[];
    excludeTags: string[];
    mobile: boolean;
    formats: ScrapeFormat[];
    rawHtmlOut: string;
    rawHtml: string;
    wantsAnyAssetExtraction: boolean;
  },
) {
  if (!shouldStoreInCache(options, env, statusCode, document) || maxAge <= 0) {
    return;
  }

  try {
    await putToCache(
      { DB: env.DB, BUCKET: env.BUCKET },
      {
        url: params.url,
        onlyMainContent: params.onlyMainContent,
        includeTags: params.includeTags,
        excludeTags: params.excludeTags,
        mobile: params.mobile,
        maxAge,
        formats: params.formats,
      },
      {
        url: params.url,
        html: params.rawHtmlOut,
        ...(params.wantsAnyAssetExtraction ? { rawHtml: params.rawHtml } : {}),
        statusCode: document.statusCode ?? 200,
        ...(document.markdown ? { markdown: document.markdown } : {}),
        ...(document.documentJson !== undefined
          ? { documentJson: document.documentJson }
          : {}),
      },
      params.url,
    );
  } catch (e) {
    logger.warn("[scrape] cache store failed", {
      error: e instanceof Error ? e.message : String(e),
    });
  }
}
