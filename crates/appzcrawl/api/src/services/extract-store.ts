/**
 * Extract store: D1 operations for extract_jobs table.
 * Adapted from Firecrawl's extract-redis.ts — replaces Redis with D1.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ExtractJobRow {
  id: string;
  team_id: string;
  status: string;
  urls: string | null;
  prompt: string | null;
  schema_json: string | null;
  system_prompt: string | null;
  options: string | null;
  result: string | null;
  r2_key: string | null;
  error: string | null;
  warning: string | null;
  credits_billed: number;
  webhook: string | null;
  zero_data_retention: number;
  created_at: string;
  updated_at: string;
  expires_at: string;
}

export interface StoredExtract {
  id: string;
  teamId: string;
  status: "pending" | "processing" | "completed" | "failed";
  urls: string[] | null;
  prompt: string | null;
  schemaJson: unknown | null;
  systemPrompt: string | null;
  options: Record<string, unknown> | null;
  result: unknown | null;
  r2Key: string | null;
  error: string | null;
  warning: string | null;
  creditsBilled: number;
  webhook: string | null;
  zeroDataRetention: boolean;
  createdAt: string;
  updatedAt: string;
  expiresAt: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function rowToExtract(row: ExtractJobRow): StoredExtract {
  return {
    id: row.id,
    teamId: row.team_id,
    status: row.status as StoredExtract["status"],
    urls: row.urls ? JSON.parse(row.urls) : null,
    prompt: row.prompt,
    schemaJson: row.schema_json ? JSON.parse(row.schema_json) : null,
    systemPrompt: row.system_prompt,
    options: row.options ? JSON.parse(row.options) : null,
    result: row.result ? JSON.parse(row.result) : null,
    r2Key: row.r2_key,
    error: row.error,
    warning: row.warning,
    creditsBilled: row.credits_billed,
    webhook: row.webhook,
    zeroDataRetention: Boolean(row.zero_data_retention),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    expiresAt: row.expires_at,
  };
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/** Create a new extract job record (pending state). */
export async function createExtractJob(
  db: D1Database,
  params: {
    id: string;
    teamId: string;
    urls?: string[];
    prompt?: string;
    schemaJson?: unknown;
    systemPrompt?: string;
    options?: Record<string, unknown>;
    webhook?: string;
    zeroDataRetention?: boolean;
  },
): Promise<void> {
  const now = new Date().toISOString();
  const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();

  await db
    .prepare(
      `INSERT INTO extract_jobs
        (id, team_id, status, urls, prompt, schema_json, system_prompt, options, webhook, zero_data_retention, created_at, updated_at, expires_at)
       VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      params.id,
      params.teamId,
      params.urls ? JSON.stringify(params.urls) : null,
      params.prompt ?? null,
      params.schemaJson ? JSON.stringify(params.schemaJson) : null,
      params.systemPrompt ?? null,
      params.options ? JSON.stringify(params.options) : null,
      params.webhook ?? null,
      params.zeroDataRetention ? 1 : 0,
      now,
      now,
      expiresAt,
    )
    .run();
}

/** Get an extract job by ID. */
export async function getExtractJob(
  db: D1Database,
  id: string,
): Promise<StoredExtract | null> {
  const row = await db
    .prepare("SELECT * FROM extract_jobs WHERE id = ?")
    .bind(id)
    .first<ExtractJobRow>();
  return row ? rowToExtract(row) : null;
}

/** Get team_id for auth check (lightweight). */
export async function getExtractTeamId(
  db: D1Database,
  id: string,
): Promise<{ teamId: string } | null> {
  const row = await db
    .prepare("SELECT team_id FROM extract_jobs WHERE id = ?")
    .bind(id)
    .first<{ team_id: string }>();
  return row ? { teamId: row.team_id } : null;
}

/** Update extract job with success result. */
export async function updateExtractSuccess(
  db: D1Database,
  id: string,
  result: unknown,
  opts?: { warning?: string; creditsBilled?: number; r2Key?: string },
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE extract_jobs
       SET status = 'completed', result = ?, r2_key = ?, warning = ?, credits_billed = ?, updated_at = ?
       WHERE id = ?`,
    )
    .bind(
      JSON.stringify(result),
      opts?.r2Key ?? null,
      opts?.warning ?? null,
      opts?.creditsBilled ?? 0,
      now,
      id,
    )
    .run();
}

/** Update extract job with failure. */
export async function updateExtractFailure(
  db: D1Database,
  id: string,
  error: string,
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE extract_jobs SET status = 'failed', error = ?, updated_at = ? WHERE id = ?`,
    )
    .bind(error, now, id)
    .run();
}

/** Update extract job status to processing. */
export async function markExtractProcessing(
  db: D1Database,
  id: string,
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE extract_jobs SET status = 'processing', updated_at = ? WHERE id = ?`,
    )
    .bind(now, id)
    .run();
}
