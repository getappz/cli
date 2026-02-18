/**
 * Benchmark controller — measures performance of the three HTML processing backends.
 *
 * POST /v2/benchmark
 *   Body: { html?, url?, iterations?, functions?, warmup? }
 *
 * Backends benchmarked:
 *   1. **wasm** — firecrawl_rs compiled to WASM, running in-Worker (sub-ms).
 *   2. **engine** — appzcrawl-engine Worker via RPC Service Binding (pure Rust).
 *   3. **container** — Cloudflare Container running native Rust via Axum.
 *
 * ## Timer behavior (Spectre mitigation)
 *
 * Per Cloudflare docs (workers/runtime-apis/performance/): performance.now() and
 * Date.now() **only advance after I/O occurs** when deployed. CPU-bound work
 * (WASM, some Engine RPC) will show 0ms because the timer is frozen until fetch,
 * KV, R2, etc. Run locally (`wrangler dev`) to measure CPU-bound timings.
 *
 * Only available in devel environment (or when ENVIRONMENT is unset, i.e. local dev).
 */

import type { Context } from "hono";
import * as engine from "../services/engine-processor";
import * as container from "../services/native-container";
import * as wasmNative from "../services/wasm-processor";
import {
  getWasmDiagnostics,
  getInitError as getWasmInitError,
} from "../services/wasm-processor";
import type { AppEnv } from "../types";

// ---------------------------------------------------------------------------
// Timing helper — microsecond precision
// ---------------------------------------------------------------------------

interface TimingResult {
  /** Average time in ms (4 decimal places = 0.1us resolution). */
  avg: number;
  min: number;
  max: number;
  p50: number;
  p95: number;
  /** Individual run times in ms. */
  runs: number[];
  /** Human-readable average (e.g. "0.0423ms" or "42.3us"). */
  avgHuman: string;
  error?: string;
}

async function time(
  fn: () => unknown | Promise<unknown>,
  iterations: number,
  warmupRuns: number,
): Promise<TimingResult> {
  // Warmup: discard first N runs (JIT, allocator warmup, cold starts)
  for (let i = 0; i < warmupRuns; i++) {
    try {
      await fn();
    } catch {
      // ignore warmup errors
    }
  }

  const runs: number[] = [];
  let lastError: string | undefined;

  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    try {
      await fn();
    } catch (e) {
      lastError = e instanceof Error ? e.message : String(e);
    }
    runs.push(performance.now() - start);
  }

  const sorted = [...runs].sort((a, b) => a - b);
  const sum = sorted.reduce((a, b) => a + b, 0);
  const avg = sum / sorted.length;

  return {
    avg: precise(avg),
    min: precise(sorted[0]),
    max: precise(sorted[sorted.length - 1]),
    p50: precise(sorted[Math.floor(sorted.length * 0.5)]),
    p95: precise(sorted[Math.floor(sorted.length * 0.95)]),
    runs: runs.map(precise),
    avgHuman: humanTime(avg),
    ...(lastError ? { error: lastError } : {}),
  };
}

/** Round to 4 decimal places (0.0001ms = 0.1 microsecond resolution). */
function precise(n: number): number {
  return Math.round(n * 10000) / 10000;
}

/** Round to 2 decimal places (for summary display). */
function round2(n: number): number {
  return Math.round(n * 100) / 100;
}

/**
 * Human-readable time: shows microseconds for sub-ms values.
 * When deployed, CPU-bound work shows 0 (timer frozen until I/O) — we surface that.
 */
function humanTime(ms: number): string {
  if (ms >= 1) return `${round2(ms)}ms`;
  if (ms >= 0.001) return `${round2(ms * 1000)}us`;
  if (ms > 0) return `${round2(ms * 1000000)}ns`;
  return "<1ms (timer frozen — run locally to measure CPU-bound work)";
}

// ---------------------------------------------------------------------------
// Default test fixtures
// ---------------------------------------------------------------------------

