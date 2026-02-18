import type { AppzcrawlContainer } from "./container";
import type { ScrapeFormat } from "./contracts/scrape";

/**
 * Worker bindings and env for appzcrawl API.
 */
export interface AppEnv {
  Bindings: {
    DB: D1Database;
    BUCKET: R2Bucket;
    /** RPC binding to appz-browser service (screenshots, branding). Required for screenshot/branding formats. */
    BROWSER_SERVICE?: import("@services/appz-browser").BrowserServiceBinding;
    /** Cloudflare Container running the native addon server (extract-links, transform-html, etc.) */
    APPZCRAWL_CONTAINER: DurableObjectNamespace<AppzcrawlContainer>;
    /** RPC binding to appzcrawl-engine (workers-rs). Pure Rust Worker for HTML processing + html-to-markdown. */
    APPZCRAWL_ENGINE?: AppzcrawlEngineRpc;
    MARKDOWN_SERVICE_URL?: string;
    /** Optional fire-engine service URL (e.g. https://fire-engine.example.com). When set, scrape can use browser/TLS client via POST /scrape + poll. */
    FIRE_ENGINE_URL?: string;
    /** Cloudflare account ID for Browser Rendering API (engine: cloudflare). */
    CLOUDFLARE_ACCOUNT_ID?: string;
    /** API token for Cloudflare Browser Rendering (engine: cloudflare). Use wrangler secret for production. */
    CLOUDFLARE_BROWSER_RENDERING_API_TOKEN?: string;
    /** Cloudflare Workers AI for branding LLM enhancement */
    AI?: Ai;
    /** LlamaParse API key for PDF parsing. When set, PDFs use LlamaParse instead of native pdf-extract. */
    LLAMAPARSE_API_KEY?: string;
    /** Sarvam Vision API key for PDF parsing. When set (and LlamaParse not set), PDFs use Sarvam Document Intelligence. */
    SARVAM_API_KEY?: string;
    ENVIRONMENT?: string;
    /**
     * When `"true"`, HTML processing functions (extractLinks, extractMetadata,
     * etc.) are routed through the native Rust container instead of the
     * Worker-native HTMLRewriter implementation. Useful for debugging or
     * when the container provides higher-fidelity results.
     */
    USE_CONTAINER_BACKEND?: string;
    /**
     * When `"true"`, disables the WASM backend for HTML processing functions.
     * Falls back to HTMLRewriter (worker-native) or Container.
     */
    DISABLE_WASM_BACKEND?: string;
    /** Single API key for dev; if set, any request with this key is authenticated as this team */
    API_KEY?: string;
    /** Secret required in X-Dev-Create-Key header to call /dev/create-api-key from anywhere */
    DEV_CREATE_KEY?: string;
    /** Cloudflare Queue producer for async crawl jobs. */
    CRAWL_QUEUE?: Queue<CrawlQueueMessage>;
    /** Cloudflare Queue producer for per-URL scrape jobs (fan-out from crawl). */
    SCRAPE_QUEUE?: Queue<ScrapeQueueMessage>;
  };
  Variables: {
    auth?: { team_id: string };
    /** Full auth context — equivalent to Firecrawl's AuthCreditUsageChunk. */
    acuc?: import("./lib/auth-context").AuthCreditUsageChunk;
    account?: { remainingCredits: number };
    requestTiming?: { startTime: number; version: string };
  };
}

/** Message shape sent to CRAWL_QUEUE. */
export interface CrawlQueueMessage {
  /** Crawl job ID (crawl_jobs.id). */
  crawlId: string;
  /** Seed URL. */
  url: string;
  /** Team that owns this crawl. */
  teamId: string;
}

/** Message shape sent to SCRAPE_QUEUE for per-URL fan-out. */
export interface ScrapeQueueMessage {
  /** Job type: "crawl" (link discovery crawl) or "batch_scrape" (direct batch scrape). */
  jobType: "crawl" | "batch_scrape";
  /** Parent crawl/batch job ID. */
  crawlId: string;
  /** URL to scrape. */
  url: string;
  /** Team that owns this crawl. */
  teamId: string;
  /** Scrape options (serialized from crawl's scrapeOptions). */
  scrapeOptions?: {
    onlyMainContent?: boolean;
    useFireEngine?: boolean;
    engine?: "native" | "cloudflare" | "auto";
    screenshotBaseUrl?: string;
    formats?: ScrapeFormat[];
    includeTags?: string[];
    excludeTags?: string[];
    maxAge?: number;
    storeInCache?: boolean;
    zeroDataRetention?: boolean;
    headers?: Record<string, string>;
    citations?: boolean;
    mobile?: boolean;
    removeBase64Images?: boolean;
    blockAds?: boolean;
    skipTlsVerification?: boolean;
    timeout?: number;
    screenshotOptions?: {
      fullPage?: boolean;
      viewport?: { width: number; height: number };
      quality?: number;
    };
    waitFor?: number;
    jsonOptions?: { prompt?: string; schema?: Record<string, unknown> };
  };
  /** Depth from seed URL (for maxDiscoveryDepth enforcement). Used only for crawl jobs. */
  depth?: number;
}

/** Rate limiter mode for auth middleware (used for future rate limiting). */
export enum RateLimiterMode {
  Crawl = "crawl",
  CrawlStatus = "crawlStatus",
  Scrape = "scrape",
  ScrapeAgentPreview = "scrapeAgentPreview",
  Preview = "preview",
  Search = "search",
  Map = "map",
  Extract = "extract",
  ExtractStatus = "extractStatus",
  ExtractAgentPreview = "extractAgentPreview",
}

export const UNSUPPORTED_SITE_MESSAGE =
  "This site is not supported. Please check the blocklist or contact support.";

/**
 * RPC interface for appzcrawl-engine (workers-rs).
 * Each method takes/returns JSON strings and is callable via Service Binding.
 */
export interface AppzcrawlEngineRpc {
  rpc_extract_links(html: string): string;
  rpc_extract_base_href(html: string, url: string): string;
  rpc_extract_metadata(html: string): string;
  rpc_transform_html(opts_json: string): string;
  rpc_get_inner_json(html: string): string;
  rpc_extract_attributes(html: string, options_json: string): string;
  rpc_extract_images(html: string, base_url: string): string;
  rpc_extract_assets(
    html: string,
    base_url: string,
    formats_json: string,
  ): string;
  rpc_post_process_markdown(
    markdown: string,
    base_url: string,
    citations: boolean,
  ): string;
  rpc_html_to_markdown(html: string): string;
  rpc_filter_links(params_json: string): string;
  rpc_parse_sitemap(xml: string): string;
  rpc_convert_document(params_json: string): string;
  /** Async — performs DDG search via Workers fetch. Returns Promise. */
  rpc_search(query: string, options_json: string): Promise<string>;
}
