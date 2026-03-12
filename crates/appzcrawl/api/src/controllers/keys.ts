/**
 * API Key management endpoints.
 *
 * Adapted from Firecrawl's:
 *   - controllers/v0/admin/rotate-api-key.ts
 *   - Supabase api_keys table management
 *
 * All endpoints require authentication — the team_id from auth determines
 * which keys the user can manage.
 */

import type { Context } from "hono";
import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  rotateApiKey,
} from "../services/key-management";
import type { AppEnv } from "../types";

// ---------------------------------------------------------------------------
// POST /v2/keys — Create a new API key
// ---------------------------------------------------------------------------

export async function createKeyController(c: Context<AppEnv>) {
  const auth = c.get("auth");
  if (!auth) return c.json({ success: false, error: "Unauthorized" }, 401);

  const body = await c.req.json<{
    name?: string;
    scopes?: string[];
    expiresAt?: string;
  }>();

  // Validate scopes if provided
  const validScopes = [
    "scrape",
    "crawl",
    "search",
    "map",
    "extract",
    "agent",
    "browser",
  ];
  if (body.scopes) {
    for (const scope of body.scopes) {
      if (!validScopes.includes(scope)) {
        return c.json(
          {
            success: false,
            error: `Invalid scope: "${scope}". Valid scopes: ${validScopes.join(", ")}`,
          },
          400,
        );
      }
    }
  }

  try {
    const result = await createApiKey(c.env.DB, auth.team_id, {
      name: body.name,
      scopes: body.scopes,
      expiresAt: body.expiresAt,
      createdBy: auth.team_id,
    });

    return c.json({
      success: true,
      data: {
        id: result.id,
        key: result.fullKey,
        keyPrefix: result.keyPrefix,
        message: "Save this key securely. It will not be shown again.",
      },
    });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to create key",
      },
      500,
    );
  }
}

// ---------------------------------------------------------------------------
// GET /v2/keys — List all API keys for the authenticated team
// ---------------------------------------------------------------------------

export async function listKeysController(c: Context<AppEnv>) {
  const auth = c.get("auth");
  if (!auth) return c.json({ success: false, error: "Unauthorized" }, 401);

  try {
    const keys = await listApiKeys(c.env.DB, auth.team_id);

    return c.json({
      success: true,
      data: keys,
    });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to list keys",
      },
      500,
    );
  }
}

// ---------------------------------------------------------------------------
// DELETE /v2/keys/:keyId — Revoke an API key
// ---------------------------------------------------------------------------

export async function revokeKeyController(c: Context<AppEnv>) {
  const auth = c.get("auth");
  if (!auth) return c.json({ success: false, error: "Unauthorized" }, 401);

  const keyId = Number.parseInt(c.req.param("keyId"), 10);
  if (Number.isNaN(keyId)) {
    return c.json({ success: false, error: "Invalid key ID" }, 400);
  }

  try {
    const revoked = await revokeApiKey(c.env.DB, keyId, auth.team_id);

    if (!revoked) {
      return c.json(
        { success: false, error: "Key not found or already revoked" },
        404,
      );
    }

    return c.json({ success: true, message: "API key revoked" });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to revoke key",
      },
      500,
    );
  }
}

// ---------------------------------------------------------------------------
// POST /v2/keys/:keyId/rotate — Rotate an API key
// ---------------------------------------------------------------------------

export async function rotateKeyController(c: Context<AppEnv>) {
  const auth = c.get("auth");
  if (!auth) return c.json({ success: false, error: "Unauthorized" }, 401);

  const keyId = Number.parseInt(c.req.param("keyId"), 10);
  if (Number.isNaN(keyId)) {
    return c.json({ success: false, error: "Invalid key ID" }, 400);
  }

  try {
    const result = await rotateApiKey(c.env.DB, keyId, auth.team_id);

    return c.json({
      success: true,
      data: {
        id: result.id,
        key: result.fullKey,
        keyPrefix: result.keyPrefix,
        message:
          "Old key has been revoked. Save this new key securely. It will not be shown again.",
      },
    });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to rotate key",
      },
      500,
    );
  }
}
