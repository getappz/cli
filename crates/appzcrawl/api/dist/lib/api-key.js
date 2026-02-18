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
// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
/** Key prefix identifying coacharena API keys (like Firecrawl's "fc-"). */
export const KEY_PREFIX = "ac-";
// ---------------------------------------------------------------------------
// Key generation
// ---------------------------------------------------------------------------
/**
 * Generate a new API key.
 * Returns the full key (to show the user once), its hash, and display prefix.
 */
export async function generateApiKey() {
    const uuid = crypto.randomUUID();
    const uuidNoDashes = uuid.replaceAll("-", "");
    const fullKey = `${KEY_PREFIX}${uuidNoDashes}`;
    const keyHash = await hashKey(fullKey);
    const keyPrefix = buildKeyPrefix(fullKey);
    return { fullKey, keyHash, keyPrefix };
}
// ---------------------------------------------------------------------------
// Key parsing
// ---------------------------------------------------------------------------
/**
 * Parse an incoming API key token.
 * All keys must use the ac-{hex} format.
 */
export function parseApiKey(token) {
    if (token.startsWith(KEY_PREFIX)) {
        const hex = token.slice(KEY_PREFIX.length);
        const uuid = hexToUuid(hex);
        return { normalizedUuid: uuid, fullKey: token };
    }
    // Bare token — normalize to ac- format for hashing, then validate
    const cleaned = token.replaceAll("-", "");
    const uuid = hexToUuid(cleaned);
    const fullKey = `${KEY_PREFIX}${cleaned}`;
    return { normalizedUuid: uuid, fullKey };
}
// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------
/** SHA-256 hash of a key (hex string). */
export async function hashKey(key) {
    const data = new TextEncoder().encode(key);
    const hash = await crypto.subtle.digest("SHA-256", data);
    return Array.from(new Uint8Array(hash))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
}
// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------
/**
 * Build a display-safe prefix from a full key.
 * Shows first 7 chars + "..." + last 4 chars.
 * Example: "ac-3d47...fd2a"
 */
export function buildKeyPrefix(fullKey) {
    if (fullKey.length <= 14)
        return fullKey;
    return `${fullKey.slice(0, 7)}...${fullKey.slice(-4)}`;
}
// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------
/** Check if a normalized UUID is valid. */
export function isValidUuid(uuid) {
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(uuid);
}
// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------
/** Convert 32 hex chars to UUID format with dashes. */
function hexToUuid(hex) {
    const clean = hex.replace(/[^0-9a-f]/gi, "");
    if (clean.length !== 32)
        return hex; // Can't convert; return as-is
    return `${clean.slice(0, 8)}-${clean.slice(8, 12)}-${clean.slice(12, 16)}-${clean.slice(16, 20)}-${clean.slice(20)}`;
}
//# sourceMappingURL=api-key.js.map