/**
 * URL fetching logic for the scrape pipeline.
 * Handles HTML fetch (Worker fetch, fire-engine, Cloudflare Browser Rendering)
 * and document fetch (docx, xlsx, etc.) with unified timeout/abort patterns.
 */

import {
  resolveScreenshotOptions,
  type ScrapeFormat,
} from "../contracts/scrape";
import { logger } from "../lib/logger";
import { uploadScreenshotToR2 } from "../lib/screenshot-upload";
import type { AppEnv } from "../types";
import {
  cloudflareFetchContent,
  cloudflareFetchSnapshot,
  isCloudflareBrowserEnabled,
} from "./cloudflare-browser-client";
import { fireEngineFetchHtml, isFireEngineEnabled } from "./fire-engine-client";
import { convertDocument, convertPdf } from "./html-processor";
import { parsePdfWithLlamaParse } from "./llamaparse-client";
import { parsePdfWithSarvam } from "./sarvam-client";
import type { ScrapeRunnerError, ScrapeRunnerOptions } from "./scrape-runner";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const USER_AGENT =
  "Mozilla/5.0 (compatible; Appzcrawl/1.0; +https://appz.dev)";

/** Document extensions supported by the document engine (docx, xlsx, odt, rtf, doc, xls). */
const DOCUMENT_EXTENSIONS = [
  ".docx",
  ".doc",
  ".odt",
  ".rtf",
  ".xlsx",
  ".xls",
] as const;

/** PDF extension supported by the PDF engine. Firecrawl-compatible. */
const PDF_EXTENSIONS = [".pdf"] as const;

const VALID_DOCUMENT_CONTENT_TYPES = [
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "application/vnd.ms-excel",
  "application/msword",
  "application/rtf",
  "text/rtf",
  "application/vnd.oasis.opendocument.text",
];

const VALID_PDF_CONTENT_TYPES = ["application/pdf", "application/x-pdf"];

// ---------------------------------------------------------------------------
// URL classification
// ---------------------------------------------------------------------------

export function isDocumentUrl(url: string): boolean {
  const lower = url.toLowerCase();
  return DOCUMENT_EXTENSIONS.some(
    (ext) => lower.endsWith(ext) || lower.includes(`${ext}/`),
  );
}

export function isPdfUrl(url: string): boolean {
  const lower = url.toLowerCase();
  return PDF_EXTENSIONS.some(
    (ext) => lower.endsWith(ext) || lower.includes(`${ext}/`),
  );
}

function isValidDocumentContentType(contentType: string | null): boolean {
  if (!contentType) return false;
  const ct = contentType.toLowerCase();
  return VALID_DOCUMENT_CONTENT_TYPES.some((t) => ct.includes(t));
}

function isValidPdfContentType(contentType: string | null): boolean {
  if (!contentType) return false;
  const ct = contentType.toLowerCase();
  return VALID_PDF_CONTENT_TYPES.some((t) => ct.includes(t));
}

/** Resolve fetch engine for web URLs. Documents and PDFs always use native. */
export function resolveEngine(
  env: AppEnv["Bindings"],
  options: Pick<ScrapeRunnerOptions, "engine">,
): "native" | "cloudflare" {
  const engine = options.engine;
  if (
    (engine === "cloudflare" || engine === "auto") &&
    isCloudflareBrowserEnabled(env)
  ) {
    return "cloudflare";
  }
  return "native";
}

// ---------------------------------------------------------------------------
// Shared fetch with timeout + abort
// ---------------------------------------------------------------------------

interface FetchWithTimeoutResult {
  res: Response;
}

