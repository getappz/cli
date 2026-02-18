/**
 * Search execution service.
 * Adapted from firecrawl/apps/api/src/search/execute.ts
 */
import type { SearchRequestBody, SearchResponse } from "../contracts/search";
import type { logger } from "../lib/logger";
import type { AppEnv } from "../types";
interface SearchExecuteOptions {
    body: SearchRequestBody;
    teamId: string;
    env: AppEnv["Bindings"];
    logger: typeof logger;
}
interface SearchExecuteResult {
    response: SearchResponse;
    totalResultsCount: number;
    searchCredits: number;
}
/**
 * Execute search using native Rust implementation (DuckDuckGo or SearXNG).
 * - Calls native container /search endpoint
 * - Returns structured response matching Firecrawl's SearchResponse
 * - Calculates credits based on results
 */
export declare function executeSearch(options: SearchExecuteOptions): Promise<SearchExecuteResult>;
export {};
//# sourceMappingURL=search-execute.d.ts.map