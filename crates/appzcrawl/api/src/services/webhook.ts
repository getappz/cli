/**
 * Webhook delivery service with webhook_logs audit trail.
 * Adapted from Firecrawl's services/webhook/delivery.ts.
 *
 * Sends webhook events for crawl/batch-scrape/extract completion
 * and logs delivery attempts to D1 webhook_logs table.
 */

import { logger } from "../lib/logger";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type WebhookEventType =
  | "crawl.started"
  | "crawl.page"
  | "crawl.completed"
  | "crawl.failed"
  | "batch_scrape.page"
  | "batch_scrape.completed"
  | "extract.completed"
  | "extract.failed"
  | "agent.completed"
  | "agent.failed";

export interface WebhookPayload {
  success: boolean;
  type: WebhookEventType;
  id: string;
  data?: unknown;
  error?: string;
  [key: string]: unknown;
}

export interface WebhookDeliveryParams {
  /** Webhook destination URL. */
  url: string;
  /** Team that owns the parent job. */
  teamId: string;
  /** Parent job ID. */
  jobId: string;
  /** Job type: crawl | batch_scrape | extract | agent. */
  jobType: string;
  /** Event type. */
  event: WebhookEventType;
  /** Payload to send. */
  payload: WebhookPayload;
  /** HMAC secret for signing (from team config). */
  hmacSecret?: string | null;
}

// ---------------------------------------------------------------------------
// HMAC signing (Firecrawl-compatible)
// ---------------------------------------------------------------------------

async function signPayload(payload: string, secret: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(payload),
  );
  const hex = Array.from(new Uint8Array(sig))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return `sha256=${hex}`;
}

// ---------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------

const WEBHOOK_TIMEOUT_MS = 10_000; // 10 seconds (Firecrawl v2 default)

/**
 * Deliver a webhook and log the result to D1.
 * Returns true if delivery was successful.
 */
export async function deliverWebhook(
  db: D1Database,
  params: WebhookDeliveryParams,
): Promise<boolean> {
  const body = JSON.stringify(params.payload);
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

  // HMAC signature (Firecrawl-compatible)
  if (params.hmacSecret) {
    headers["X-Firecrawl-Signature"] = await signPayload(
      body,
      params.hmacSecret,
    );
  }

  let statusCode: number | null = null;
  let success = false;
  let responseBody: string | null = null;
  let error: string | null = null;

  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), WEBHOOK_TIMEOUT_MS);

    const response = await fetch(params.url, {
      method: "POST",
      headers,
      body,
      signal: controller.signal,
    });

    clearTimeout(timeout);
    statusCode = response.status;
    success = response.ok;

    // Capture response body for debugging (truncated)
    try {
      const text = await response.text();
      responseBody = text.slice(0, 1000);
    } catch {
      // Ignore response body read failures
    }
  } catch (e) {
    error = e instanceof Error ? e.message : String(e);
    if (error.includes("abort")) {
      error = `Webhook timeout after ${WEBHOOK_TIMEOUT_MS}ms`;
    }
  }

  // Log to webhook_logs table
  try {
    const logId = crypto.randomUUID();
    const now = new Date().toISOString();

    await db
      .prepare(
        `INSERT INTO webhook_logs
          (id, team_id, job_id, job_type, event, url, status_code, success, attempt, request_body, response_body, error, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?)`,
      )
      .bind(
        logId,
        params.teamId,
        params.jobId,
        params.jobType,
        params.event,
        params.url,
        statusCode,
        success ? 1 : 0,
        // Don't store full request body for large payloads (truncate to 1KB)
        body.length > 1000 ? `${body.slice(0, 1000)}...` : body,
        responseBody,
        error,
        now,
      )
      .run();
  } catch (e) {
    // Best-effort logging
    logger.warn("[webhook] failed to log delivery", {
      error: e instanceof Error ? e.message : String(e),
      jobId: params.jobId,
    });
  }

  if (!success) {
    logger.warn("[webhook] delivery failed", {
      url: params.url,
      statusCode,
      error,
      jobId: params.jobId,
    });
  }

  return success;
}

/**
 * Send crawl completion webhook if configured.
 * Call via waitUntil() to not block the response.
 */
export async function sendCrawlWebhook(
  db: D1Database,
  params: {
    webhookUrl: string;
    teamId: string;
    crawlId: string;
    event: "crawl.completed" | "crawl.failed" | "crawl.page";
    data?: unknown;
    error?: string;
    hmacSecret?: string | null;
  },
): Promise<void> {
  await deliverWebhook(db, {
    url: params.webhookUrl,
    teamId: params.teamId,
    jobId: params.crawlId,
    jobType: "crawl",
    event: params.event,
    payload: {
      success: params.event !== "crawl.failed",
      type: params.event,
      id: params.crawlId,
      data: params.data,
      error: params.error,
    },
    hmacSecret: params.hmacSecret,
  });
}
