/**
 * Contract tests for Firecrawl-compatible scrape API.
 */

import { describe, expect, it } from "vitest";
import { mapToFirecrawlResponse } from "../../services/scrape-response-mapper";
import type { ScrapeRunnerDocument } from "../../services/scrape-runner";
import {
  parseScrapeRequestBody,
  SCRAPE_DEFAULTS,
  type ScrapeRequestBody,
} from "../scrape";

describe("parseScrapeRequestBody", () => {
  it("accepts minimal request (url only) and applies defaults", () => {
    const result = parseScrapeRequestBody({ url: "https://example.com" });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.url).toBe("https://example.com");
      expect(result.data.formats).toEqual(SCRAPE_DEFAULTS.formats);
      expect(result.data.onlyMainContent).toBe(SCRAPE_DEFAULTS.onlyMainContent);
      expect(result.data.timeout).toBe(SCRAPE_DEFAULTS.timeout);
    }
  });

  it("rejects null or non-object body", () => {
    expect(parseScrapeRequestBody(null).ok).toBe(false);
    expect(parseScrapeRequestBody("string").ok).toBe(false);
    expect(parseScrapeRequestBody(123).ok).toBe(false);
  });

  it("rejects missing or empty url", () => {
    expect(parseScrapeRequestBody({}).ok).toBe(false);
    expect(parseScrapeRequestBody({ url: "" }).ok).toBe(false);
    expect(parseScrapeRequestBody({ url: "   " }).ok).toBe(false);
  });

  it("parses full payload with many options", () => {
    const body = {
      url: "https://example.com/page",
      formats: ["markdown", "html", "links"],
      onlyMainContent: false,
      includeTags: ["article"],
      excludeTags: ["script", "style"],
      maxAge: 172800000,
      timeout: 60000,
      useFireEngine: true,
    };
    const result = parseScrapeRequestBody(body);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.url).toBe("https://example.com/page");
      expect(result.data.formats).toEqual(["markdown", "html", "links"]);
      expect(result.data.onlyMainContent).toBe(false);
      expect(result.data.includeTags).toEqual(["article"]);
      expect(result.data.excludeTags).toEqual(["script", "style"]);
      expect(result.data.maxAge).toBe(172800000);
      expect(result.data.timeout).toBe(60000);
      expect(result.data.useFireEngine).toBe(true);
    }
  });

  it("filters invalid format strings", () => {
    const result = parseScrapeRequestBody({
      url: "https://x.com",
      formats: ["markdown", "invalid", "html"],
    });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.formats).toEqual(["markdown", "html"]);
    }
  });

  it("ignores unknown keys without failing", () => {
    const result = parseScrapeRequestBody({
      url: "https://example.com",
      unknownKey: "value",
      extra: { nested: true },
    });
    expect(result.ok).toBe(true);
  });

  it("parses engine option", () => {
    const nativeResult = parseScrapeRequestBody({
      url: "https://example.com",
      engine: "native",
    });
    expect(nativeResult.ok).toBe(true);
    if (nativeResult.ok) expect(nativeResult.data.engine).toBe("native");

    const cloudflareResult = parseScrapeRequestBody({
      url: "https://example.com",
      engine: "cloudflare",
    });
    expect(cloudflareResult.ok).toBe(true);
    if (cloudflareResult.ok)
      expect(cloudflareResult.data.engine).toBe("cloudflare");

    const autoResult = parseScrapeRequestBody({
      url: "https://example.com",
      engine: "auto",
    });
    expect(autoResult.ok).toBe(true);
    if (autoResult.ok) expect(autoResult.data.engine).toBe("auto");

    const invalidResult = parseScrapeRequestBody({
      url: "https://example.com",
      engine: "invalid",
    });
    expect(invalidResult.ok).toBe(true);
    if (invalidResult.ok) expect(invalidResult.data.engine).toBeUndefined();
  });
});

describe("mapToFirecrawlResponse", () => {
  const baseDocument: ScrapeRunnerDocument = {
    url: "https://example.com",
    rawHtml: "<html><body>Hello</body></html>",
    html: "<html><body>Hello</body></html>",
    markdown: "Hello",
    metadata: {
      title: "Example",
      statusCode: 200,
      sourceURL: "https://example.com",
    },
    links: ["https://example.com/a", "https://example.com/b"],
    statusCode: 200,
  };

  it("returns Firecrawl-shaped success response with markdown format (Firecrawl: only include requested formats)", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["markdown"],
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.markdown).toBe("Hello");
      expect(response.data.rawHtml).toBeUndefined();
      expect(response.data.links).toBeUndefined();
      expect(response.data.metadata).toBeDefined();
      expect(response.data.metadata?.statusCode).toBe(200);
      expect(response.cacheState).toBe("miss");
      expect(response.cachedAt).toBeDefined();
      expect(response.creditsUsed).toBe(1);
      expect(response.concurrencyLimited).toBe(false);
    }
  });

  it("returns cacheState hit and cachedAt when document has cachedAt in metadata", () => {
    const cachedDoc: ScrapeRunnerDocument = {
      ...baseDocument,
      metadata: {
        ...baseDocument.metadata,
        cachedAt: "2026-02-11T14:43:55.001Z",
      },
    };
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["markdown"],
    };
    const response = mapToFirecrawlResponse(cachedDoc, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.cacheState).toBe("hit");
      expect(response.cachedAt).toBe("2026-02-11T14:43:55.001Z");
      expect(response.creditsUsed).toBe(1);
      expect(response.concurrencyLimited).toBe(false);
    }
  });

  it("includes only requested formats", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["html", "links"],
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.html).toBe(baseDocument.html);
      expect(response.data.links).toEqual(baseDocument.links);
      expect(response.data.markdown).toBeUndefined();
      expect(response.data.rawHtml).toBeUndefined();
    }
  });

  it("includes rawHtml and links only when explicitly requested (Firecrawl parity)", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["markdown", "rawHtml", "links"],
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.markdown).toBe("Hello");
      expect(response.data.rawHtml).toBe(baseDocument.rawHtml);
      expect(response.data.links).toEqual(baseDocument.links);
    }
  });

  it("adds warning for unsupported formats", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["markdown", "screenshot", "changeTracking"],
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.warning).toBeDefined();
      expect(response.data.warning).toContain("screenshot");
      expect(response.data.warning).toContain("changeTracking");
    }
  });

  it("adds warning for unsupported request options", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["markdown"],
      actions: [{ type: "wait", milliseconds: 1000 }],
      location: { country: "US" },
      maxAge: 1000,
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.warning).toBeDefined();
    }
  });

  it("returns null placeholders for unsupported formats", () => {
    const request: ScrapeRequestBody = {
      url: "https://example.com",
      formats: ["screenshot", "branding", "json"],
    };
    const response = mapToFirecrawlResponse(baseDocument, request);
    expect(response.success).toBe(true);
    if (response.success) {
      expect(response.data.screenshot).toBeNull();
      expect(response.data.branding).toBeNull();
      expect(response.data.llm_extraction).toBeNull();
    }
  });
});
