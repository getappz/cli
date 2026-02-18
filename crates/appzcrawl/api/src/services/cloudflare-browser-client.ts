/**
 * Cloudflare Browser Rendering API client.
 *
 * Provides a CloudflareBrowserEngine class with methods for:
 * - content: Fetch rendered HTML
 * - snapshot: HTML + base64 screenshot
 * - markdown: Extract page as Markdown
 * - scrape: Extract elements by CSS selectors (text, html, attributes, dimensions)
 * - json: AI-powered structured extraction (prompt + optional JSON schema)
 *
 * API: https://developers.cloudflare.com/browser-rendering/
 *      https://developers.cloudflare.com/api/resources/browser_rendering/
 */

const CLOUDFLARE_API_BASE = "https://api.cloudflare.com/client/v4/accounts";

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

export interface CloudflareContentResult {
  success: true;
  html: string;
  statusCode: number;
}

export interface CloudflareSnapshotResult {
  success: true;
  html: string;
  statusCode: number;
  /** Base64-encoded PNG (or JPEG). Caller uploads to R2 and constructs URL. */
  screenshotBase64: string;
}

export interface CloudflareMarkdownResult {
  success: true;
  markdown: string;
  statusCode: number;
}

/** Single scraped element: text, html, attributes, dimensions */
export interface CloudflareScrapedElement {
  text?: string;
  html?: string;
  attributes?: Array<{ name: string; value: string }>;
  height?: number;
  width?: number;
  top?: number;
  left?: number;
}

/** Per-selector scrape result */
export interface CloudflareScrapeSelectorResult {
  selector: string;
  results: CloudflareScrapedElement[];
}

export interface CloudflareScrapeResult {
  success: true;
  data: CloudflareScrapeSelectorResult[];
  statusCode: number;
}

export interface CloudflareJsonResult {
  success: true;
  data: Record<string, unknown>;
  statusCode: number;
}

export interface CloudflareBrowserError {
  success: false;
  error: string;
}

export type CloudflareContentOutput =
  | CloudflareContentResult
  | CloudflareBrowserError;

export type CloudflareSnapshotOutput =
  | CloudflareSnapshotResult
  | CloudflareBrowserError;

export type CloudflareMarkdownOutput =
  | CloudflareMarkdownResult
  | CloudflareBrowserError;

export type CloudflareScrapeOutput =
  | CloudflareScrapeResult
  | CloudflareBrowserError;

export type CloudflareJsonOutput =
  | CloudflareJsonResult
  | CloudflareBrowserError;

// ---------------------------------------------------------------------------
// Options types
// ---------------------------------------------------------------------------

export type GotoWaitUntil =
  | "load"
  | "domcontentloaded"
  | "networkidle0"
  | "networkidle2";

/** Shared options for page load behavior (url/html input, gotoOptions, viewport). */
export interface CloudflarePageLoadOptions {
  waitUntil?: GotoWaitUntil;
  viewport?: { width: number; height: number };
  timeout?: number;
}

export interface CloudflareContentOptions extends CloudflarePageLoadOptions {}

export interface CloudflareSnapshotOptions extends CloudflarePageLoadOptions {
  fullPage?: boolean;
  screenshotViewport?: { width: number; height: number };
}

export interface CloudflareMarkdownOptions extends CloudflarePageLoadOptions {
  /** Regex patterns to reject requests (e.g. exclude CSS). */
  rejectRequestPattern?: string[];
}

export interface CloudflareScrapeElement {
  selector: string;
}

export interface CloudflareScrapeOptions extends CloudflarePageLoadOptions {
  elements: CloudflareScrapeElement[];
}

/** JSON schema for response_format (OpenAI-compatible). */
export interface CloudflareJsonSchema {
  type: "json_schema";
  schema: Record<string, unknown>;
}

/** Custom AI model (BYO API key). */
export interface CloudflareCustomAi {
  model: string;
  authorization: string;
}

export interface CloudflareJsonOptions extends CloudflarePageLoadOptions {
  /** Natural language prompt for extraction. */
  prompt?: string;
  /** JSON schema to structure output. Use with Workers AI; avoid with Anthropic. */
  responseFormat?: CloudflareJsonSchema;
  /** Custom AI model(s); first succeeds, rest are fallbacks. */
  customAi?: CloudflareCustomAi[];
}

// ---------------------------------------------------------------------------
// CloudflareBrowserEngine class
// ---------------------------------------------------------------------------

function buildGotoOptions(
  options: CloudflarePageLoadOptions,
): Record<string, unknown> {
  return {
    waitUntil: options.waitUntil ?? "networkidle0",
    timeout: options.timeout ?? 30_000,
  };
}

