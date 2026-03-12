/**
 * Agent store: D1 operations for agent_jobs table.
 * Adapted from Firecrawl's agent controller + Supabase agents table.
 */
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function rowToAgent(row) {
    return {
        id: row.id,
        teamId: row.team_id,
        status: row.status,
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
export async function createAgentJob(db, params) {
    const now = new Date().toISOString();
    const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
    await db
        .prepare(`INSERT INTO agent_jobs
        (id, team_id, status, prompt, urls, schema_json, model, options, webhook, zero_data_retention, created_at, updated_at, expires_at)
       VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
        .bind(params.id, params.teamId, params.prompt, params.urls ? JSON.stringify(params.urls) : null, params.schemaJson ? JSON.stringify(params.schemaJson) : null, params.model ?? null, params.options ? JSON.stringify(params.options) : null, params.webhook ?? null, params.zeroDataRetention ? 1 : 0, now, now, expiresAt)
        .run();
}
/** Get an agent job by ID. */
export async function getAgentJob(db, id) {
    const row = await db
        .prepare("SELECT * FROM agent_jobs WHERE id = ?")
        .bind(id)
        .first();
    return row ? rowToAgent(row) : null;
}
/** Get team_id for auth check (lightweight). */
export async function getAgentTeamId(db, id) {
    const row = await db
        .prepare("SELECT team_id FROM agent_jobs WHERE id = ?")
        .bind(id)
        .first();
    return row ? { teamId: row.team_id } : null;
}
/** Update agent job with success result. */
export async function updateAgentSuccess(db, id, result, opts) {
    const now = new Date().toISOString();
    await db
        .prepare(`UPDATE agent_jobs
       SET status = 'completed', result = ?, r2_key = ?, credits_billed = ?, updated_at = ?
       WHERE id = ?`)
        .bind(JSON.stringify(result), opts?.r2Key ?? null, opts?.creditsBilled ?? 0, now, id)
        .run();
}
/** Update agent job with failure. */
export async function updateAgentFailure(db, id, error) {
    const now = new Date().toISOString();
    await db
        .prepare(`UPDATE agent_jobs SET status = 'failed', error = ?, updated_at = ? WHERE id = ?`)
        .bind(error, now, id)
        .run();
}
/** Mark agent job as processing. */
export async function markAgentProcessing(db, id) {
    const now = new Date().toISOString();
    await db
        .prepare(`UPDATE agent_jobs SET status = 'processing', updated_at = ? WHERE id = ?`)
        .bind(now, id)
        .run();
}
//# sourceMappingURL=agent-store.js.map