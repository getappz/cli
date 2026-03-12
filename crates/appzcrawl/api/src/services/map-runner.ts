/**
 * Map runner: discover URLs from sitemap(s) and/or seed page links.
 * Firecrawl-compatible implementation with path filtering, deduplication, and search ranking.
 * Sitemap XML is parsed by the Rust native container (quick-xml); no Node XML deps.
 */

import type { MapDocument, MapRequest } from "../contracts/map";
import { logger } from "../lib/logger";
import {
  checkAndUpdateURLForMap,
  isSameDomain,
  isSameSubdomain,
  removeDuplicateUrls,
} from "../lib/validateUrl";
import type { AppEnv } from "../types";
import { parseSitemap } from "./html-processor";
import { runScrapeUrl } from "./scrape-runner";

const SITEMAP_LIMIT = 50;
const DEFAULT_MAP_TIMEOUT_MS = 60_000;

function normalizeUrl(url: string): string {
  if (!/^https?:\/\//i.test(url)) return `https://${url}`;
  return url;
}

/** Try common sitemap URL for a base URL. */
function getSitemapCandidates(baseUrl: string): string[] {
  try {
    const u = new URL(normalizeUrl(baseUrl));
    const base = `${u.origin}${u.pathname.replace(/\/$/, "")}`;
    return [
      `${base}/sitemap.xml`,
      `${base}/sitemap_index.xml`,
      `${u.origin}/sitemap.xml`,
      `${u.origin}/sitemap_index.xml`,
    ];
  } catch {
    return [];
  }
}

async function fetchSitemapXml(
  env: AppEnv["Bindings"],
  sitemapUrl: string,
  engine?: "native" | "cloudflare" | "auto",
): Promise<string | null> {
  const result = await runScrapeUrl(env, sitemapUrl, {
    formats: ["rawHtml"],
    onlyMainContent: false,
    engine,
  });
  if (result.success && result.document.rawHtml) {
    return result.document.rawHtml;
  }
  return null;
}

async function collectFromSitemap(
  env: AppEnv["Bindings"],
  sitemapUrl: string,
  out: MapDocument[],
  seenSitemaps: Set<string>,
  limit: number,
  _timeoutMs: number,
  engine?: "native" | "cloudflare" | "auto",
): Promise<void> {
  if (seenSitemaps.size >= SITEMAP_LIMIT || out.length >= limit) return;
  if (seenSitemaps.has(sitemapUrl)) return;
  seenSitemaps.add(sitemapUrl);

  const xml = await fetchSitemapXml(env, sitemapUrl, engine);
  if (!xml) return;

  let urls: string[];
  let sitemapUrls: string[];
  try {
    const parsed = await parseSitemap(env, xml);
    urls = parsed.urls ?? [];
    sitemapUrls = parsed.sitemapUrls ?? [];
  } catch {
    return;
  }

  for (const url of urls) {
    if (out.length >= limit) break;
    out.push({ url });
  }

  for (const child of sitemapUrls) {
    if (out.length >= limit || seenSitemaps.size >= SITEMAP_LIMIT) break;
    await collectFromSitemap(
      env,
      child,
      out,
      seenSitemaps,
      limit,
      _timeoutMs,
      engine,
    );
  }
}

/** Score for search relevance (higher = better match). Simple: URL/path contains query words. */
function searchRelevance(url: string, query: string): number {
  const q = query.toLowerCase().replace(/\s+/g, " ");
  const words = q.split(" ").filter(Boolean);
  const urlLower = url.toLowerCase();
  let score = 0;
  for (const w of words) {
    if (urlLower.includes(w)) score += 1;
  }
  return score;
}

export interface MapRunnerResult {
  success: true;
  links: MapDocument[];
}

/**
 * Convert a path pattern (glob-like or regex) to a regex for matching.
 * Supports * and ** wildcards when regexOnFullURL is false.
 */
function patternToRegex(pattern: string, regexOnFullURL: boolean): RegExp {
  if (regexOnFullURL) {
    // Pattern is already a regex
    return new RegExp(pattern);
  }
  // Convert glob-like pattern to regex
  // * matches any characters except /
  // ** matches any characters including /
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, "\\$&") // Escape regex special chars except * and ?
    .replace(/\*\*/g, "<<<DOUBLE_STAR>>>") // Placeholder for **
    .replace(/\*/g, "[^/]*") // * matches non-slash chars
    .replace(/<<<DOUBLE_STAR>>>/g, ".*") // ** matches anything
    .replace(/\?/g, "."); // ? matches single char
  return new RegExp(`^${escaped}$`);
}

/**
 * Check if a URL matches include/exclude path patterns.
 */
function matchesPathPatterns(
  url: string,
  includePaths: string[],
  excludePaths: string[],
  regexOnFullURL: boolean,
): boolean {
  const testTarget = regexOnFullURL ? url : new URL(url).pathname;

  // If excludePaths match, exclude the URL
  if (excludePaths.length > 0) {
    for (const pattern of excludePaths) {
      try {
        const regex = patternToRegex(pattern, regexOnFullURL);
        if (regex.test(testTarget)) {
          return false;
        }
      } catch {
        // Invalid pattern, skip
      }
    }
  }

  // If includePaths is empty, include all (that weren't excluded)
  if (includePaths.length === 0) {
    return true;
  }

  // If includePaths specified, URL must match at least one
  for (const pattern of includePaths) {
    try {
      const regex = patternToRegex(pattern, regexOnFullURL);
      if (regex.test(testTarget)) {
        return true;
      }
    } catch {
      // Invalid pattern, skip
    }
  }

  return false;
}