const SAMPLE_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta name="description" content="A sample page for benchmarking" />
  <meta name="og:title" content="Benchmark Page" />
  <base href="https://example.com/" />
  <title>Benchmark Page</title>
  <link rel="stylesheet" href="/styles/main.css" />
  <link rel="stylesheet" href="/styles/theme.css" />
  <script src="/js/app.js"></script>
  <script src="/js/vendor.js"></script>
</head>
<body>
  <header>
    <nav>
      <a href="/">Home</a>
      <a href="/about">About</a>
      <a href="/contact">Contact</a>
      <a href="https://external.com/page">External</a>
    </nav>
  </header>
  <main>
    <h1>Welcome to the Benchmark Page</h1>
    <p>This is a sample page used for testing HTML processing performance across the three backends.</p>
    <img src="/images/hero.png" alt="Hero image" />
    <img src="/images/logo.svg" alt="Logo" />
    <div class="content">
      <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.</p>
      <a href="/products/1">Product 1</a>
      <a href="/products/2">Product 2</a>
      <a href="/products/3">Product 3</a>
      <a href="https://cdn.example.com/resource">CDN Resource</a>
    </div>
    <section>
      <h2>Features</h2>
      <ul>
        <li><a href="/features/speed">Speed</a></li>
        <li><a href="/features/reliability">Reliability</a></li>
        <li><a href="/features/scale">Scale</a></li>
      </ul>
    </section>
    <video src="/media/intro.mp4" poster="/media/poster.jpg"></video>
    <audio src="/media/podcast.mp3"></audio>
    <iframe src="https://embed.example.com/widget"></iframe>
  </main>
  <footer>
    <a href="/privacy">Privacy</a>
    <a href="/terms">Terms</a>
  </footer>
</body>
</html>`;

const SAMPLE_MARKDOWN = `# Welcome

