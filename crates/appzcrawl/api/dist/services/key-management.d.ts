/**
 * API Key management service — generate, rotate, revoke, list.
 *
 * Adapted from Firecrawl's:
 *   - controllers/v0/admin/rotate-api-key.ts (key rotation)
 *   - Supabase api_keys table interactions
 *
 * Only the SHA-256 hash is stored — the plaintext key is never persisted.
 */
export interface ApiKeyRecord {
    id: number;
    /** Display-safe truncated key prefix: "ac-3d47...fd2a" */
    keyPrefix: string;
    teamId: string;
    name: string;
    scopes: string[] | null;
    lastUsedAt: string | null;
    expiresAt: string | null;
    createdBy: string | null;
    createdAt: string;
}
export interface GeneratedKey {
    /** Full API key — show to user ONCE, then discard. */
    fullKey: string;
    /** Display-safe truncated key prefix. */
    keyPrefix: string;
    /** API key record ID. */
    id: number;
}
/**
 * Generate and store a new API key for a team.
 *
 * @param db        D1Database binding
 * @param teamId    Team to create the key for
 * @param opts      Optional key configuration
 * @returns         The full key (to show user once) + record metadata
 */
export declare function createApiKey(db: D1Database, teamId: string, opts?: {
    name?: string;
    scopes?: string[];
    expiresAt?: string;
    createdBy?: string;
}): Promise<GeneratedKey>;
/**
 * Rotate an API key: create a new key and soft-delete the old one.
 *
 * Adapted from Firecrawl's rotate-api-key.ts pattern (create-then-delete).
 */
export declare function rotateApiKey(db: D1Database, oldKeyId: number, teamId: string): Promise<GeneratedKey>;
/**
 * Revoke an API key by soft-deleting it (sets deleted_at).
 */
export declare function revokeApiKey(db: D1Database, keyId: number, teamId: string): Promise<boolean>;
/**
 * List all active API keys for a team.
 * Returns display-safe info (never the full key).
 */
export declare function listApiKeys(db: D1Database, teamId: string): Promise<ApiKeyRecord[]>;
/**
 * Provision a new team with credits and an initial API key.
 * Convenience function for onboarding.
 */
export declare function provisionTeam(db: D1Database, opts: {
    teamId: string;
    teamName?: string;
    initialCredits?: number;
    createdBy?: string;
}): Promise<GeneratedKey>;
//# sourceMappingURL=key-management.d.ts.map