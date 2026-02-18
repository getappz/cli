/**
 * Firecrawl-compatible scrape API contracts.
 * Request/response types aligned to Firecrawl v2 scrape endpoint for drop-in replacement.
 */

/** Output formats supported by Firecrawl scrape API. */
export const SCRAPE_FORMATS = [
  "markdown",
  "html",
  "rawHtml",
  "links",
  "images",
  "assets",
  "css",
  "js",
  "fonts",
  "videos",
  "audio",
  "iframes",
  "screenshot",
  "screenshot@fullPage",
  "json",
  "changeTracking",
  "branding",
] as const;

/** Asset format types that trigger extract-assets. "assets" = all, others = specific. */
export const ASSET_FORMAT_TYPES = [
  "assets",
  "images",
  "css",
  "js",
  "fonts",
  "videos",
  "audio",
  "iframes",
] as const;

export type AssetFormatType = (typeof ASSET_FORMAT_TYPES)[number];

/** Formats that map to native extract-assets (excludes "assets" which means "all"). */
export const ASSET_TYPE_FORMATS: AssetFormatType[] = [
  "images",
  "css",
  "js",
  "fonts",
  "videos",
  "audio",
  "iframes",
];

/**
 * Returns true if formats requests any asset extraction.
 */
export function wantsAnyAssets(formats: ScrapeFormat[]): boolean {
  return formats.some((f) => ASSET_FORMAT_TYPES.includes(f as AssetFormatType));
}

/**
 * Returns the asset formats to pass to native extract-assets.
 * - ["assets"] when user requested "assets" (all types)
 * - Otherwise the list of specific asset types requested (e.g. ["css", "js"])
 */
export function getAssetFormatsToExtract(formats: ScrapeFormat[]): string[] {
  if (formats.includes("assets")) return ["assets"];
  return formats.filter((f) =>
    ASSET_TYPE_FORMATS.includes(f as AssetFormatType),
  ) as string[];
}

export type ScrapeFormat = (typeof SCRAPE_FORMATS)[number];

/** Firecrawl-compatible scrape request body. */
export interface ScrapeRequestBody {
  /** Required: URL to scrape. */
  url: string;
  /** Output formats. Default: ["markdown"]. */
  formats?: ScrapeFormat[];
  /** Only main content. Default: true. */
  onlyMainContent?: boolean;
  /** Tags to include. */
  includeTags?: string[];
  /** Tags to exclude. */
  excludeTags?: string[];
  /** Cache max age in ms. */
  maxAge?: number;
  /** Custom headers for request. */
  headers?: Record<string, string>;
  /** Wait delay in ms. */
  waitFor?: number;
  /** Mobile emulation. */
  mobile?: boolean;
  /** Skip TLS verification. */
  skipTlsVerification?: boolean;
  /** Timeout in ms. */
  timeout?: number;
  /** Browser actions (partially supported). */
  actions?: unknown[];
  /** Location settings. */
  location?: { country?: string; languages?: string[] };
  /** Remove base64 images. */
  removeBase64Images?: boolean;
  /** Block ads. */
  blockAds?: boolean;
  /** Proxy mode. */
  proxy?: "basic" | "enhanced" | "auto";
  /** Store in cache. */
  storeInCache?: boolean;
  /** Zero data retention. */
  zeroDataRetention?: boolean;
  /** Appzcrawl: use fire-engine for fetch. */
  useFireEngine?: boolean;
  /** Fetch engine for web URLs: native (default), cloudflare, or auto. */
  engine?: "native" | "cloudflare" | "auto";
  /** Appzcrawl: convert links to citations (text⟨1⟩ + References section). */
  citations?: boolean;
  /** V2 screenshot options (when formats includes screenshot). */
  screenshotOptions?: {
    fullPage?: boolean;
    viewport?: { width: number; height: number };
    quality?: number;
  };
  /** JSON (LLM extraction) options when formats includes "json". Prompt + schema drive structured output. */
  jsonOptions?: {
    prompt?: string;
    schema?: Record<string, unknown>;
  };
}

/** Resolved screenshot options from formats + screenshotOptions. */
export interface ScreenshotOptionsResolved {
  fullPage: boolean;
  viewport?: { width: number; height: number };
  quality?: number;
}

