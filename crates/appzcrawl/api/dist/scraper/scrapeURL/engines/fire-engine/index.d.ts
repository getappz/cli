import { Meta } from "../..";
import { EngineScrapeResult } from "..";
export declare function scrapeURLWithFireEngineChromeCDP(meta: Meta): Promise<EngineScrapeResult>;
export declare function scrapeURLWithFireEnginePlaywright(meta: Meta): Promise<EngineScrapeResult>;
export declare function scrapeURLWithFireEngineTLSClient(meta: Meta): Promise<EngineScrapeResult>;
export declare function fireEngineMaxReasonableTime(meta: Meta, engine: "chrome-cdp" | "playwright" | "tlsclient"): number;
//# sourceMappingURL=index.d.ts.map