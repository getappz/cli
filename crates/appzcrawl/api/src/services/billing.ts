/**
 * Billing service: credit deduction + audit logging using billing_log + team_credits.
 * Adapted from Firecrawl's credit_billing.ts + batch_billing.ts.
 *
 * Simplified for D1 (no Redis batching needed; D1 writes are fast and atomic).
 * Credits are deducted from team_credits and logged to billing_log in one go.
 */

import { logger } from "../lib/logger";

// ---------------------------------------------------------------------------
// Credit calculation (adapted from Firecrawl's scrape-billing.ts)
// ---------------------------------------------------------------------------

export interface CreditCalcInput {
  /** Whether JSON format was requested (5x cost). */
  isJson?: boolean;
  /** Number of PDF pages (1 credit per page after first). */
  pdfPages?: number;
  /** Whether stealth proxy was used (+4 credits). */
  stealthProxy?: boolean;
  /** Zero data retention flag. */
  zeroDataRetention?: boolean;
  /** ZDR cost override from team flags. */
  zdrCost?: number;
}

/** Calculate credits for a single scrape (Firecrawl-compatible). */
export function calculateScrapeCredits(input: CreditCalcInput): number {
  let credits = 1; // Base: 1 credit per document

  if (input.isJson) {
    credits = 5; // JSON format: 5 credits
  }

  if (input.pdfPages && input.pdfPages > 1) {
    credits += input.pdfPages - 1; // +1 per page beyond first
  }

  if (input.stealthProxy) {
    credits += 4; // Stealth proxy surcharge
  }

  if (input.zeroDataRetention) {
    credits += input.zdrCost ?? 1; // ZDR surcharge
  }

  return credits;
}

// ---------------------------------------------------------------------------
// Billing operations
// ---------------------------------------------------------------------------

/**
 * Deduct credits from a team and log the transaction.
 * Atomic: updates team_credits and inserts billing_log in a batch.
 * Returns the new balance, or null if insufficient credits.
 */
export async function billTeam(
  db: D1Database,
  params: {
    teamId: string;
    credits: number;
    jobId?: string;
    jobType?: string;
    reason: string;
  },
): Promise<{ success: boolean; balanceAfter: number }> {
  const { teamId, credits, jobId, jobType, reason } = params;

  if (credits <= 0) {
    return { success: true, balanceAfter: 0 };
  }

  // Read current balance
  const row = await db
    .prepare("SELECT credits FROM team_credits WHERE team_id = ? LIMIT 1")
    .bind(teamId)
    .first<{ credits: number }>();

  const currentBalance = row?.credits ?? 999_999;

  if (currentBalance < credits) {
    logger.warn("[billing] insufficient credits", {
      teamId,
      requested: credits,
      available: currentBalance,
    });
    return { success: false, balanceAfter: currentBalance };
  }

  const newBalance = currentBalance - credits;
  const logId = crypto.randomUUID();
  const now = new Date().toISOString();

  // Batch: deduct credits + insert audit log
  const batch = [
    db
      .prepare("UPDATE team_credits SET credits = ? WHERE team_id = ?")
      .bind(newBalance, teamId),
    db
      .prepare(
        `INSERT INTO billing_log (id, team_id, job_id, job_type, credits, reason, balance_after, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        logId,
        teamId,
        jobId ?? null,
        jobType ?? null,
        credits,
        reason,
        newBalance,
        now,
      ),
  ];

  await db.batch(batch);

  return { success: true, balanceAfter: newBalance };
}

/**
 * Record a credit refund (negative credits in billing_log, add back to balance).
 */
export async function refundCredits(
  db: D1Database,
  params: {
    teamId: string;
    credits: number;
    jobId?: string;
    jobType?: string;
    reason: string;
  },
): Promise<void> {
  const { teamId, credits, jobId, jobType, reason } = params;
  const logId = crypto.randomUUID();
  const now = new Date().toISOString();

  const batch = [
    db
      .prepare(
        "UPDATE team_credits SET credits = credits + ? WHERE team_id = ?",
      )
      .bind(credits, teamId),
    db
      .prepare(
        `INSERT INTO billing_log (id, team_id, job_id, job_type, credits, reason, balance_after, created_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL, ?)`,
      )
      .bind(
        logId,
        teamId,
        jobId ?? null,
        jobType ?? null,
        -credits,
        reason,
        now,
      ),
  ];

  await db.batch(batch);
}
