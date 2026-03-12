/**
 * Resolve raw link hrefs to absolute URLs (Firecrawl-compatible).
 * Uses baseUrl and optional baseHref from <base> for relative resolution.
 */

export function resolveUrlWithBaseHref(
  href: string,
  baseUrl: string,
  baseHref: string,
): string {
  let resolutionBase = baseUrl;

  if (baseHref) {
    try {
      new URL(baseHref);
      resolutionBase = baseHref;
    } catch {
      try {
        resolutionBase = new URL(baseHref, baseUrl).href;
      } catch {
        resolutionBase = baseUrl;
      }
    }
  }

  try {
    if (href.startsWith("http://") || href.startsWith("https://")) {
      return href;
    }
    if (href.startsWith("mailto:")) {
      return href;
    }
    if (href.startsWith("#")) {
      return "";
    }
    return new URL(href, resolutionBase).href;
  } catch {
    return "";
  }
}

/** Resolve raw hrefs to absolute URLs and dedupe (Firecrawl links format). */
export function resolveLinks(
  rawHrefs: string[],
  baseUrl: string,
  baseHref: string,
): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const href of rawHrefs) {
    const trimmed = href.trim();
    if (!trimmed) continue;
    const resolved = resolveUrlWithBaseHref(trimmed, baseUrl, baseHref);
    if (resolved && !seen.has(resolved)) {
      seen.add(resolved);
      out.push(resolved);
    }
  }
  return out;
}