/** Resolve screenshot options from request. */
export function resolveScreenshotOptions(
  formats: ScrapeFormat[],
  screenshotOptions?: ScrapeRequestBody["screenshotOptions"],
): ScreenshotOptionsResolved | null {
  const hasScreenshot =
    formats.includes("screenshot") || formats.includes("screenshot@fullPage");
  if (!hasScreenshot) return null;

  return {
    fullPage:
      screenshotOptions?.fullPage ?? formats.includes("screenshot@fullPage"),
    viewport: screenshotOptions?.viewport,
    quality: screenshotOptions?.quality,
  };
}

/** Default request values matching Firecrawl. */
export const SCRAPE_DEFAULTS: Required<
  Pick<
    ScrapeRequestBody,
    | "formats"
    | "onlyMainContent"
    | "includeTags"
    | "excludeTags"
    | "maxAge"
    | "waitFor"
    | "mobile"
    | "skipTlsVerification"
    | "timeout"
    | "removeBase64Images"
    | "blockAds"
    | "proxy"
    | "storeInCache"
    | "zeroDataRetention"
  >
> = {
  formats: ["markdown"],
  onlyMainContent: true,
  includeTags: [],
  excludeTags: [],
  /** 2 days — enables cache by default so branding comes from cache (Firecrawl uses 4h–2d). */
  maxAge: 2 * 24 * 60 * 60 * 1000,
  waitFor: 0,
  mobile: false,
  skipTlsVerification: true,
  timeout: 30000,
  removeBase64Images: true,
  blockAds: true,
  proxy: "auto",
  storeInCache: true,
  zeroDataRetention: false,
};

/** Firecrawl-compatible scrape response metadata. */
export interface ScrapeResponseMetadata {
  title?: string;
  description?: string;
  language?: string | null;
  sourceURL?: string;
  statusCode?: number;
  error?: string | null;
  [key: string]: unknown;
}

/** Firecrawl-compatible scrape response data. */
export interface ScrapeResponseData {
  markdown?: string;
  html?: string | null;
  rawHtml?: string | null;
  screenshot?: string | null;
  links?: string[];
  images?: string[];
  assets?: string[];
  actions?: {
    screenshots?: string[];
    scrapes?: Array<{ url: string; html: string }>;
    javascriptReturns?: Array<{ type: string; value: unknown }>;
    pdfs?: string[];
  } | null;
  metadata?: ScrapeResponseMetadata;
  llm_extraction?: unknown | null;
  warning?: string | null;
  changeTracking?: {
    previousScrapeAt?: string | null;
    changeStatus?: "new" | "same" | "changed" | "removed";
    visibility?: "visible" | "hidden";
    diff?: string | null;
    json?: Record<string, unknown> | null;
  } | null;
  branding?: Record<string, unknown> | null;
}

/** Firecrawl-compatible response envelope (cacheState, creditsUsed, etc.). */
export interface ScrapeResponseEnvelope {
  cacheState: "hit" | "miss";
  cachedAt: string;
  creditsUsed: number;
  concurrencyLimited: boolean;
}

/** Build response envelope for cache miss (e.g. crawl, extract, agent, search, map). */
export function responseEnvelope(creditsUsed = 1): ScrapeResponseEnvelope {
  return {
    cacheState: "miss",
    cachedAt: new Date().toISOString(),
    creditsUsed,
    concurrencyLimited: false,
  };
}

/** Firecrawl-compatible scrape success response. */
export interface ScrapeSuccessResponse {
  success: true;
  data: ScrapeResponseData;
  cacheState: "hit" | "miss";
  cachedAt: string;
  creditsUsed: number;
  concurrencyLimited: boolean;
}

/** Firecrawl-compatible scrape error response. */
export interface ScrapeErrorResponse {
  success: false;
  error: string;
  url?: string;
}

export type ScrapeResponse = ScrapeSuccessResponse | ScrapeErrorResponse;

