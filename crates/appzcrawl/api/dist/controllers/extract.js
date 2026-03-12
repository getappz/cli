/**
 * Extract controller: Firecrawl-compatible /extract endpoints.
 * Adapted from firecrawl/apps/api/src/controllers/v2/extract.ts + extract-status.ts.
 *
 * Flow:
 * - POST /v2/extract → Store job in D1 → Return job ID (async)
 * - GET /v2/extract/:jobId → Look up job in D1 → Return status/result
 *
 * NOTE: Actual LLM extraction is not yet implemented; the job stays in
 * "pending" until a queue consumer or external service picks it up.
 */
import { logger } from "../lib/logger";
import { createExtractJob, getExtractJob, getExtractTeamId, } from "../services/extract-store";
// ---------------------------------------------------------------------------
// POST /v2/extract — create extract job (async, Firecrawl-compatible)
// ---------------------------------------------------------------------------
export async function extractController(c) {
    let rawBody;
    try {
        rawBody = (await c.req.json());
    }
    catch {
        return c.json({ success: false, error: "Invalid JSON body" }, 400);
    }
    const auth = c.get("auth");
    if (!auth) {
        return c.json({ success: false, error: "Unauthorized" }, 401);
    }
    const jobId = crypto.randomUUID();
    // Extract fields from body (Firecrawl-compatible)
    const urls = Array.isArray(rawBody.urls)
        ? rawBody.urls
        : undefined;
    const prompt = typeof rawBody.prompt === "string" ? rawBody.prompt : undefined;
    const schema = rawBody.schema ?? undefined;
    const systemPrompt = typeof rawBody.systemPrompt === "string" ? rawBody.systemPrompt : undefined;
    const webhook = typeof rawBody.webhook === "string" ? rawBody.webhook : undefined;
    // Must have either urls or prompt
    if (!urls?.length && !prompt) {
        return c.json({ success: false, error: "Either 'urls' or 'prompt' is required" }, 400);
    }
    try {
        await createExtractJob(c.env.DB, {
            id: jobId,
            teamId: auth.team_id,
            urls,
            prompt,
            schemaJson: schema,
            systemPrompt,
            options: {
                limit: rawBody.limit,
                ignoreSitemap: rawBody.ignoreSitemap,
                includeSubdomains: rawBody.includeSubdomains,
                allowExternalLinks: rawBody.allowExternalLinks,
                enableWebSearch: rawBody.enableWebSearch,
            },
            webhook,
        });
    }
    catch (e) {
        logger.error("[extract] failed to create job", {
            jobId,
            error: e instanceof Error ? e.message : String(e),
        });
        return c.json({ success: false, error: "Failed to create extract job" }, 500);
    }
    logger.info("[extract] job created", { jobId, teamId: auth.team_id });
    return c.json({ success: true, id: jobId }, 200);
}
// ---------------------------------------------------------------------------
// GET /v2/extract/:jobId — poll extract status
// ---------------------------------------------------------------------------
export async function extractStatusController(c) {
    const jobId = c.req.param("jobId");
    if (!jobId) {
        return c.json({ success: false, error: "Missing jobId" }, 400);
    }
    const auth = c.get("auth");
    if (!auth) {
        return c.json({ success: false, error: "Unauthorized" }, 401);
    }
    // Auth check
    const jobTeam = await getExtractTeamId(c.env.DB, jobId);
    if (!jobTeam) {
        return c.json({ success: false, error: "Job not found." }, 404);
    }
    if (jobTeam.teamId !== auth.team_id) {
        return c.json({ success: false, error: "You are not allowed to access this resource." }, 403);
    }
    const job = await getExtractJob(c.env.DB, jobId);
    if (!job) {
        return c.json({ success: false, error: "Job not found." }, 404);
    }
    if (job.status === "failed") {
        return c.json({
            success: false,
            status: "failed",
            error: job.error ?? "Extract failed",
            id: jobId,
        });
    }
    if (job.status === "completed") {
        const creditsUsed = job.creditsBilled ?? 1;
        return c.json({
            success: true,
            status: "completed",
            data: job.result,
            warning: job.warning ?? undefined,
            creditsUsed,
            expiresAt: job.expiresAt,
            cacheState: "miss",
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
//# sourceMappingURL=extract.js.map