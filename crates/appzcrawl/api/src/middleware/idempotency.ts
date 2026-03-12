import type { Context, Next } from "hono";
import type { AppEnv } from "../types";

// UUID v4 regex (Firecrawl requires valid UUID for idempotency key)
const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function isValidUuid(value: string): boolean {
  return UUID_REGEX.test(value.trim());
}

/**
 * Idempotency middleware (Firecrawl-compatible).
 * When x-idempotency-key header is present:
 * - Validates key is a UUID; if not, returns 409.
 * - Checks D1 idempotency_keys table; if key exists, returns 409.
 * - Inserts key before running controller so duplicate requests are rejected.
 */
export async function idempotencyMiddleware(c: Context<AppEnv>, next: Next) {
  const rawKey = c.req.header("x-idempotency-key");
  if (!rawKey || typeof rawKey !== "string") return next();

  const key = rawKey.trim();
  if (!key) return next();

  if (!isValidUuid(key)) {
    return c.json(
      {
        success: false,
        error: "Invalid idempotency key; must be a valid UUID",
      },
      409,
    );
  }

  try {
    const existing = await c.env.DB.prepare(
      "SELECT 1 FROM idempotency_keys WHERE idempotency_key = ? LIMIT 1",
    )
      .bind(key)
      .first();

    if (existing) {
      return c.json(
        { success: false, error: "Idempotency key already used" },
        409,
      );
    }

    // Claim the key (insert) before running controller.
    // On race: second request may pass SELECT, then INSERT throws due to UNIQUE constraint.
    const now = new Date().toISOString();
    await c.env.DB.prepare(
      "INSERT INTO idempotency_keys (idempotency_key, created_at) VALUES (?, ?)",
    )
      .bind(key, now)
      .run();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (
      msg.includes("UNIQUE") ||
      msg.includes("unique") ||
      msg.includes("constraint")
    ) {
      return c.json(
        { success: false, error: "Idempotency key already used" },
        409,
      );
    }
    throw e;
  }

  return next();
}
