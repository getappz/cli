/**
 * Agent controller: Firecrawl-compatible /agent endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/agent.ts + agent-status.ts.
 *
 * Flow:
 * - POST /v2/agent → Store job in D1 → Return job ID (async)
 * - GET /v2/agent/:jobId → Look up job in D1 → Return status/result
 * - DELETE /v2/agent/:jobId → Mark job cancelled
 *
 * NOTE: Actual agent execution is not yet implemented; the job stays in
 * "pending" until an external agent service picks it up.
 */

import type { Context } from "hono";
import { logger } from "../lib/logger";
import {
  createAgentJob,
  getAgentJob,
  getAgentTeamId,
  updateAgentFailure,
} from "../services/agent-store";
import type { AppEnv } from "../types";

// ---------------------------------------------------------------------------
// POST /v2/agent — create agent job (async, Firecrawl-compatible)
// ---------------------------------------------------------------------------

export async function agentController(c: Context<AppEnv>) {
  let rawBody: Record<string, unknown>;
  try {
    rawBody = (await c.req.json()) as Record<string, unknown>;
  } catch {
    return c.json({ success: false, error: "Invalid JSON body" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const prompt =
    typeof rawBody.prompt === "string" ? rawBody.prompt : undefined;
  if (!prompt) {
    return c.json({ success: false, error: "'prompt' is required" }, 400);
  }

  const jobId = crypto.randomUUID();

  const urls = Array.isArray(rawBody.urls)
    ? (rawBody.urls as string[])
    : undefined;
  const schema = rawBody.schema ?? undefined;
  const model = typeof rawBody.model === "string" ? rawBody.model : undefined;
  const webhook =
    typeof rawBody.webhook === "string" ? rawBody.webhook : undefined;

  try {
    await createAgentJob(c.env.DB, {
      id: jobId,
      teamId: auth.team_id,
      prompt,
      urls,
      schemaJson: schema,
      model,
      options: {
        maxCredits: rawBody.maxCredits,
        strictConstrainToURLs: rawBody.strictConstrainToURLs,
      },
      webhook,
    });
  } catch (e) {
    logger.error("[agent] failed to create job", {
      jobId,
      error: e instanceof Error ? e.message : String(e),
    });
    return c.json({ success: false, error: "Failed to create agent job" }, 500);
  }

  logger.info("[agent] job created", { jobId, teamId: auth.team_id });

  return c.json({ success: true, id: jobId }, 200);
}

// ---------------------------------------------------------------------------
// GET /v2/agent/:jobId — poll agent status
// ---------------------------------------------------------------------------

export async function agentStatusController(c: Context<AppEnv>) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  // Auth check
  const jobTeam = await getAgentTeamId(c.env.DB, jobId);
  if (!jobTeam) {
    return c.json({ success: false, error: "Job not found." }, 404);
  }
  if (jobTeam.teamId !== auth.team_id) {
    return c.json(
      { success: false, error: "You are not allowed to access this resource." },
      403,
    );
  }

  const job = await getAgentJob(c.env.DB, jobId);
  if (!job) {
    return c.json({ success: false, error: "Job not found." }, 404);
  }

  if (job.status === "failed") {
    return c.json({
      success: false,
      status: "failed",
      error: job.error ?? "Agent failed",
      id: jobId,
    });
  }

  if (job.status === "completed") {
    const creditsUsed = job.creditsBilled ?? 1;
    return c.json({
      success: true,
      status: "completed",
      data: job.result,
      creditsUsed,
      expiresAt: job.expiresAt,
      cacheState: "miss" as const,
      cachedAt: new Date().toISOString(),
      concurrencyLimited: false,
    });
  }

  // pending or processing
  return c.json({
    success: true,
    status: job.status === "processing" ? "processing" : "pending",
    id: jobId,
    expiresAt: job.expiresAt,
  });
}

// ---------------------------------------------------------------------------
// DELETE /v2/agent/:jobId — cancel agent job
// ---------------------------------------------------------------------------

export async function agentCancelController(c: Context<AppEnv>) {
  const jobId = c.req.param("jobId");
  if (!jobId) {
    return c.json({ success: false, error: "Missing jobId" }, 400);
  }

  const auth = c.get("auth");
  if (!auth) {
    return c.json({ success: false, error: "Unauthorized" }, 401);
  }

  const jobTeam = await getAgentTeamId(c.env.DB, jobId);
  if (!jobTeam) {
    return c.json({ success: false, error: "Job not found." }, 404);
  }
  if (jobTeam.teamId !== auth.team_id) {
    return c.json(
      { success: false, error: "You are not allowed to access this resource." },
      403,
    );
  }

  // Mark as failed with cancellation message
  await updateAgentFailure(c.env.DB, jobId, "Job cancelled by user");

  return c.json({ success: true, id: jobId });
}
