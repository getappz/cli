/**
 * Crawl store: D1 operations for crawl_jobs and crawl_results.
 * Replaces Firecrawl's Redis crawl-redis.ts + BullMQ with D1.
 * Adapted from firecrawl/apps/api/src/lib/crawl-redis.ts.
 */

import type {
  CrawlDocument,
  CrawlerOptions,
  CrawlJobStatus,
} from "../contracts/crawl";
import type { ScrapeRequestBody } from "../contracts/scrape";
import { logger } from "../lib/logger";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CrawlJobRow {
  id: string;
  type: string; // "crawl" | "batch_scrape"
  team_id: string;
  origin_url: string;
  status: string;
  crawler_options: string | null;
  scrape_options: string | null;
  robots_txt: string | null;
  completed_count: number;
  total_count: number;
  credits_billed: number;
  webhook: string | null;
  cancelled: number; // D1 stores booleans as 0/1
  zero_data_retention: number;
  created_at: string;
  updated_at: string;
  expires_at: string;
}

export interface CrawlResultRow {
  id: string;
  crawl_id: string;
  url: string;
  status: string;
  r2_key: string | null;
  document_json: string | null;
  error: string | null;
  code: string | null;
  status_code: number | null;
  created_at: string;
}

export interface StoredCrawl {
  id: string;
  type: "crawl" | "batch_scrape";
  teamId: string;
  originUrl: string;
  status: CrawlJobStatus;
  crawlerOptions: CrawlerOptions;
  scrapeOptions: Omit<ScrapeRequestBody, "url">;
  robotsTxt: string | null;
  completedCount: number;
  totalCount: number;
  creditsBilled: number;
  webhook: string | null;
  cancelled: boolean;
  zeroDataRetention: boolean;
  createdAt: string;
  updatedAt: string;
  expiresAt: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function rowToCrawl(row: CrawlJobRow): StoredCrawl {
  return {
    id: row.id,
    type: (row.type as "crawl" | "batch_scrape") || "crawl",
    teamId: row.team_id,
    originUrl: row.origin_url,
    status: row.status as CrawlJobStatus,
    crawlerOptions: row.crawler_options ? JSON.parse(row.crawler_options) : {},
    scrapeOptions: row.scrape_options ? JSON.parse(row.scrape_options) : {},
    robotsTxt: row.robots_txt,
    completedCount: row.completed_count,
    totalCount: row.total_count,
    creditsBilled: row.credits_billed,
    webhook: row.webhook,
    cancelled: Boolean(row.cancelled),
    zeroDataRetention: Boolean(row.zero_data_retention),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    expiresAt: row.expires_at,
  };
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/** Create a new crawl job in D1 (replaces saveCrawl + markCrawlActive in Redis). */
export async function createCrawlJob(
  db: D1Database,
  params: {
    id: string;
    type?: "crawl" | "batch_scrape";
    teamId: string;
    originUrl: string;
    crawlerOptions: CrawlerOptions;
    scrapeOptions: Omit<ScrapeRequestBody, "url">;
    webhook?: string;
    zeroDataRetention?: boolean;
    expiresAt?: string;
  },
): Promise<StoredCrawl> {
  const now = new Date().toISOString();
  const expiresAt =
    params.expiresAt ??
    new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
  const type = params.type ?? "crawl";

  await db
    .prepare(
      `INSERT INTO crawl_jobs
        (id, type, team_id, origin_url, status, crawler_options, scrape_options,
         webhook, zero_data_retention, created_at, updated_at, expires_at)
       VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      params.id,
      type,
      params.teamId,
      params.originUrl,
      JSON.stringify(params.crawlerOptions),
      JSON.stringify(params.scrapeOptions),
      params.webhook ?? null,
      params.zeroDataRetention ? 1 : 0,
      now,
      now,
      expiresAt,
    )
    .run();

  return {
    id: params.id,
    type,
    teamId: params.teamId,
    originUrl: params.originUrl,
    status: "pending",
    crawlerOptions: params.crawlerOptions,
    scrapeOptions: params.scrapeOptions,
    robotsTxt: null,
    completedCount: 0,
    totalCount: 0,
    creditsBilled: 0,
    webhook: params.webhook ?? null,
    cancelled: false,
    zeroDataRetention: Boolean(params.zeroDataRetention),
    createdAt: now,
    updatedAt: now,
    expiresAt,
  };
}

/** Fetch crawl job by ID (replaces getCrawl from Redis). */
export async function getCrawlJob(
  db: D1Database,
  crawlId: string,
): Promise<StoredCrawl | null> {
  const row = await db
    .prepare("SELECT * FROM crawl_jobs WHERE id = ? LIMIT 1")
    .bind(crawlId)
    .first<CrawlJobRow>();
  return row ? rowToCrawl(row) : null;
}

/** Update crawl job status (replaces saveCrawl in Redis). */
export async function updateCrawlStatus(
  db: D1Database,
  crawlId: string,
  status: CrawlJobStatus,
  updates?: {
    completedCount?: number;
    totalCount?: number;
    creditsBilled?: number;
    robotsTxt?: string;
    cancelled?: boolean;
  },
): Promise<void> {
  const now = new Date().toISOString();
  const sets: string[] = ["status = ?", "updated_at = ?"];
  const values: (string | number | null)[] = [status, now];

  if (updates?.completedCount !== undefined) {
    sets.push("completed_count = ?");
    values.push(updates.completedCount);
  }
  if (updates?.totalCount !== undefined) {
    sets.push("total_count = ?");
    values.push(updates.totalCount);
  }
  if (updates?.creditsBilled !== undefined) {
    sets.push("credits_billed = ?");
    values.push(updates.creditsBilled);
  }
  if (updates?.robotsTxt !== undefined) {
    sets.push("robots_txt = ?");
    values.push(updates.robotsTxt);
  }
  if (updates?.cancelled !== undefined) {
    sets.push("cancelled = ?");
    values.push(updates.cancelled ? 1 : 0);
  }

  values.push(crawlId);
  await db
    .prepare(`UPDATE crawl_jobs SET ${sets.join(", ")} WHERE id = ?`)
    .bind(...values)
    .run();
}

/** Cancel a crawl job. */
export async function cancelCrawlJob(
  db: D1Database,
  crawlId: string,
): Promise<void> {
  await updateCrawlStatus(db, crawlId, "cancelled", { cancelled: true });
}

/** List ongoing crawls for a team (replaces ongoingCrawlsController from Redis). */
export async function getOngoingCrawls(
  db: D1Database,
  teamId: string,
): Promise<StoredCrawl[]> {
  const { results } = await db
    .prepare(
      `SELECT * FROM crawl_jobs
       WHERE team_id = ? AND status IN ('pending', 'scraping')
       ORDER BY created_at DESC`,
    )
    .bind(teamId)
    .all<CrawlJobRow>();
  return (results ?? []).map(rowToCrawl);
}

// ---------------------------------------------------------------------------
// Crawl results
// ---------------------------------------------------------------------------

/** Insert a crawl result (per-URL scrape output). */
export async function addCrawlResult(
  db: D1Database,
  params: {
    id: string;
    crawlId: string;
    url: string;
    status: "success" | "failed";
    documentJson?: string;
    r2Key?: string;
    error?: string;
    code?: string;
    statusCode?: number;
  },
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `INSERT INTO crawl_results
        (id, crawl_id, url, status, document_json, r2_key, error, code, status_code, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      params.id,
      params.crawlId,
      params.url,
      params.status,
      params.documentJson ?? null,
      params.r2Key ?? null,
      params.error ?? null,
      params.code ?? null,
      params.statusCode ?? null,
      now,
    )
    .run();
}

/** Get completed crawl results for a job, paginated. */
export async function getCrawlResults(
  db: D1Database,
  crawlId: string,
  options: { skip?: number; limit?: number } = {},
): Promise<CrawlDocument[]> {
  const skip = options.skip ?? 0;
  const limit = options.limit ?? 100;

  const { results } = await db
    .prepare(
      `SELECT * FROM crawl_results
       WHERE crawl_id = ? AND status = 'success'
       ORDER BY created_at ASC
       LIMIT ? OFFSET ?`,
    )
    .bind(crawlId, limit, skip)
    .all<CrawlResultRow>();

  const docs: CrawlDocument[] = [];
  for (const row of results ?? []) {
    if (row.document_json) {
      try {
        docs.push(JSON.parse(row.document_json));
      } catch {
        logger.warn("[crawl-store] failed to parse document_json", {
          resultId: row.id,
        });
      }
    }
  }
  return docs;
}

/** Get failed results (errors) for a crawl. */
export async function getCrawlErrors(
  db: D1Database,
  crawlId: string,
): Promise<
  Array<{
    id: string;
    url: string;
    error: string;
    code: string | null;
    timestamp: string;
  }>
> {
  const { results } = await db
    .prepare(
      `SELECT id, url, error, code, created_at FROM crawl_results
       WHERE crawl_id = ? AND status = 'failed'
       ORDER BY created_at ASC`,
    )
    .bind(crawlId)
    .all<{
      id: string;
      url: string;
      error: string;
      code: string | null;
      created_at: string;
    }>();

  return (results ?? []).map((r) => ({
    id: r.id,
    url: r.url,
    error: r.error,
    code: r.code,
    timestamp: r.created_at,
  }));
}

/** Increment completed count atomically. */
export async function incrementCompleted(
  db: D1Database,
  crawlId: string,
  by = 1,
): Promise<void> {
  await db
    .prepare(
      `UPDATE crawl_jobs
       SET completed_count = completed_count + ?, updated_at = ?
       WHERE id = ?`,
    )
    .bind(by, new Date().toISOString(), crawlId)
    .run();
}

/** Set total count (called after URL discovery). */
export async function setTotalCount(
  db: D1Database,
  crawlId: string,
  total: number,
): Promise<void> {
  await db
    .prepare(
      `UPDATE crawl_jobs SET total_count = ?, updated_at = ? WHERE id = ?`,
    )
    .bind(total, new Date().toISOString(), crawlId)
    .run();
}

/** Check if a crawl has been cancelled (quick read). */
export async function isCrawlCancelled(
  db: D1Database,
  crawlId: string,
): Promise<boolean> {
  const row = await db
    .prepare("SELECT cancelled FROM crawl_jobs WHERE id = ? LIMIT 1")
    .bind(crawlId)
    .first<{ cancelled: number }>();
  return Boolean(row?.cancelled);
}

/** Get URLs blocked by robots.txt during a crawl (Firecrawl-compatible). */
export async function getRobotsBlocked(
  db: D1Database,
  crawlId: string,
): Promise<string[]> {
  const { results } = await db
    .prepare(
      `SELECT url FROM crawl_robots_blocked
       WHERE crawl_id = ?
       ORDER BY created_at ASC`,
    )
    .bind(crawlId)
    .all<{ url: string }>();
  return (results ?? []).map((r) => r.url);
}

/** Add a URL blocked by robots.txt (replaces Redis SADD crawl:id:robots_blocked). */
export async function addRobotsBlocked(
  db: D1Database,
  crawlId: string,
  url: string,
): Promise<void> {
  const now = new Date().toISOString();
  try {
    await db
      .prepare(
        `INSERT INTO crawl_robots_blocked (crawl_id, url, created_at)
         VALUES (?, ?, ?)`,
      )
      .bind(crawlId, url, now)
      .run();
  } catch (e) {
    // Unique constraint violation — URL already tracked; ignore
    const msg = e instanceof Error ? e.message : String(e);
    if (
      msg.includes("UNIQUE") ||
      msg.includes("unique") ||
      msg.includes("constraint")
    ) {
      return;
    }
    throw e;
  }
}