/**
 * Deduplicate MapDocument array, preferring entries with titles.
 */
function dedupeMapDocumentArray(documents: MapDocument[]): MapDocument[] {
  const urlMap = new Map<string, MapDocument>();

  for (const doc of documents) {
    const existing = urlMap.get(doc.url);

    if (!existing) {
      urlMap.set(doc.url, doc);
    } else if (doc.title !== undefined && existing.title === undefined) {
      // Prefer doc with title
      urlMap.set(doc.url, doc);
    }
  }

  return Array.from(urlMap.values());
}

export async function getMapResults(
  env: AppEnv["Bindings"],
  options: MapRequest & { abort?: AbortSignal },
): Promise<MapRunnerResult> {
  const url = normalizeUrl(options.url);
  const limit = options.limit ?? 5000;
  const timeoutMs = options.timeout ?? DEFAULT_MAP_TIMEOUT_MS;
  // abort signal is passed through options but not currently used in this implementation
  const _abort = options.abort;

  // URL filtering options
  const includeSubdomains = options.includeSubdomains ?? true;
  const allowSubdomains = options.allowSubdomains ?? false;
  const allowExternalLinks = options.allowExternalLinks ?? false;
  const ignoreQueryParameters = options.ignoreQueryParameters ?? true;
  const filterByPath = options.filterByPath ?? true;

  // Path patterns
  const includePaths = options.includePaths ?? [];
  const excludePaths = options.excludePaths ?? [];
  const regexOnFullURL = options.regexOnFullURL ?? false;

  // Deduplication
  const deduplicateSimilarURLs = options.deduplicateSimilarURLs ?? true;

  const sitemapMode = options.sitemap ?? "include";

  const links: MapDocument[] = [];

  logger.info("[map] starting", {
    url,
    sitemap: sitemapMode,
    limit,
    includeSubdomains,
    allowExternalLinks,
    filterByPath,
    includePathsCount: includePaths.length,
    excludePathsCount: excludePaths.length,
  });

  const engine = options.engine;

  if (sitemapMode === "only" || sitemapMode === "include") {
    const seenSitemaps = new Set<string>();
    const candidates = getSitemapCandidates(url);
    for (const sitemapUrl of candidates) {
      if (links.length >= limit) break;
      await collectFromSitemap(
        env,
        sitemapUrl,
        links,
        seenSitemaps,
        limit,
        timeoutMs,
        engine,
      );
    }
  }

  if (
    sitemapMode === "skip" ||
    (sitemapMode === "include" && links.length === 0)
  ) {
    const result = await runScrapeUrl(env, url, {
      formats: ["markdown"],
      onlyMainContent: false,
      engine,
    });
    if (result.success && result.document.links?.length) {
      const seedDoc: MapDocument = { url };
      if (!links.some((l) => l.url === url)) links.unshift(seedDoc);
      for (const link of result.document.links) {
        if (links.length >= limit) break;
        const trimmed = link.trim();
        if (trimmed && !links.some((l) => l.url === trimmed)) {
          links.push({ url: trimmed });
        }
      }
    } else if (links.length === 0 && result.success) {
      links.push({ url });
    } else if (links.length === 0) {
      links.push({ url });
    }
  }

  // Normalize URLs and strip query params if requested
  let mapped: MapDocument[] = [];
  for (const doc of links) {
    try {
      const { url: normalized } = checkAndUpdateURLForMap(
        doc.url,
        ignoreQueryParameters,
      );
      mapped.push({ ...doc, url: normalized.trim() });
    } catch {
      // skip invalid
    }
  }

  // Domain filtering
  if (!allowExternalLinks) {
    mapped = mapped.filter((x) => isSameDomain(x.url, url));
  }

  // Subdomain filtering
  // includeSubdomains=true means include all subdomains of the base domain
  // allowSubdomains is similar but typically used in crawl context
  if (!includeSubdomains && !allowSubdomains) {
    mapped = mapped.filter((x) => isSameSubdomain(x.url, url));
  }

  // Path filtering (only if URL has a significant path)
  if (filterByPath && !allowExternalLinks) {
    try {
      const urlObj = new URL(url);
      const urlPath = urlObj.pathname;
      if (urlPath && urlPath !== "/" && urlPath.length > 1) {
        mapped = mapped.filter((x) => {
          try {
            const linkObj = new URL(x.url);
            return linkObj.pathname.startsWith(urlPath);
          } catch {
            return false;
          }
        });
      }
    } catch {
      // ignore
    }
  }

  // Apply include/exclude path patterns
  if (includePaths.length > 0 || excludePaths.length > 0) {
    mapped = mapped.filter((x) =>
      matchesPathPatterns(x.url, includePaths, excludePaths, regexOnFullURL),
    );
  }

  // Deduplication
  if (deduplicateSimilarURLs) {
    mapped = dedupeMapDocumentArray(mapped);
  } else {
    // Still remove exact duplicates
    const urlsOnly = mapped.map((x) => x.url);
    const deduped = removeDuplicateUrls(urlsOnly);
    mapped = deduped.map((u) => mapped.find((d) => d.url === u) ?? { url: u });
  }

  let final = mapped.slice(0, limit);

  // Sort by search relevance if search query provided
  if (options.search) {
    const searchQuery = options.search;
    final = [...final].sort(
      (a, b) =>
        searchRelevance(b.url, searchQuery) -
        searchRelevance(a.url, searchQuery),
    );
  }

  logger.info("[map] complete", {
    url,
    linksCount: final.length,
    sitemap: sitemapMode,
  });

  return { success: true, links: final };
}
