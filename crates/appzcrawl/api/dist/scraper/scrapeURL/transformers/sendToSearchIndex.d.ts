/**
 * Transformer: Send Document to Search Index
 *
 * Integrates with the existing scraper transformer stack.
 * Queues documents for real-time search indexing.
 *
 * Sampling: Controlled via SEARCH_INDEX_SAMPLE_RATE (0.0-1.0)
 * - 0.1 = 10% of documents indexed (recommended for initial rollout)
 * - 1.0 = 100% of documents indexed (full production)
 */
import { Document } from "../../../controllers/v1/types";
import { Meta } from "..";
/**
 * Transformer: Send document to search index
 */
export declare function sendDocumentToSearchIndex(meta: Meta, document: Document): Promise<Document>;
//# sourceMappingURL=sendToSearchIndex.d.ts.map