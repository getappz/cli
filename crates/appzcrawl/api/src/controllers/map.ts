/**
 * POST /v2/map — Firecrawl-compatible map endpoint.
 * Maps multiple URLs from a base URL (sitemap and/or page links).
 */

import type { Context } from "hono";
import { parseMapRequestBody } from "../contracts/map";
import { logger } from "../lib/logger";
import { getMapResults } from "../services/map-runner";
import type { AppEnv } from "../types";

const MAP_TIMEOUT_MS = 60_000;

export async function mapController(c: Context<AppEnv>) {
  let rawBody: unknown;
  try {
    rawBody = await c.req.json();
  } catch {
    return c.json(
      {
        success: false,
        error: "Invalid JSON body; expected map request object",
      },
      400,
    );
  }

  const parsed = parseMapRequestBody(rawBody);
  if (!parsed.ok) {
    return c.json({ success: false, error: parsed.error }, 400);
  }

  const body = parsed.data;
  const timeoutMs = body.timeout ?? MAP_TIMEOUT_MS;
  const abort = new AbortController();
  const timeoutId =
    timeoutMs > 0 ? setTimeout(() => abort.abort(), timeoutMs) : undefined;

  try {
    const result = await getMapResults(c.env, {
      ...body,
      abort: abort.signal,
    });

    if (timeoutId) clearTimeout(timeoutId);

    logger.info("[map] success", {
      url: body.url,
      linksCount: result.links.length,
      sitemap: body.sitemap,
    });

    return c.json({
      success: true as const,
      links: result.links,
      cacheState: "miss" as const,
      cachedAt: new Date().toISOString(),
      creditsUsed: 1,
      concurrencyLimited: false,
    });
  } catch (err) {
    if (timeoutId) clearTimeout(timeoutId);
    const message = err instanceof Error ? err.message : "Map request failed";
    if (abort.signal.aborted) {
      return c.json(
        { success: false, error: "Map request timed out", code: "MAP_TIMEOUT" },
        408,
      );
    }
    logger.warn("[map] error", { url: body.url, error: message });
    return c.json({ success: false, error: message }, 500);
  }
}
