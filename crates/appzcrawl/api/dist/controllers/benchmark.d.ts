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
import type { AppEnv } from "../types";
export declare function benchmarkController(c: Context<AppEnv>): Promise<(Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 403, "json">) | (Response & import("hono").TypedResponse<{
    success: false;
    error: string;
}, 400, "json">) | (Response & import("hono").TypedResponse<{
    success: true;
    config: {
        wasmInitError?: string | undefined;
        wasmDiagnostics?: {
            [x: string]: import("hono/utils/types").JSONValue;
        } | undefined;
        htmlSource: string;
        htmlSizeKb: number;
        iterations: number;
        warmup: number;
        functions: string[];
        wasmAvailable: boolean;
        engineAvailable: boolean;
        timerBehavior: string;
    };
    results: {
        [x: string]: {
            [x: string]: {
                avg: number;
                min: number;
                max: number;
                p50: number;
                p95: number;
                runs: number[];
                avgHuman: string;
                error?: string | undefined;
            } | {
                skipped: string;
            };
        };
    };
    summary: {
        [x: string]: {
            fastest: string;
            speedup: string;
            times: {
                [x: string]: string;
            };
        };
    };
}, import("hono/utils/http-status").ContentfulStatusCode, "json">)>;
//# sourceMappingURL=benchmark.d.ts.map