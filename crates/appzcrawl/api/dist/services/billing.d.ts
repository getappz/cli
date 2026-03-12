/**
 * Billing service: credit deduction + audit logging using billing_log + team_credits.
 * Adapted from Firecrawl's credit_billing.ts + batch_billing.ts.
 *
 * Simplified for D1 (no Redis batching needed; D1 writes are fast and atomic).
 * Credits are deducted from team_credits and logged to billing_log in one go.
 */
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
export declare function calculateScrapeCredits(input: CreditCalcInput): number;
/**
 * Deduct credits from a team and log the transaction.
 * Atomic: updates team_credits and inserts billing_log in a batch.
 * Returns the new balance, or null if insufficient credits.
 */
export declare function billTeam(db: D1Database, params: {
    teamId: string;
    credits: number;
    jobId?: string;
    jobType?: string;
    reason: string;
}): Promise<{
    success: boolean;
    balanceAfter: number;
}>;
/**
 * Record a credit refund (negative credits in billing_log, add back to balance).
 */
export declare function refundCredits(db: D1Database, params: {
    teamId: string;
    credits: number;
    jobId?: string;
    jobType?: string;
    reason: string;
}): Promise<void>;
//# sourceMappingURL=billing.d.ts.map