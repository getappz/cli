import {
  index,
  integer,
  primaryKey,
  sqliteTable,
  text,
} from "drizzle-orm/sqlite-core";

export const apiKeys = sqliteTable(
  "api_keys",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    /** SHA-256 hash of the full ac-{uuid} key — used for secure lookup. */
    keyHash: text("key_hash").notNull().unique(),
    /** Visible prefix for display: e.g. "ac-3d47...fd2a". */
    keyPrefix: text("key_prefix").notNull(),
    /** FK to teams.id / team_credits.team_id. */
    teamId: text("team_id").notNull(),
    /** Human-readable key name: "Default", "Production", "CI/CD", etc. */
    name: text("name").default("Default"),
    /** JSON array of allowed scopes. null = all scopes. e.g. ["scrape","crawl"]. */
    scopes: text("scopes"),
    /** Last time this key was used for authentication (ISO). */
    lastUsedAt: text("last_used_at"),
    /** Optional expiry timestamp (ISO). Null = never expires. */
    expiresAt: text("expires_at"),
    /** Who/what created this key (user ID or "system"). */
    createdBy: text("created_by"),
    createdAt: text("created_at").notNull(),
    deletedAt: text("deleted_at"),
  },
  (table) => ({
    teamIdx: index("idx_api_keys_team").on(table.teamId),
  }),
);

export const teamCredits = sqliteTable("team_credits", {
  teamId: text("team_id").primaryKey(),
  credits: integer("credits").notNull().default(999_999),
});

export const idempotencyKeys = sqliteTable("idempotency_keys", {
  idempotencyKey: text("idempotency_key").primaryKey(),
  createdAt: text("created_at").notNull(),
});

export const jobs = sqliteTable("jobs", {
  id: text("id").primaryKey(),
  type: text("type").notNull(),
  status: text("status").notNull().default("pending"),
  teamId: text("team_id").notNull(),
  payload: text("payload"),
  result: text("result"),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

/** Scrape cache: D1 metadata for R2 document lookup. */
export const scrapeCache = sqliteTable(
  "scrape_cache",
  {
    id: text("id").primaryKey(),
    urlHash: text("url_hash").notNull(),
    cacheKey: text("cache_key").notNull(),
    urlResolved: text("url_resolved"),
    createdAtMs: integer("created_at_ms").notNull(),
    expiresAtMs: integer("expires_at_ms").notNull(),
    schemaVersion: integer("schema_version").notNull().default(1),
    statusCode: integer("status_code").notNull(),
    r2Key: text("r2_key").notNull(),
    formats: text("formats"),
  },
  (table) => ({
    lookupIdx: index("idx_scrape_cache_lookup").on(
      table.urlHash,
      table.cacheKey,
      table.expiresAtMs,
    ),
  }),
);

// ---------------------------------------------------------------------------
// Crawl jobs: tracks async crawl requests (Firecrawl /crawl equivalent).
// Replaces Redis StoredCrawl + BullMQ with D1 + Cloudflare Queues.
// ---------------------------------------------------------------------------

/** Main crawl job record — one per POST /v2/crawl or POST /v2/batch/scrape request. */
export const crawlJobs = sqliteTable(
  "crawl_jobs",
  {
    /** UUID v7 (time-sortable). */
    id: text("id").primaryKey(),
    /** Type: crawl | batch_scrape */
    type: text("type").notNull().default("crawl"),
    teamId: text("team_id").notNull(),
    /** Seed URL for the crawl (or first URL for batch scrape). */
    originUrl: text("origin_url").notNull(),
    /** pending | scraping | completed | cancelled | failed */
    status: text("status").notNull().default("pending"),
    /** JSON-serialised CrawlerOptions (null for batch scrape). */
    crawlerOptions: text("crawler_options"),
    /** JSON-serialised scrapeOptions (per-page). */
    scrapeOptions: text("scrape_options"),
    /** robots.txt content fetched at crawl start. */
    robotsTxt: text("robots_txt"),
    /** Number of pages successfully scraped. */
    completedCount: integer("completed_count").notNull().default(0),
    /** Total URLs discovered (queued + done + failed). */
    totalCount: integer("total_count").notNull().default(0),
    /** Credits billed. */
    creditsBilled: integer("credits_billed").notNull().default(0),
    /** Webhook URL for completion notification. */
    webhook: text("webhook"),
    /** Whether the crawl was cancelled. */
    cancelled: integer("cancelled", { mode: "boolean" })
      .notNull()
      .default(false),
    /** Zero data retention flag. */
    zeroDataRetention: integer("zero_data_retention", { mode: "boolean" })
      .notNull()
      .default(false),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
    /** Expiry timestamp (ISO). After this, the crawl and its results are eligible for cleanup. */
    expiresAt: text("expires_at").notNull(),
  },
  (table) => ({
    teamStatusIdx: index("idx_crawl_jobs_team_status").on(
      table.teamId,
      table.status,
    ),
    typeIdx: index("idx_crawl_jobs_type").on(table.type),
  }),
);

/** Per-URL results within a crawl (scrape output for each discovered page). */
export const crawlResults = sqliteTable(
  "crawl_results",
  {
    /** UUID per result. */
    id: text("id").primaryKey(),
    /** FK to crawl_jobs.id. */
    crawlId: text("crawl_id").notNull(),
    /** Scraped URL. */
    url: text("url").notNull(),
    /** success | failed */
    status: text("status").notNull().default("pending"),
    /** R2 key for full document JSON (optional; for large payloads). */
    r2Key: text("r2_key"),
    /** JSON document when small enough to fit inline. */
    documentJson: text("document_json"),
    /** Error message for failed scrapes. */
    error: text("error"),
    /** Error code (e.g. SCRAPE_TIMEOUT, FETCH_ERROR). */
    code: text("code"),
    /** HTTP status code from scrape. */
    statusCode: integer("status_code"),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    crawlIdx: index("idx_crawl_results_crawl").on(table.crawlId, table.status),
  }),
);

/** URLs blocked by robots.txt during a crawl (Firecrawl-compatible). */
export const crawlRobotsBlocked = sqliteTable(
  "crawl_robots_blocked",
  {
    /** FK to crawl_jobs.id. */
    crawlId: text("crawl_id").notNull(),
    /** URL that was blocked by robots.txt. */
    url: text("url").notNull(),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    crawlIdx: index("idx_crawl_robots_blocked").on(table.crawlId),
    uniqueUrl: index("idx_crawl_robots_blocked_unique").on(
      table.crawlId,
      table.url,
    ),
  }),
);

