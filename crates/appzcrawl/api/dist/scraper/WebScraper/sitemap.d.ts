import type { Logger } from "winston";
import type { ScrapeOptions } from "../../controllers/v2/types";
export declare function getLinksFromSitemap({ sitemapUrl, urlsHandler, mode, maxAge, zeroDataRetention, location, headers, }: {
    sitemapUrl: string;
    urlsHandler(urls: string[]): unknown;
    mode?: "axios" | "fire-engine";
    maxAge?: number;
    zeroDataRetention: boolean;
    location?: ScrapeOptions["location"];
    headers?: Record<string, string>;
}, logger: Logger, crawlId: string, sitemapsHit: Set<string>, abort?: AbortSignal, mock?: string): Promise<number>;
//# sourceMappingURL=sitemap.d.ts.map