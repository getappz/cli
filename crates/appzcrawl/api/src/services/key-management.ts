/**
 * API Key management service — generate, rotate, revoke, list.
 *
 * Adapted from Firecrawl's:
 *   - controllers/v0/admin/rotate-api-key.ts (key rotation)
 *   - Supabase api_keys table interactions
 *
 * Only the SHA-256 hash is stored — the plaintext key is never persisted.
 */

import { generateApiKey } from "../lib/api-key";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Generate a new API key
// ---------------------------------------------------------------------------

/**
 * Generate and store a new API key for a team.
 *
 * @param db        D1Database binding
 * @param teamId    Team to create the key for
 * @param opts      Optional key configuration
 * @returns         The full key (to show user once) + record metadata
 */
export async function createApiKey(
  db: D1Database,
  teamId: string,
  opts: {
    name?: string;
    scopes?: string[];
    expiresAt?: string;
    createdBy?: string;
  } = {},
): Promise<GeneratedKey> {
  const { fullKey, keyHash, keyPrefix } = await generateApiKey();
  const now = new Date().toISOString();

  const result = await db
    .prepare(
      `INSERT INTO api_keys
        (key_hash, key_prefix, team_id, name, scopes, expires_at, created_by, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      keyHash,
      keyPrefix,
      teamId,
      opts.name ?? "Default",
      opts.scopes ? JSON.stringify(opts.scopes) : null,
      opts.expiresAt ?? null,
      opts.createdBy ?? null,
      now,
    )
    .run();

  const id = result.meta.last_row_id;

  return {
    fullKey,
    keyPrefix,
    id: Number(id),
  };
}

// ---------------------------------------------------------------------------
// Rotate an API key (create new + soft-delete old)
// ---------------------------------------------------------------------------

/**
 * Rotate an API key: create a new key and soft-delete the old one.
 *
 * Adapted from Firecrawl's rotate-api-key.ts pattern (create-then-delete).
 */
export async function rotateApiKey(
  db: D1Database,
  oldKeyId: number,
  teamId: string,
): Promise<GeneratedKey> {
  // 1. Verify the old key belongs to this team and is active
  const oldKey = await db
    .prepare(
      "SELECT id, name, scopes, expires_at, created_by FROM api_keys WHERE id = ? AND team_id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(oldKeyId, teamId)
    .first<{
      id: number;
      name: string | null;
      scopes: string | null;
      expires_at: string | null;
      created_by: string | null;
    }>();

  if (!oldKey) {
    throw new Error("API key not found or does not belong to this team");
  }

  // 2. Create new key with the same name/scopes
  const newKey = await createApiKey(db, teamId, {
    name: oldKey.name ?? "Rotated Key",
    scopes: oldKey.scopes ? JSON.parse(oldKey.scopes) : undefined,
    expiresAt: oldKey.expires_at ?? undefined,
    createdBy: oldKey.created_by ?? undefined,
  });

  // 3. Soft-delete old key
  await revokeApiKey(db, oldKeyId, teamId);

  return newKey;
}

// ---------------------------------------------------------------------------
// Revoke (soft-delete) an API key
// ---------------------------------------------------------------------------

/**
 * Revoke an API key by soft-deleting it (sets deleted_at).
 */
export async function revokeApiKey(
  db: D1Database,
  keyId: number,
  teamId: string,
): Promise<boolean> {
  const result = await db
    .prepare(
      "UPDATE api_keys SET deleted_at = ? WHERE id = ? AND team_id = ? AND deleted_at IS NULL",
    )
    .bind(new Date().toISOString(), keyId, teamId)
    .run();

  return (result.meta.changes ?? 0) > 0;
}

// ---------------------------------------------------------------------------
// List API keys for a team
// ---------------------------------------------------------------------------

/**
 * List all active API keys for a team.
 * Returns display-safe info (never the full key).
 */
export async function listApiKeys(
  db: D1Database,
  teamId: string,
): Promise<ApiKeyRecord[]> {
  const rows = await db
    .prepare(
      `SELECT id, key_prefix, team_id, name, scopes, last_used_at, expires_at, created_by, created_at
       FROM api_keys
       WHERE team_id = ? AND deleted_at IS NULL
       ORDER BY created_at DESC`,
    )
    .bind(teamId)
    .all<{
      id: number;
      key_prefix: string;
      team_id: string;
      name: string | null;
      scopes: string | null;
      last_used_at: string | null;
      expires_at: string | null;
      created_by: string | null;
      created_at: string;
    }>();

  return rows.results.map((r) => ({
    id: r.id,
    keyPrefix: r.key_prefix,
    teamId: r.team_id,
    name: r.name ?? "Default",
    scopes: r.scopes ? JSON.parse(r.scopes) : null,
    lastUsedAt: r.last_used_at,
    expiresAt: r.expires_at,
    createdBy: r.created_by,
    createdAt: r.created_at,
  }));
}

// ---------------------------------------------------------------------------
// Provision a team (create team + credits + first key)
// ---------------------------------------------------------------------------

/**
 * Provision a new team with credits and an initial API key.
 * Convenience function for onboarding.
 */
export async function provisionTeam(
  db: D1Database,
  opts: {
    teamId: string;
    teamName?: string;
    initialCredits?: number;
    createdBy?: string;
  },
): Promise<GeneratedKey> {
  const now = new Date().toISOString();

  // 1. Create team record
  await db
    .prepare(
      `INSERT OR IGNORE INTO teams (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)`,
    )
    .bind(opts.teamId, opts.teamName ?? opts.teamId, now, now)
    .run();

  // 2. Create team credits
  await db
    .prepare(
      `INSERT OR IGNORE INTO team_credits (team_id, credits) VALUES (?, ?)`,
    )
    .bind(opts.teamId, opts.initialCredits ?? 500)
    .run();

  // 3. Generate initial API key
  const key = await createApiKey(db, opts.teamId, {
    name: "Default",
    createdBy: opts.createdBy ?? "system",
  });

  return key;
}
