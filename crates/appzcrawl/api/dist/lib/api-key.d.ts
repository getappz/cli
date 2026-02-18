/**
 * API Key format, generation, and parsing.
 * Adapted from Firecrawl's parseApi.ts + key generation patterns.
 *
 * Key format: `ac-{uuid-without-dashes}`
 *   - Prefix "ac-" identifies this as an appzcrawl/coacharena key
 *   - Body is a UUID v4 with dashes removed (32 hex chars)
 *   - Example: ac-3d478a296e59403e85c794aba81ffd2a
 *
 * Storage: Only the SHA-256 hash of the full key is stored (key_hash column).
 *          The plaintext key is never persisted.
 * Display: Truncated prefix for UI display (key_prefix column).
 *   - Example: ac-3d47...fd2a
 */
/** Key prefix identifying coacharena API keys (like Firecrawl's "fc-"). */
export declare const KEY_PREFIX = "ac-";
/**
 * Generate a new API key.
 * Returns the full key (to show the user once), its hash, and display prefix.
 */
export declare function generateApiKey(): Promise<{
    /** Full key to give to the user (only shown once). Never stored. */
    fullKey: string;
    /** SHA-256 hex hash for storage in key_hash column. */
    keyHash: string;
    /** Display-safe truncated prefix: ac-xxxx...yyyy */
    keyPrefix: string;
}>;
/**
 * Parse an incoming API key token.
 * All keys must use the ac-{hex} format.
 */
export declare function parseApiKey(token: string): {
    /** Normalized UUID (with dashes) for format validation. */
    normalizedUuid: string;
    /** The full key as provided (for hashing). */
    fullKey: string;
};
/** SHA-256 hash of a key (hex string). */
export declare function hashKey(key: string): Promise<string>;
/**
 * Build a display-safe prefix from a full key.
 * Shows first 7 chars + "..." + last 4 chars.
 * Example: "ac-3d47...fd2a"
 */
export declare function buildKeyPrefix(fullKey: string): string;
/** Check if a normalized UUID is valid. */
export declare function isValidUuid(uuid: string): boolean;
//# sourceMappingURL=api-key.d.ts.map