/**
 * Maps scrape runner output to Firecrawl-compatible response shape.
 */
import { type ScrapeRequestBody, type ScrapeSuccessResponse } from "../contracts/scrape";
import type { ScrapeRunnerDocument } from "./scrape-runner";
/**
 * Map runner document to Firecrawl-compatible response data.
 * Only includes fields for requested formats; adds warning for unsupported features.
 */
export declare function mapToFirecrawlResponse(document: ScrapeRunnerDocument, request: ScrapeRequestBody): ScrapeSuccessResponse;
//# sourceMappingURL=scrape-response-mapper.d.ts.map