// ---------------------------------------------------------------------------
// Scrapes: tracks individual scrape requests (Firecrawl /scrape equivalent).
// Stores job metadata and result for status lookup via GET /v2/scrape/:jobId.
// ---------------------------------------------------------------------------

/** Individual scrape job record — one per POST /v2/scrape request. */
export const scrapes = sqliteTable(
  "scrapes",
  {
    /** UUID (matches Firecrawl's scrape ID format). */
    id: text("id").primaryKey(),
    /** Team that owns this scrape. */
    teamId: text("team_id").notNull(),
    /** Scraped URL. */
    url: text("url").notNull(),
    /** pending | completed | failed */
    status: text("status").notNull().default("pending"),
    /** Whether the scrape succeeded. */
    success: integer("success", { mode: "boolean" }).notNull().default(false),
    /** JSON-serialised scrape options (formats, onlyMainContent, etc.). */
    options: text("options"),
    /** JSON-serialised result document (when small enough to fit inline). */
    result: text("result"),
    /** R2 key for large result documents. */
    r2Key: text("r2_key"),
    /** Error message for failed scrapes. */
    error: text("error"),
    /** Zero data retention flag - when true, result is not stored. */
    zeroDataRetention: integer("zero_data_retention", { mode: "boolean" })
      .notNull()
      .default(false),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (table) => ({
    teamIdx: index("idx_scrapes_team").on(table.teamId),
    statusIdx: index("idx_scrapes_status").on(table.status),
  }),
);

// ---------------------------------------------------------------------------
// Extract jobs: tracks LLM-based extraction requests (Firecrawl /extract).
// Maps to Firecrawl's Redis extract store + Supabase `extracts` table.
// ---------------------------------------------------------------------------

/** Extract job record — one per POST /v2/extract request. */
export const extractJobs = sqliteTable(
  "extract_jobs",
  {
    /** UUID. */
    id: text("id").primaryKey(),
    /** Team that owns this extract job. */
    teamId: text("team_id").notNull(),
    /** pending | processing | completed | failed */
    status: text("status").notNull().default("pending"),
    /** JSON array of source URLs to extract from. */
    urls: text("urls"),
    /** Extraction prompt describing what to extract. */
    prompt: text("prompt"),
    /** JSON schema for structured extraction output. */
    schemaJson: text("schema_json"),
    /** Optional system prompt for the LLM. */
    systemPrompt: text("system_prompt"),
    /** JSON-serialised extract options (limit, ignoreSitemap, etc.). */
    options: text("options"),
    /** JSON result (inline if small). */
    result: text("result"),
    /** R2 key for large results. */
    r2Key: text("r2_key"),
    /** Error message if failed. */
    error: text("error"),
    /** Warning message (partial success, etc.). */
    warning: text("warning"),
    /** Credits billed for this extraction. */
    creditsBilled: integer("credits_billed").notNull().default(0),
    /** Webhook URL for completion notification. */
    webhook: text("webhook"),
    /** Zero data retention flag. */
    zeroDataRetention: integer("zero_data_retention", { mode: "boolean" })
      .notNull()
      .default(false),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
    /** Expiry timestamp (ISO). After this, the job is eligible for cleanup. */
    expiresAt: text("expires_at").notNull(),
  },
  (table) => ({
    teamIdx: index("idx_extract_jobs_team").on(table.teamId),
    statusIdx: index("idx_extract_jobs_status").on(table.status),
  }),
);

// ---------------------------------------------------------------------------
// Agent jobs: tracks AI agent requests (Firecrawl /agent).
// Maps to Firecrawl's Supabase `agents` table + external agent service.
// ---------------------------------------------------------------------------

/** Agent job record — one per POST /v2/agent request. */
export const agentJobs = sqliteTable(
  "agent_jobs",
  {
    /** UUID. */
    id: text("id").primaryKey(),
    /** Team that owns this agent job. */
    teamId: text("team_id").notNull(),
    /** pending | processing | completed | failed */
    status: text("status").notNull().default("pending"),
    /** Agent prompt (required). */
    prompt: text("prompt").notNull(),
    /** JSON array of seed URLs. */
    urls: text("urls"),
    /** JSON schema for structured output. */
    schemaJson: text("schema_json"),
    /** Agent model: spark-1-pro | spark-1-mini. */
    model: text("model"),
    /** JSON-serialised agent options (maxCredits, strictConstrainToURLs, etc.). */
    options: text("options"),
    /** JSON result (inline if small). */
    result: text("result"),
    /** R2 key for large results. */
    r2Key: text("r2_key"),
    /** Error message if failed. */
    error: text("error"),
    /** Credits billed for this agent run. */
    creditsBilled: integer("credits_billed").notNull().default(0),
    /** Webhook URL for completion notification. */
    webhook: text("webhook"),
    /** Zero data retention flag. */
    zeroDataRetention: integer("zero_data_retention", { mode: "boolean" })
      .notNull()
      .default(false),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
    /** Expiry timestamp (ISO). After this, the job is eligible for cleanup. */
    expiresAt: text("expires_at").notNull(),
  },
  (table) => ({
    teamIdx: index("idx_agent_jobs_team").on(table.teamId),
    statusIdx: index("idx_agent_jobs_status").on(table.status),
  }),
);

// ---------------------------------------------------------------------------
// Webhook logs: audit trail for webhook delivery attempts.
// Maps to Firecrawl's Supabase `webhook_logs` table.
// ---------------------------------------------------------------------------

/** Webhook delivery log — one row per delivery attempt. */
export const webhookLogs = sqliteTable(
  "webhook_logs",
  {
    /** UUID. */
    id: text("id").primaryKey(),
    /** Team that owns the parent job. */
    teamId: text("team_id").notNull(),
    /** FK to the parent job (crawl_jobs, extract_jobs, agent_jobs, etc.). */
    jobId: text("job_id").notNull(),
    /** Type of the parent job: crawl | batch_scrape | extract | agent. */
    jobType: text("job_type").notNull(),
    /** Webhook event: crawl.completed | crawl.page | batch_scrape.completed | etc. */
    event: text("event").notNull(),
    /** Webhook destination URL. */
    url: text("url").notNull(),
    /** HTTP response status code from the webhook target. */
    statusCode: integer("status_code"),
    /** Whether the delivery was successful. */
    success: integer("success", { mode: "boolean" }).notNull().default(false),
    /** Delivery attempt number (1-based). */
    attempt: integer("attempt").notNull().default(1),
    /** Sent payload JSON (optional, for debugging). */
    requestBody: text("request_body"),
    /** Response body from webhook target (optional, for debugging). */
    responseBody: text("response_body"),
    /** Error message if delivery failed. */
    error: text("error"),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    jobIdx: index("idx_webhook_logs_job").on(table.jobId),
    teamIdx: index("idx_webhook_logs_team").on(table.teamId),
  }),
);

