import { Meta } from "..";
import { EngineScrapeResult } from "../engines";
export interface Postprocessor {
    name: string;
    shouldRun: (meta: Meta, url: URL, postProcessorsUsed?: string[]) => boolean;
    run: (meta: Meta, engineResult: EngineScrapeResult) => Promise<EngineScrapeResult>;
}
export declare const postprocessors: Postprocessor[];
//# sourceMappingURL=index.d.ts.map