/** Parse and normalize request body with Firecrawl defaults. Accepts unknown keys without failing. */
export function parseScrapeRequestBody(
  body: unknown,
): { ok: true; data: ScrapeRequestBody } | { ok: false; error: string } {
  if (body === null || typeof body !== "object") {
    return { ok: false, error: "Invalid JSON body; expected object" };
  }
  const raw = body as Record<string, unknown>;
  const url = raw.url;
  if (typeof url !== "string" || !url.trim()) {
    return { ok: false, error: "Missing or invalid url in body" };
  }

  const formats = raw.formats;
  const formatsArray = Array.isArray(formats)
    ? (formats as unknown[]).filter(
        (f): f is ScrapeFormat =>
          typeof f === "string" && SCRAPE_FORMATS.includes(f as ScrapeFormat),
      )
    : SCRAPE_DEFAULTS.formats;

  return {
    ok: true,
    data: {
      url: url.trim(),
      formats: formatsArray.length > 0 ? formatsArray : SCRAPE_DEFAULTS.formats,
      onlyMainContent:
        typeof raw.onlyMainContent === "boolean"
          ? raw.onlyMainContent
          : SCRAPE_DEFAULTS.onlyMainContent,
      includeTags: Array.isArray(raw.includeTags)
        ? (raw.includeTags as string[]).filter((t) => typeof t === "string")
        : SCRAPE_DEFAULTS.includeTags,
      excludeTags: Array.isArray(raw.excludeTags)
        ? (raw.excludeTags as string[]).filter((t) => typeof t === "string")
        : SCRAPE_DEFAULTS.excludeTags,
      maxAge:
        typeof raw.maxAge === "number" && raw.maxAge >= 0
          ? raw.maxAge
          : SCRAPE_DEFAULTS.maxAge,
      headers:
        raw.headers &&
        typeof raw.headers === "object" &&
        !Array.isArray(raw.headers)
          ? (raw.headers as Record<string, string>)
          : undefined,
      waitFor:
        typeof raw.waitFor === "number" && raw.waitFor >= 0
          ? raw.waitFor
          : SCRAPE_DEFAULTS.waitFor,
      mobile:
        typeof raw.mobile === "boolean" ? raw.mobile : SCRAPE_DEFAULTS.mobile,
      skipTlsVerification:
        typeof raw.skipTlsVerification === "boolean"
          ? raw.skipTlsVerification
          : SCRAPE_DEFAULTS.skipTlsVerification,
      timeout:
        typeof raw.timeout === "number" && raw.timeout > 0
          ? Math.min(raw.timeout, 300_000)
          : SCRAPE_DEFAULTS.timeout,
      actions: Array.isArray(raw.actions) ? raw.actions : undefined,
      location:
        raw.location && typeof raw.location === "object"
          ? (raw.location as { country?: string; languages?: string[] })
          : undefined,
      removeBase64Images:
        typeof raw.removeBase64Images === "boolean"
          ? raw.removeBase64Images
          : SCRAPE_DEFAULTS.removeBase64Images,
      blockAds:
        typeof raw.blockAds === "boolean"
          ? raw.blockAds
          : SCRAPE_DEFAULTS.blockAds,
      proxy:
        raw.proxy === "basic" ||
        raw.proxy === "enhanced" ||
        raw.proxy === "auto"
          ? raw.proxy
          : SCRAPE_DEFAULTS.proxy,
      storeInCache:
        typeof raw.storeInCache === "boolean"
          ? raw.storeInCache
          : SCRAPE_DEFAULTS.storeInCache,
      zeroDataRetention:
        typeof raw.zeroDataRetention === "boolean"
          ? raw.zeroDataRetention
          : SCRAPE_DEFAULTS.zeroDataRetention,
      useFireEngine: Boolean(raw.useFireEngine),
      engine:
        raw.engine === "native" ||
        raw.engine === "cloudflare" ||
        raw.engine === "auto"
          ? raw.engine
          : undefined,
      citations: typeof raw.citations === "boolean" ? raw.citations : false,
      screenshotOptions:
        raw.screenshotOptions &&
        typeof raw.screenshotOptions === "object" &&
        !Array.isArray(raw.screenshotOptions)
          ? (raw.screenshotOptions as ScrapeRequestBody["screenshotOptions"])
          : undefined,
      jsonOptions:
        raw.jsonOptions &&
        typeof raw.jsonOptions === "object" &&
        !Array.isArray(raw.jsonOptions)
          ? (() => {
              const jo = raw.jsonOptions as Record<string, unknown>;
              return {
                prompt: typeof jo.prompt === "string" ? jo.prompt : undefined,
                schema:
                  jo.schema && typeof jo.schema === "object"
                    ? (jo.schema as Record<string, unknown>)
                    : undefined,
              };
            })()
          : undefined,
    },
  };
}