function parseCloudflareResponse<T>(
  res: Response,
  data: Record<string, unknown>,
  resultValidator: (result: unknown) => T | null,
): { success: true; result: T } | CloudflareBrowserError {
  if (!res.ok) {
    const errMsg =
      (Array.isArray(data.errors) &&
        data.errors.length > 0 &&
        (data.errors[0] as { message?: string }).message) ||
      "Cloudflare Browser Rendering request failed";
    return {
      success: false,
      error: typeof errMsg === "string" ? errMsg : String(data.errors),
    };
  }

  if (data.success === false) {
    const errMsg =
      (Array.isArray(data.errors) &&
        data.errors.length > 0 &&
        (data.errors[0] as { message?: string }).message) ||
      "Cloudflare returned success: false";
    return {
      success: false,
      error: typeof errMsg === "string" ? errMsg : "Unknown error",
    };
  }

  const result = data.result;
  const validated = resultValidator(result);
  if (validated === null) {
    return {
      success: false,
      error: "Cloudflare returned invalid result shape",
    };
  }

  return { success: true, result: validated };
}

type CfRequestResult =
  | { ok: true; data: Record<string, unknown>; res: Response }
  | { ok: false; error: string };

async function cfRequest(
  accountId: string,
  apiToken: string,
  endpoint: string,
  body: Record<string, unknown>,
): Promise<CfRequestResult> {
  let res: Response;
  try {
    res = await fetch(
      `${CLOUDFLARE_API_BASE}/${accountId}/browser-rendering/${endpoint}`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${apiToken}`,
        },
        body: JSON.stringify(body),
      },
    );
  } catch (e) {
    return {
      ok: false,
      error: e instanceof Error ? e.message : "Cloudflare request failed",
    };
  }

  let data: Record<string, unknown>;
  try {
    data = (await res.json()) as Record<string, unknown>;
  } catch {
    return {
      ok: false,
      error: "Invalid JSON from Cloudflare Browser Rendering",
    };
  }

  return { ok: true, data, res };
}

/**
 * Cloudflare Browser Rendering Engine.
 * Use accountId and apiToken (from env CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_BROWSER_RENDERING_API_TOKEN).
 */
export class CloudflareBrowserEngine {
  constructor(
    public readonly accountId: string,
    public readonly apiToken: string,
  ) {}

  /**
   * Fetch rendered HTML from /content.
   * https://developers.cloudflare.com/browser-rendering/rest-api/content-endpoint/
   */
  async content(
    url: string,
    options: CloudflareContentOptions = {},
  ): Promise<CloudflareContentOutput> {
    const body: Record<string, unknown> = {
      url,
      gotoOptions: buildGotoOptions(options),
    };
    if (options.viewport) body.viewport = options.viewport;

    const req = await cfRequest(this.accountId, this.apiToken, "content", body);
    if (!req.ok) return { success: false, error: req.error };

    const parsed = parseCloudflareResponse(req.res, req.data, (r) =>
      typeof r === "string" ? r : null,
    );
    if (!parsed.success) return parsed;
    return {
      success: true,
      html: parsed.result,
      statusCode: 200,
    };
  }

  /**
   * Fetch HTML + base64 screenshot from /snapshot.
   * https://developers.cloudflare.com/browser-rendering/rest-api/snapshot/
   */
  async snapshot(
    url: string,
    options: CloudflareSnapshotOptions = {},
  ): Promise<CloudflareSnapshotOutput> {
    const body: Record<string, unknown> = {
      url,
      gotoOptions: buildGotoOptions(options),
      screenshotOptions: {
        fullPage: options.fullPage ?? false,
        ...(options.screenshotViewport && {
          viewport: options.screenshotViewport,
        }),
      },
    };
    if (options.viewport) body.viewport = options.viewport;

    const req = await cfRequest(
      this.accountId,
      this.apiToken,
      "snapshot",
      body,
    );
    if (!req.ok) return { success: false, error: req.error };

    const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
      if (
        r &&
        typeof r === "object" &&
        "content" in r &&
        "screenshot" in r &&
        typeof (r as { content?: unknown }).content === "string" &&
        typeof (r as { screenshot?: unknown }).screenshot === "string"
      ) {
        return r as { content: string; screenshot: string };
      }
      return null;
    });
    if (!parsed.success) return parsed;
    return {
      success: true,
      html: parsed.result.content,
      statusCode: 200,
      screenshotBase64: parsed.result.screenshot,
    };
  }

  /**
   * Extract page as Markdown from /markdown.
   * https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/
   */
  async markdown(
    url: string,
    options: CloudflareMarkdownOptions = {},
  ): Promise<CloudflareMarkdownOutput> {
    const body: Record<string, unknown> = {
      url,
      gotoOptions: buildGotoOptions(options),
    };
    if (options.viewport) body.viewport = options.viewport;
    if (options.rejectRequestPattern?.length)
      body.rejectRequestPattern = options.rejectRequestPattern;

    const req = await cfRequest(
      this.accountId,
      this.apiToken,
      "markdown",
      body,
    );
    if (!req.ok) return { success: false, error: req.error };

    const parsed = parseCloudflareResponse(req.res, req.data, (r) =>
      typeof r === "string" ? r : null,
    );
    if (!parsed.success) return parsed;
    return {
      success: true,
      markdown: parsed.result,
      statusCode: 200,
    };
  }

  /**
   * Scrape elements by CSS selectors from /scrape.
   * Returns text, html, attributes, dimensions per element.
   * https://developers.cloudflare.com/browser-rendering/rest-api/scrape-endpoint/
   */
  async scrape(
    url: string,
    options: CloudflareScrapeOptions,
  ): Promise<CloudflareScrapeOutput> {
    const { elements, ...pageOpts } = options;
    if (!elements?.length) {
      return {
        success: false,
        error: "scrape requires at least one element with selector",
      };
    }

    const body: Record<string, unknown> = {
      url,
      elements: elements.map((e) => ({ selector: e.selector })),
      gotoOptions: buildGotoOptions(pageOpts),
    };
    if (pageOpts.viewport) body.viewport = pageOpts.viewport;

    const req = await cfRequest(this.accountId, this.apiToken, "scrape", body);
    if (!req.ok) return { success: false, error: req.error };

    const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
      if (Array.isArray(r)) {
        return r as CloudflareScrapeSelectorResult[];
      }
      return null;
    });
    if (!parsed.success) return parsed;
    return {
      success: true,
      data: parsed.result,
      statusCode: 200,
    };
  }

  /**
   * AI-powered structured JSON extraction from /json.
   * Requires prompt and/or responseFormat. Uses Workers AI by default; use customAi for BYO model.
   * https://developers.cloudflare.com/browser-rendering/rest-api/json-endpoint/
   */
  async json(
    url: string,
    options: CloudflareJsonOptions,
  ): Promise<CloudflareJsonOutput> {
    const { prompt, responseFormat, customAi, ...pageOpts } = options;
    if (!prompt && !responseFormat) {
      return {
        success: false,
        error: "json requires prompt and/or responseFormat",
      };
    }

    const body: Record<string, unknown> = {
      url,
      gotoOptions: buildGotoOptions(pageOpts),
    };
    if (pageOpts.viewport) body.viewport = pageOpts.viewport;
    if (prompt) body.prompt = prompt;
    if (responseFormat)
      body.response_format = {
        type: "json_schema",
        schema: responseFormat.schema,
      };
    if (customAi?.length) body.custom_ai = customAi;

    const req = await cfRequest(this.accountId, this.apiToken, "json", body);
    if (!req.ok) return { success: false, error: req.error };

    const parsed = parseCloudflareResponse(req.res, req.data, (r) => {
      if (r && typeof r === "object" && !Array.isArray(r)) {
        return r as Record<string, unknown>;
      }
      return null;
    });
    if (!parsed.success) return parsed;
    return {
      success: true,
      data: parsed.result,
      statusCode: 200,
    };
  }
}

// ---------------------------------------------------------------------------
// Standalone functions (backward compatibility)
// ---------------------------------------------------------------------------

export async function cloudflareFetchContent(
  accountId: string,
  apiToken: string,
  url: string,
  options: CloudflareContentOptions = {},
): Promise<CloudflareContentOutput> {
  const engine = new CloudflareBrowserEngine(accountId, apiToken);
  return engine.content(url, options);
}

export async function cloudflareFetchSnapshot(
  accountId: string,
  apiToken: string,
  url: string,
  options: CloudflareSnapshotOptions = {},
): Promise<CloudflareSnapshotOutput> {
  const engine = new CloudflareBrowserEngine(accountId, apiToken);
  return engine.snapshot(url, options);
}

export function isCloudflareBrowserEnabled(env: {
  CLOUDFLARE_ACCOUNT_ID?: string;
  CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
}): boolean {
  const accountId = env.CLOUDFLARE_ACCOUNT_ID;
  const token = env.CLOUDFLARE_BROWSER_RENDERING_API_TOKEN;
  return (
    typeof accountId === "string" &&
    accountId.trim().length > 0 &&
    typeof token === "string" &&
    token.trim().length > 0
  );
}

/**
 * Create a CloudflareBrowserEngine from env bindings.
 * Returns null if credentials are not configured.
 */
export function createCloudflareBrowserEngine(env: {
  CLOUDFLARE_ACCOUNT_ID?: string;
  CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
}): CloudflareBrowserEngine | null {
  if (!isCloudflareBrowserEnabled(env)) return null;
  return new CloudflareBrowserEngine(
    env.CLOUDFLARE_ACCOUNT_ID!,
    env.CLOUDFLARE_BROWSER_RENDERING_API_TOKEN!,
  );
}