async function fetchWithTimeout(
  url: string,
  timeoutMs: number,
): Promise<FetchWithTimeoutResult | ScrapeRunnerError> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  let res: Response;
  try {
    res = await fetch(url, {
      signal: controller.signal,
      headers: { "User-Agent": USER_AGENT },
      redirect: "follow",
    });
    clearTimeout(timer);
  } catch (e) {
    clearTimeout(timer);
    const isAbort = e instanceof Error && e.name === "AbortError";
    return {
      success: false,
      url,
      error: isAbort
        ? "Request timed out"
        : e instanceof Error
          ? e.message
          : "Fetch failed",
    };
  }

  return { res };
}

async function readErrorBody(res: Response): Promise<string> {
  try {
    return await res.text();
  } catch {
    return "";
  }
}

// ---------------------------------------------------------------------------
// HTML fetch (Worker fetch)
// ---------------------------------------------------------------------------

export async function fetchHtmlWithWorker(
  url: string,
  opts: { timeout?: number } = {},
): Promise<
  { success: true; rawHtml: string; statusCode: number } | ScrapeRunnerError
> {
  const result = await fetchWithTimeout(url, opts.timeout ?? 30_000);
  if ("success" in result) return result;

  const { res } = result;
  const statusCode = res.status;

  if (!res.ok) {
    const errorBody = await readErrorBody(res);
    return {
      success: false,
      url,
      error: errorBody || res.statusText || `HTTP ${statusCode}`,
      statusCode,
    };
  }

  const contentType = res.headers.get("content-type") ?? "";
  if (!contentType.toLowerCase().includes("text/html")) {
    return {
      success: false,
      url,
      error: `Unsupported content type: ${contentType}`,
      statusCode,
    };
  }

  try {
    const rawHtml = await res.text();
    return { success: true, rawHtml, statusCode };
  } catch (e) {
    return {
      success: false,
      url,
      error: e instanceof Error ? e.message : "Failed to read body",
      statusCode,
    };
  }
}

// ---------------------------------------------------------------------------
// Document fetch (Worker fetch for docx/xlsx/etc.)
// ---------------------------------------------------------------------------

export async function fetchDocumentWithWorker(
  url: string,
  opts: { timeout?: number } = {},
): Promise<
  | { success: true; data: Uint8Array; contentType: string; statusCode: number }
  | ScrapeRunnerError
> {
  const result = await fetchWithTimeout(url, opts.timeout ?? 30_000);
  if ("success" in result) return result;

  const { res } = result;
  const statusCode = res.status;

  if (!res.ok) {
    const errorBody = await readErrorBody(res);
    return {
      success: false,
      url,
      error: errorBody || res.statusText || `HTTP ${statusCode}`,
      statusCode,
    };
  }

  const contentType = res.headers.get("content-type") ?? "";
  if (!isValidDocumentContentType(contentType)) {
    return {
      success: false,
      url,
      error: `Unsupported document content type: ${contentType}`,
      statusCode,
    };
  }

  try {
    const data = await res.arrayBuffer();
    return {
      success: true,
      data: new Uint8Array(data),
      contentType,
      statusCode,
    };
  } catch (e) {
    return {
      success: false,
      url,
      error: e instanceof Error ? e.message : "Failed to read body",
      statusCode,
    };
  }
}

// ---------------------------------------------------------------------------
// PDF fetch (Worker fetch for .pdf URLs)
// ---------------------------------------------------------------------------

export async function fetchPdfWithWorker(
  url: string,
  opts: { timeout?: number } = {},
): Promise<
  | { success: true; data: Uint8Array; contentType: string; statusCode: number }
  | ScrapeRunnerError
> {
  const result = await fetchWithTimeout(url, opts.timeout ?? 30_000);
  if ("success" in result) return result;

  const { res } = result;
  const statusCode = res.status;

  if (!res.ok) {
    const errorBody = await readErrorBody(res);
    return {
      success: false,
      url,
      error: errorBody || res.statusText || `HTTP ${statusCode}`,
      statusCode,
    };
  }

  const contentType = res.headers.get("content-type") ?? "";
  if (!isValidPdfContentType(contentType)) {
    return {
      success: false,
      url,
      error: `Unsupported PDF content type: ${contentType}`,
      statusCode,
    };
  }

  try {
    const data = await res.arrayBuffer();
    return {
      success: true,
      data: new Uint8Array(data),
      contentType,
      statusCode,
    };
  } catch (e) {
    return {
      success: false,
      url,
      error: e instanceof Error ? e.message : "Failed to read body",
      statusCode,
    };
  }
}

