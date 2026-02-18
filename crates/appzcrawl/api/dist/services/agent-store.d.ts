/**
 * Agent store: D1 operations for agent_jobs table.
 * Adapted from Firecrawl's agent controller + Supabase agents table.
 */
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
/** Create a new agent job record (pending state). */
export declare function createAgentJob(db: D1Database, params: {
    id: string;
    teamId: string;
    prompt: string;
    urls?: string[];
    schemaJson?: unknown;
    model?: string;
    options?: Record<string, unknown>;
    webhook?: string;
    zeroDataRetention?: boolean;
}): Promise<void>;
/** Get an agent job by ID. */
export declare function getAgentJob(db: D1Database, id: string): Promise<StoredAgent | null>;
/** Get team_id for auth check (lightweight). */
export declare function getAgentTeamId(db: D1Database, id: string): Promise<{
    teamId: string;
} | null>;
/** Update agent job with success result. */
export declare function updateAgentSuccess(db: D1Database, id: string, result: unknown, opts?: {
    creditsBilled?: number;
    r2Key?: string;
}): Promise<void>;
/** Update agent job with failure. */
export declare function updateAgentFailure(db: D1Database, id: string, error: string): Promise<void>;
/** Mark agent job as processing. */
export declare function markAgentProcessing(db: D1Database, id: string): Promise<void>;
//# sourceMappingURL=agent-store.d.ts.map