This is a [multi
line link](https://example.com) test.

[Skip to Content](#main)

Here is another [link with
newlines inside](https://example.com/page).

Some regular text here.`;

const SAMPLE_SITEMAP = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/</loc></url>
  <url><loc>https://example.com/about</loc></url>
  <url><loc>https://example.com/products/1</loc></url>
  <url><loc>https://example.com/products/2</loc></url>
  <url><loc>https://example.com/blog/post-1</loc></url>
  <url><loc>https://example.com/blog/post-2</loc></url>
  <url><loc>https://example.com/contact</loc></url>
</urlset>`;

// ---------------------------------------------------------------------------
// Available benchmark functions
// ---------------------------------------------------------------------------

type BenchmarkFn = (
  env: AppEnv["Bindings"],
  html: string,
) => Record<string, () => unknown | Promise<unknown>>;

const BENCHMARKS: Record<string, BenchmarkFn> = {
  extractLinks: (env, html) => ({
    wasm: () => wasmNative.extractLinks(html),
    engine: () => engine.extractLinks(env, html),
    container: () => container.extractLinks(env, html),
  }),

  extractBaseHref: (env, html) => ({
    wasm: () => wasmNative.extractBaseHref(html, "https://example.com"),
    engine: () => engine.extractBaseHref(env, html, "https://example.com"),
    container: () =>
      container.extractBaseHref(env, html, "https://example.com"),
  }),

  extractMetadata: (env, html) => ({
    wasm: () => wasmNative.extractMetadata(html),
    engine: () => engine.extractMetadata(env, html),
    container: () => container.extractMetadata(env, html),
  }),

  transformHtml: (env, html) => ({
    wasm: () =>
      wasmNative.transformHtml({
        html,
        url: "https://example.com",
        only_main_content: true,
      }),
    engine: () =>
      engine.transformHtml(env, {
        html,
        url: "https://example.com",
        only_main_content: true,
      }),
    container: () =>
      container.transformHtml(env, {
        html,
        url: "https://example.com",
        only_main_content: true,
      }),
  }),

  getInnerJson: (env, html) => ({
    wasm: () => wasmNative.getInnerJson(html),
    engine: () => engine.getInnerJson(env, html),
    container: () => container.getInnerJson(env, html),
  }),

  extractImages: (env, html) => ({
    wasm: () => wasmNative.extractImages(html, "https://example.com"),
    engine: () => engine.extractImages(env, html, "https://example.com"),
    container: () => container.extractImages(env, html, "https://example.com"),
  }),

  extractAssets: (env, html) => ({
    wasm: () => wasmNative.extractAssets(html, "https://example.com"),
    engine: () => engine.extractAssets(env, html, "https://example.com"),
    container: () => container.extractAssets(env, html, "https://example.com"),
  }),

  postProcessMarkdown: (env, _html) => ({
    wasm: () => wasmNative.postProcessMarkdown(SAMPLE_MARKDOWN),
    engine: () => engine.postProcessMarkdown(env, SAMPLE_MARKDOWN),
    container: () => container.postProcessMarkdown(env, SAMPLE_MARKDOWN),
  }),

  parseSitemap: (env, _html) => ({
    wasm: () => wasmNative.parseSitemap(SAMPLE_SITEMAP),
    engine: () => engine.parseSitemap(env, SAMPLE_SITEMAP),
    container: () => container.parseSitemap(env, SAMPLE_SITEMAP),
  }),

  extractAttributes: (env, html) => ({
    wasm: () =>
      wasmNative.extractAttributes(html, {
        selectors: [
          { selector: "a", attribute: "href" },
          { selector: "img", attribute: "src" },
        ],
      }),
    engine: () =>
      engine.extractAttributes(env, html, {
        selectors: [
          { selector: "a", attribute: "href" },
          { selector: "img", attribute: "src" },
        ],
      }),
    container: () =>
      container.extractAttributes(env, html, {
        selectors: [
          { selector: "a", attribute: "href" },
          { selector: "img", attribute: "src" },
        ],
      }),
  }),

  filterLinks: (env, _html) => ({
    wasm: () =>
      wasmNative.filterLinks({
        links: [
          "https://example.com/products/1",
          "https://example.com/products/2",
          "https://example.com/about",
          "https://external.com/page",
          "https://example.com/features/speed",
        ],
        maxDepth: 3,
        baseUrl: "https://example.com",
        initialUrl: "https://example.com",
      }),
    engine: () =>
      engine.filterLinks(env, {
        links: [
          "https://example.com/products/1",
          "https://example.com/products/2",
          "https://example.com/about",
          "https://external.com/page",
          "https://example.com/features/speed",
        ],
        maxDepth: 3,
        baseUrl: "https://example.com",
        initialUrl: "https://example.com",
      }),
    container: () =>
      container.filterLinks(env, {
        links: [
          "https://example.com/products/1",
          "https://example.com/products/2",
          "https://example.com/about",
          "https://external.com/page",
          "https://example.com/features/speed",
        ],
        maxDepth: 3,
        baseUrl: "https://example.com",
        initialUrl: "https://example.com",
      }),
  }),

  // --- Engine + Container only (no WASM) ---

  htmlToMarkdown: (env, html) => ({
    engine: () => engine.htmlToMarkdown(env, html),
    container: () => container.htmlToMarkdown(env, html),
  }),

  nativeSearch: (env, _html) => ({
    engine: () =>
      engine.nativeSearch(env, { query: "cloudflare workers", num_results: 3 }),
    container: () =>
      container.nativeSearch(env, {
        query: "cloudflare workers",
        num_results: 3,
      }),
  }),
};

const ALL_FUNCTIONS = Object.keys(BENCHMARKS);

// ---------------------------------------------------------------------------
// Controller
// ---------------------------------------------------------------------------

export async function benchmarkController(c: Context<AppEnv>) {
  const environment = c.env.ENVIRONMENT;
  if (environment && environment !== "devel") {
    return c.json(
      {
        success: false,
        error: "Benchmark only available in devel environment",
      },
      403,
    );
  }

  let body: {
    html?: string;
    url?: string;
    iterations?: number;
    functions?: string[];
    /** Number of warmup runs per backend (discarded). Default: 2. */
    warmup?: number;
  } = {};
  try {
    body = await c.req.json();
  } catch {
    // empty/invalid JSON -> use defaults
  }

  const iterations = Math.min(Math.max(body.iterations ?? 10, 1), 100);
  const warmup = Math.min(Math.max(body.warmup ?? 2, 0), 10);
  const requestedFns = body.functions?.length
    ? body.functions.filter((f) => ALL_FUNCTIONS.includes(f))
    : ALL_FUNCTIONS;

  // Resolve HTML
  let html = body.html ?? "";
  let htmlSource = "inline";
  if (!html && body.url) {
    try {
      const resp = await fetch(body.url, {
        headers: { "User-Agent": "appzcrawl-benchmark/1.0" },
      });
      html = await resp.text();
      htmlSource = body.url;
    } catch (e) {
      return c.json(
        {
          success: false,
          error: `Failed to fetch URL: ${e instanceof Error ? e.message : String(e)}`,
        },
        400,
      );
    }
  }
  if (!html) {
    html = SAMPLE_HTML;
    htmlSource = "sample (built-in)";
  }

  const htmlSizeKb = round2(new Blob([html]).size / 1024);

  // Backend availability
  const wasmAvailable = wasmNative.isAvailable();
  const engineAvailable = engine.isAvailable(c.env);

  // Run benchmarks
  const results: Record<
    string,
    Record<string, TimingResult | { skipped: string }>
  > = {};

  for (const fnName of requestedFns) {
    const backends = BENCHMARKS[fnName](c.env, html);
    results[fnName] = {};

    for (const [backendName, fn] of Object.entries(backends)) {
      if (backendName === "wasm" && !wasmAvailable) {
        results[fnName][backendName] = { skipped: "WASM module not available" };
        continue;
      }
      if (backendName === "engine" && !engineAvailable) {
        results[fnName][backendName] = {
          skipped: "APPZCRAWL_ENGINE binding not available",
        };
        continue;
      }
      results[fnName][backendName] = await time(fn, iterations, warmup);
    }
  }

  // Build summary: fastest backend per function
  const summary: Record<
    string,
    { fastest: string; speedup: string; times: Record<string, string> }
  > = {};
  for (const fnName of requestedFns) {
    const fnResults = results[fnName];
    let fastest = "";
    let fastestAvg = Number.POSITIVE_INFINITY;
    const times: Record<string, string> = {};

    for (const [backend, result] of Object.entries(fnResults)) {
      if ("skipped" in result) continue;
      times[backend] = result.avgHuman;
      if (result.avg < fastestAvg) {
        fastestAvg = result.avg;
        fastest = backend;
      }
    }

    // Find slowest for speedup calculation
    let slowestAvg = 0;
    let slowest = "";
    for (const [backend, result] of Object.entries(fnResults)) {
      if ("skipped" in result) continue;
      if (result.avg > slowestAvg) {
        slowestAvg = result.avg;
        slowest = backend;
      }
    }

    if (fastest && slowest) {
      const speedup =
        fastestAvg > 0
          ? `${round2(slowestAvg / fastestAvg)}x faster than ${slowest}`
          : `${slowest}: ${humanTime(slowestAvg)}`;
      summary[fnName] = { fastest, speedup, times };
    }
  }

  return c.json({
    success: true,
    config: {
      htmlSource,
      htmlSizeKb,
      iterations,
      warmup,
      functions: requestedFns,
      wasmAvailable,
      engineAvailable,
      timerBehavior:
        "Deployed: timer advances only after I/O (Spectre mitigation). CPU-bound (WASM, Engine) show 0. Run wrangler dev locally for accurate CPU timings. See https://developers.cloudflare.com/workers/runtime-apis/performance/",
      ...(wasmAvailable
        ? {}
        : {
            wasmInitError: getWasmInitError() ?? "unknown",
            wasmDiagnostics: getWasmDiagnostics(),
          }),
    },
    results,
    summary,
  });
}