// ---------------------------------------------------------------------------
// Screenshot capture + R2 upload
// ---------------------------------------------------------------------------

export async function captureScreenshotAndUpload(
  env: AppEnv["Bindings"],
  url: string,
  opts: {
    formats: ScrapeFormat[];
    screenshotOptions?: ScrapeRunnerOptions["screenshotOptions"];
    screenshotBaseUrl?: string;
    mobile?: boolean;
    blockAds?: boolean;
    timeout?: number;
    waitFor?: number;
  },
): Promise<string | undefined> {
  const {
    formats,
    screenshotOptions,
    screenshotBaseUrl,
    mobile = false,
    blockAds = true,
    timeout: screenshotTimeout = 30_000,
    waitFor = 0,
  } = opts;

  const resolved = resolveScreenshotOptions(formats, screenshotOptions);
  if (!resolved || !screenshotBaseUrl || !env.BROWSER_SERVICE) return undefined;

  const result = await env.BROWSER_SERVICE.captureScreenshotAndUpload({
    url,
    screenshotBaseUrl,
    options: {
      fullPage: resolved.fullPage,
      viewport: resolved.viewport,
      quality: resolved.quality,
      mobile,
      blockAds,
      waitFor,
    },
    timeout: Math.min(screenshotTimeout, 60_000),
  });

  if (!result.success) {
    logger.warn("[scrape] screenshot capture failed", {
      error: result.error,
    });
    return undefined;
  }

  return result.url;
}

// ---------------------------------------------------------------------------
// PDF parsing fallback (Sarvam or native)
// ---------------------------------------------------------------------------

async function tryPdfSarvamOrNative(
  env: AppEnv["Bindings"],
  pdfData: Uint8Array,
  effectiveTimeout: number,
  formats: ScrapeFormat[],
): Promise<{
  html: string;
  markdown?: string;
}> {
  const sarvamKey = env.SARVAM_API_KEY;
  if (sarvamKey && sarvamKey.trim().length > 0) {
    try {
      const requestedFormats = resolveSarvamFormats(formats);
      const sv = await parsePdfWithSarvam(sarvamKey, pdfData, {
        timeoutMs: Math.min(effectiveTimeout - 5000, 180_000),
        requestedFormats,
      });
      return {
        html: sv.html,
        ...(sv.markdown ? { markdown: sv.markdown } : {}),
      };
    } catch {
      // Fall through to native
    }
  }
  const convertResult = await convertPdf(env, { data: pdfData });
  return { html: convertResult.html };
}

/** Map scrape formats to Sarvam output formats. Sarvam only supports html and md (no json). */
function resolveSarvamFormats(formats: ScrapeFormat[]): ("html" | "md")[] {
  const out: ("html" | "md")[] = [];
  if (formats.includes("html") || formats.includes("rawHtml")) out.push("html");
  if (formats.includes("markdown")) out.push("md");
  if (out.length === 0) out.push("md");
  return out;
}

// ---------------------------------------------------------------------------
// Unified fetch orchestration (with engine fallback)
// ---------------------------------------------------------------------------

export type FetchResult =
  | {
      success: true;
      rawHtml: string;
      statusCode: number;
      screenshotUrl?: string;
      documentContentType?: string;
      /** From Sarvam PDF: native markdown (not derived from HTML). */
      documentMarkdown?: string;
      /** LLM-extracted JSON when available (PDF: not supported by Sarvam/LlamaParse). */
      documentJson?: unknown;
      /** From LlamaParse PDF: extracted image URLs. */
      documentImages?: string[];
    }
  | { success: false; error: string };

