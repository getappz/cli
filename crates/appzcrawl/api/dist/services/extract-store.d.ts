/**
 * Extract store: D1 operations for extract_jobs table.
 * Adapted from Firecrawl's extract-redis.ts — replaces Redis with D1.
 */
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
/** Create a new extract job record (pending state). */
export declare function createExtractJob(db: D1Database, params: {
    id: string;
    teamId: string;
    urls?: string[];
    prompt?: string;
    schemaJson?: unknown;
    systemPrompt?: string;
    options?: Record<string, unknown>;
    webhook?: string;
    zeroDataRetention?: boolean;
}): Promise<void>;
/** Get an extract job by ID. */
export declare function getExtractJob(db: D1Database, id: string): Promise<StoredExtract | null>;
/** Get team_id for auth check (lightweight). */
export declare function getExtractTeamId(db: D1Database, id: string): Promise<{
    teamId: string;
} | null>;
/** Update extract job with success result. */
export declare function updateExtractSuccess(db: D1Database, id: string, result: unknown, opts?: {
    warning?: string;
    creditsBilled?: number;
    r2Key?: string;
}): Promise<void>;
/** Update extract job with failure. */
export declare function updateExtractFailure(db: D1Database, id: string, error: string): Promise<void>;
/** Update extract job status to processing. */
export declare function markExtractProcessing(db: D1Database, id: string): Promise<void>;
//# sourceMappingURL=extract-store.d.ts.map