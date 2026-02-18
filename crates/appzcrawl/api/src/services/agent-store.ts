/**
 * Agent store: D1 operations for agent_jobs table.
 * Adapted from Firecrawl's agent controller + Supabase agents table.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgentJobRow {
  id: string;
  team_id: string;
  status: string;
  prompt: string;
  urls: string | null;
  schema_json: string | null;
  model: string | null;
  options: string | null;
  result: string | null;
  r2_key: string | null;
  error: string | null;
  credits_billed: number;
  webhook: string | null;
  zero_data_retention: number;
  created_at: string;
  updated_at: string;
  expires_at: string;
}

export interface StoredAgent {
  id: string;
  teamId: string;
  status: "pending" | "processing" | "completed" | "failed";
  prompt: string;
  urls: string[] | null;
  schemaJson: unknown | null;
  model: string | null;
  options: Record<string, unknown> | null;
  result: unknown | null;
  r2Key: string | null;
  error: string | null;
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

function rowToAgent(row: AgentJobRow): StoredAgent {
  return {
    id: row.id,
    teamId: row.team_id,
    status: row.status as StoredAgent["status"],
    prompt: row.prompt,
    urls: row.urls ? JSON.parse(row.urls) : null,
    schemaJson: row.schema_json ? JSON.parse(row.schema_json) : null,
    model: row.model,
    options: row.options ? JSON.parse(row.options) : null,
    result: row.result ? JSON.parse(row.result) : null,
    r2Key: row.r2_key,
    error: row.error,
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

/** Create a new agent job record (pending state). */
export async function createAgentJob(
  db: D1Database,
  params: {
    id: string;
    teamId: string;
    prompt: string;
    urls?: string[];
    schemaJson?: unknown;
    model?: string;
    options?: Record<string, unknown>;
    webhook?: string;
    zeroDataRetention?: boolean;
  },
): Promise<void> {
  const now = new Date().toISOString();
  const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();

  await db
    .prepare(
      `INSERT INTO agent_jobs
        (id, team_id, status, prompt, urls, schema_json, model, options, webhook, zero_data_retention, created_at, updated_at, expires_at)
       VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      params.id,
      params.teamId,
      params.prompt,
      params.urls ? JSON.stringify(params.urls) : null,
      params.schemaJson ? JSON.stringify(params.schemaJson) : null,
      params.model ?? null,
      params.options ? JSON.stringify(params.options) : null,
      params.webhook ?? null,
      params.zeroDataRetention ? 1 : 0,
      now,
      now,
      expiresAt,
    )
    .run();
}

/** Get an agent job by ID. */
export async function getAgentJob(
  db: D1Database,
  id: string,
): Promise<StoredAgent | null> {
  const row = await db
    .prepare("SELECT * FROM agent_jobs WHERE id = ?")
    .bind(id)
    .first<AgentJobRow>();
  return row ? rowToAgent(row) : null;
}

/** Get team_id for auth check (lightweight). */
export async function getAgentTeamId(
  db: D1Database,
  id: string,
): Promise<{ teamId: string } | null> {
  const row = await db
    .prepare("SELECT team_id FROM agent_jobs WHERE id = ?")
    .bind(id)
    .first<{ team_id: string }>();
  return row ? { teamId: row.team_id } : null;
}

/** Update agent job with success result. */
export async function updateAgentSuccess(
  db: D1Database,
  id: string,
  result: unknown,
  opts?: { creditsBilled?: number; r2Key?: string },
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE agent_jobs
       SET status = 'completed', result = ?, r2_key = ?, credits_billed = ?, updated_at = ?
       WHERE id = ?`,
    )
    .bind(
      JSON.stringify(result),
      opts?.r2Key ?? null,
      opts?.creditsBilled ?? 0,
      now,
      id,
    )
    .run();
}

/** Update agent job with failure. */
export async function updateAgentFailure(
  db: D1Database,
  id: string,
  error: string,
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE agent_jobs SET status = 'failed', error = ?, updated_at = ? WHERE id = ?`,
    )
    .bind(error, now, id)
    .run();
}

/** Mark agent job as processing. */
export async function markAgentProcessing(
  db: D1Database,
  id: string,
): Promise<void> {
  const now = new Date().toISOString();
  await db
    .prepare(
      `UPDATE agent_jobs SET status = 'processing', updated_at = ? WHERE id = ?`,
    )
    .bind(now, id)
    .run();
}
