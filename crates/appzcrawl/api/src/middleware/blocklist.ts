/**
 * URL blocklist middleware using D1 url_blocklist table.
 * Adapted from firecrawl/apps/api/src/scraper/WebScraper/utils/blocklist.ts.
 *
 * Loads blocklist from D1 on first use, caches in memory for the worker lifetime.
 * Checks exact domain matches and subdomain matches.
 */

import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
import { UNSUPPORTED_SITE_MESSAGE } from "../types";

// ---------------------------------------------------------------------------
// In-memory cache (per-isolate, refreshed on cold start)
// ---------------------------------------------------------------------------

let cachedPatterns: string[] | null = null;
let cacheLoadedAt = 0;
const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

async function loadBlocklist(db: D1Database): Promise<string[]> {
  const now = Date.now();
  if (cachedPatterns && now - cacheLoadedAt < CACHE_TTL_MS) {
    return cachedPatterns;
  }

  try {
    const { results } = await db
      .prepare("SELECT pattern FROM url_blocklist")
      .all<{ pattern: string }>();
    cachedPatterns = (results ?? []).map((r) => r.pattern.toLowerCase());
    cacheLoadedAt = now;
  } catch {
    // Table may not exist yet or be empty
    cachedPatterns = [];
    cacheLoadedAt = now;
  }

  return cachedPatterns;
}

// ---------------------------------------------------------------------------
// Blocking logic (adapted from Firecrawl)
// ---------------------------------------------------------------------------

function extractDomain(url: string): string | null {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
}

/**
 * Check if a URL is blocked by the blocklist.
 * Matches:
 *   - Exact domain matches (e.g. "example.com" blocks "example.com")
 *   - Subdomain matches (e.g. "example.com" blocks "sub.example.com")
 */
export async function isUrlBlocked(
  url: string,
  db: D1Database,
): Promise<boolean> {
  const domain = extractDomain(url);
  if (!domain) return false;

  const patterns = await loadBlocklist(db);
  if (patterns.length === 0) return false;

  for (const pattern of patterns) {
    // Exact match
    if (domain === pattern) return true;
    // Subdomain match (e.g. domain "sub.example.com" matches pattern "example.com")
    if (domain.endsWith(`.${pattern}`)) return true;
  }

  return false;
}

/**
 * Route-level middleware: no-op passthrough for routes that don't have a URL yet.
 * Controllers that parse a body call checkBlocklist() directly.
 */
export async function blocklistMiddleware(_c: Context<AppEnv>, next: Next) {
  return next();
}

/**
 * Call from controller when you have parsed body to return 403 if URL is blocked.
 * Now uses D1 for the blocklist lookup.
 */
export async function checkBlocklist(
  body: { url?: string } | null,
  db: D1Database,
): Promise<{ blocked: boolean; error?: string }> {
  const url = typeof body?.url === "string" ? body.url : null;
  if (!url) return { blocked: false };

  const blocked = await isUrlBlocked(url, db);
  if (blocked) return { blocked: true, error: UNSUPPORTED_SITE_MESSAGE };
  return { blocked: false };
}
