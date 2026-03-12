/**
 * Map runner: discover URLs from sitemap(s) and/or seed page links.
 * Firecrawl-compatible implementation with path filtering, deduplication, and search ranking.
 * Sitemap XML is parsed by the Rust native container (quick-xml); no Node XML deps.
 */
import type { MapDocument, MapRequest } from "../contracts/map";
import type { AppEnv } from "../types";
export interface MapRunnerResult {
    success: true;
    links: MapDocument[];
}
export declare function getMapResults(env: AppEnv["Bindings"], options: MapRequest & {
    abort?: AbortSignal;
}): Promise<MapRunnerResult>;
//# sourceMappingURL=map-runner.d.ts.map