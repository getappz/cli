import { Document } from "../../../../controllers/v1/types";
import { EngineScrapeResult } from "..";
import { Meta } from "../..";
export declare function sendDocumentToIndex(meta: Meta, document: Document): Promise<any>;
export declare function scrapeURLWithIndex(meta: Meta): Promise<EngineScrapeResult>;
export declare function indexMaxReasonableTime(meta: Meta): number;
//# sourceMappingURL=index.d.ts.map