/** Fetch HTML or document content with engine fallback strategy. */
export async function fetchContent(
  env: AppEnv["Bindings"],
  url: string,
  options: {
    resolvedEngine: "native" | "cloudflare";
    useFireEngine: boolean;
    effectiveTimeout: number;
    skipTlsVerification: boolean;
    mobile: boolean;
    wantsScreenshot: boolean;
    screenshotBaseUrl?: string;
    screenshotOptions?: ScrapeRunnerOptions["screenshotOptions"];
    formats: ScrapeFormat[];
  },
): Promise<FetchResult> {
  const {
    resolvedEngine,
    useFireEngine,
    effectiveTimeout,
    skipTlsVerification,
    mobile,
    wantsScreenshot,
    screenshotBaseUrl,
    screenshotOptions,
    formats,
  } = options;

  // Document URLs always use native fetch
  if (isDocumentUrl(url)) {
    const docResult = await fetchDocumentWithWorker(url, {
      timeout: effectiveTimeout,
    });
    if (!docResult.success) return { success: false, error: docResult.error };

    const convertResult = await convertDocument(env, {
      data: docResult.data,
      url,
      contentType: docResult.contentType,
    });
    return {
      success: true,
      rawHtml: convertResult.html,
      statusCode: docResult.statusCode,
      documentContentType: docResult.contentType,
    };
  }

  // PDF URLs: LlamaParse (when LLAMAPARSE_API_KEY present) > Sarvam > native.
  if (isPdfUrl(url)) {
    const pdfResult = await fetchPdfWithWorker(url, {
      timeout: effectiveTimeout,
    });
    if (!pdfResult.success) return { success: false, error: pdfResult.error };

    const llamaKey = env.LLAMAPARSE_API_KEY;
    const sarvamKey = env.SARVAM_API_KEY;
    const wantsImages = formats.includes("images");
    // PDF parsing: LlamaParse has priority when key present; Sarvam provides native markdown; json via LLM extraction.
    let html: string;
    let documentImages: string[] | undefined;
    let documentMarkdown: string | undefined;
    let documentJson: unknown;
    if (llamaKey && llamaKey.trim().length > 0) {
      try {
        const lp = await parsePdfWithLlamaParse(llamaKey, pdfResult.data, {
          timeoutMs: Math.min(effectiveTimeout - 5000, 120_000),
          wantsImages,
        });
        html = lp.html;
        documentImages = lp.images;
      } catch (e) {
        logger.warn(
          "[scrape] LlamaParse failed, falling back to Sarvam/native",
          {
            error: e instanceof Error ? e.message : String(e),
            url,
          },
        );
        const fallback = await tryPdfSarvamOrNative(
          env,
          pdfResult.data,
          effectiveTimeout,
          formats,
        );
        html = fallback.html;
        documentMarkdown = fallback.markdown;
      }
    } else if (sarvamKey && sarvamKey.trim().length > 0) {
      try {
        const sv = await parsePdfWithSarvam(sarvamKey, pdfResult.data, {
          timeoutMs: Math.min(effectiveTimeout - 5000, 180_000),
          requestedFormats: resolveSarvamFormats(formats),
        });
        html = sv.html;
        documentMarkdown = sv.markdown;
      } catch (e) {
        logger.warn(
          "[scrape] Sarvam failed, falling back to native pdf-extract",
          {
            error: e instanceof Error ? e.message : String(e),
            url,
          },
        );
        const convertResult = await convertPdf(env, { data: pdfResult.data });
        html = convertResult.html;
      }
    } else {
      const convertResult = await convertPdf(env, { data: pdfResult.data });
      html = convertResult.html;
    }
    return {
      success: true,
      rawHtml: html,
      statusCode: pdfResult.statusCode,
      documentContentType: pdfResult.contentType,
      ...(documentImages?.length ? { documentImages } : {}),
      ...(documentMarkdown ? { documentMarkdown } : {}),
      ...(documentJson !== undefined ? { documentJson } : {}),
    };
  }

  // Web URL fetching with engine fallback

  const fireEngineUrl = env.FIRE_ENGINE_URL;
  const tryFireEngine =
    useFireEngine && isFireEngineEnabled(env) && fireEngineUrl;

  async function tryCloudflareFetch(): Promise<FetchResult> {
    if (!isCloudflareBrowserEnabled(env)) {
      return {
        success: false,
        error: "Cloudflare Browser Rendering not configured",
      };
    }
    const accountId = env.CLOUDFLARE_ACCOUNT_ID!;
    const apiToken = env.CLOUDFLARE_BROWSER_RENDERING_API_TOKEN!;
    const viewport = mobile
      ? { width: 390, height: 844 }
      : screenshotOptions?.viewport;
    const cfOptions = {
      waitUntil: "networkidle0" as const,
      viewport,
      timeout: effectiveTimeout,
    };

    if (wantsScreenshot && screenshotBaseUrl && env.BUCKET) {
      const snap = await cloudflareFetchSnapshot(accountId, apiToken, url, {
        ...cfOptions,
        fullPage:
          screenshotOptions?.fullPage ??
          formats.includes("screenshot@fullPage"),
        screenshotViewport: viewport,
      });
      if (!snap.success) return { success: false, error: snap.error };
      let screenshotUrl: string | undefined;
      try {
        const binary = Uint8Array.from(atob(snap.screenshotBase64), (c) =>
          c.charCodeAt(0),
        );
        const { filename } = await uploadScreenshotToR2(
          env.BUCKET,
          binary,
          "image/png",
        );
        screenshotUrl = `${screenshotBaseUrl}/${filename}`;
      } catch (e) {
        logger.warn("[scrape] Cloudflare screenshot upload failed", {
          error: e instanceof Error ? e.message : String(e),
        });
      }
      return {
        success: true,
        rawHtml: snap.html,
        statusCode: snap.statusCode,
        screenshotUrl,
      };
    }

    const content = await cloudflareFetchContent(
      accountId,
      apiToken,
      url,
      cfOptions,
    );
    if (!content.success) return { success: false, error: content.error };
    return {
      success: true,
      rawHtml: content.html,
      statusCode: content.statusCode,
    };
  }

  async function tryNativeFetch(): Promise<FetchResult> {
    if (tryFireEngine && fireEngineUrl) {
      const feResult = await fireEngineFetchHtml(fireEngineUrl, url, {
        timeout: effectiveTimeout,
        skipTlsVerification,
      });
      if (feResult.success) {
        return {
          success: true,
          rawHtml: feResult.html,
          statusCode: feResult.statusCode,
        };
      }
    }
    const fetchResult = await fetchHtmlWithWorker(url, {
      timeout: effectiveTimeout,
    });
    if (!fetchResult.success) {
      return { success: false, error: fetchResult.error };
    }
    return {
      success: true,
      rawHtml: fetchResult.rawHtml,
      statusCode: fetchResult.statusCode,
    };
  }

  let fetchResult: FetchResult;
  if (resolvedEngine === "cloudflare") {
    fetchResult = await tryCloudflareFetch();
    if (!fetchResult.success) {
      logger.info("[scrape] Cloudflare fetch failed, falling back to native", {
        error: fetchResult.error,
      });
      fetchResult = await tryNativeFetch();
    }
  } else {
    fetchResult = await tryNativeFetch();
    if (!fetchResult.success && isCloudflareBrowserEnabled(env)) {
      logger.info("[scrape] native fetch failed, falling back to Cloudflare", {
        error: fetchResult.error,
      });
      fetchResult = await tryCloudflareFetch();
    }
  }

  return fetchResult;
}