// ---------------------------------------------------------------------------
// Request log: unified audit trail for all API requests.
// Maps to Firecrawl's Supabase `scrapes` + `requests` tables.
// ---------------------------------------------------------------------------

/** Request audit log — one row per API request. */
export const requestLog = sqliteTable(
  "request_log",
  {
    /** UUID. */
    id: text("id").primaryKey(),
    /** Team that made the request. */
    teamId: text("team_id").notNull(),
    /** FK to api_keys.id (optional). */
    apiKeyId: integer("api_key_id"),
    /** API endpoint path: /v2/scrape, /v2/crawl, etc. */
    endpoint: text("endpoint").notNull(),
    /** HTTP method: POST, GET, DELETE. */
    method: text("method").notNull(),
    /** Associated job ID if applicable (scrape ID, crawl ID, etc.). */
    jobId: text("job_id"),
    /** Credits billed for this request. */
    creditsBilled: integer("credits_billed").notNull().default(0),
    /** HTTP response status code. */
    statusCode: integer("status_code"),
    /** Request duration in milliseconds. */
    durationMs: integer("duration_ms"),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    teamIdx: index("idx_request_log_team").on(table.teamId),
    createdIdx: index("idx_request_log_created").on(table.createdAt),
  }),
);

// ---------------------------------------------------------------------------
// Billing log: credit transaction audit trail.
// Maps to Firecrawl's Redis `billing_batch` + Supabase billing RPCs.
// ---------------------------------------------------------------------------

/** Billing log — one row per credit transaction (deduction or refund). */
export const billingLog = sqliteTable(
  "billing_log",
  {
    /** UUID. */
    id: text("id").primaryKey(),
    /** Team that was billed/refunded. */
    teamId: text("team_id").notNull(),
    /** Associated job ID (optional). */
    jobId: text("job_id"),
    /** Job type: scrape | crawl | extract | search | agent. */
    jobType: text("job_type"),
    /** Credits: positive = deduction, negative = refund/recharge. */
    credits: integer("credits").notNull(),
    /** Reason for the transaction: scrape_base | json_format | pdf_pages | stealth_proxy | etc. */
    reason: text("reason").notNull(),
    /** Credits remaining after this transaction. */
    balanceAfter: integer("balance_after"),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    teamIdx: index("idx_billing_log_team").on(table.teamId),
    createdIdx: index("idx_billing_log_created").on(table.createdAt),
  }),
);

// ---------------------------------------------------------------------------
// Teams: extended team configuration.
// Maps to Firecrawl's Supabase `teams` table.
// Separate from team_credits to avoid locking on credit updates.
// ---------------------------------------------------------------------------

/** Team configuration — one row per team. */
export const teams = sqliteTable("teams", {
  /** team_id (matches api_keys.team_id and team_credits.team_id). */
  id: text("id").primaryKey(),
  /** Team display name. */
  name: text("name"),
  /** HMAC secret for webhook signature verification. */
  hmacSecret: text("hmac_secret"),
  /** Per-minute rate limit for scrape requests. */
  rateLimitScrape: integer("rate_limit_scrape").default(100),
  /** Per-minute rate limit for crawl requests. */
  rateLimitCrawl: integer("rate_limit_crawl").default(15),
  /** Per-minute rate limit for search requests. */
  rateLimitSearch: integer("rate_limit_search").default(100),
  /** Per-minute rate limit for extract requests. */
  rateLimitExtract: integer("rate_limit_extract").default(100),
  /** Per-minute rate limit for map requests. */
  rateLimitMap: integer("rate_limit_map").default(100),
  /** Maximum concurrent scrape/crawl jobs for this team. */
  maxConcurrency: integer("max_concurrency").default(10),
  /** Crawl TTL in hours (how long crawl data is retained). */
  crawlTtlHours: integer("crawl_ttl_hours").default(24),
  /** JSON flags: { zdrCost, bypassCreditChecks, ... }. */
  flags: text("flags"),
  /** Whether auto-recharge is enabled. */
  autoRecharge: integer("auto_recharge", { mode: "boolean" })
    .notNull()
    .default(false),
  /** Credit threshold below which auto-recharge triggers. */
  autoRechargeThreshold: integer("auto_recharge_threshold"),
  createdAt: text("created_at").notNull(),
  updatedAt: text("updated_at").notNull(),
});

// ---------------------------------------------------------------------------
// URL blocklist: domains/URLs that cannot be scraped.
// Maps to Firecrawl's Supabase `blocklist` table.
// ---------------------------------------------------------------------------

/** URL blocklist entry — blocks scraping of matching domains/URLs. */
export const urlBlocklist = sqliteTable("url_blocklist", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  /** Domain or URL pattern (glob). */
  pattern: text("pattern").notNull().unique(),
  /** Human-readable reason for blocking. */
  reason: text("reason"),
  createdAt: text("created_at").notNull(),
});

// ---------------------------------------------------------------------------
// Crawl visited URLs: persistent URL dedup during crawl.
// Replaces Firecrawl's Redis `crawl:{id}:visited` set.
// Persists across queue retries (unlike in-memory dedup).
// ---------------------------------------------------------------------------

/** Visited URL record — tracks URLs already processed in a crawl. */
export const crawlVisitedUrls = sqliteTable(
  "crawl_visited_urls",
  {
    /** FK to crawl_jobs.id. */
    crawlId: text("crawl_id").notNull(),
    /** SHA-256 hash of normalised URL (for fast dedup). */
    urlHash: text("url_hash").notNull(),
    /** Original URL (for debugging / display). */
    url: text("url").notNull(),
    createdAt: text("created_at").notNull(),
  },
  (table) => ({
    pk: primaryKey({ columns: [table.crawlId, table.urlHash] }),
  }),
);
