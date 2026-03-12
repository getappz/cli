import type { Logger } from "winston";
import { ScrapeOptions } from "../../controllers/v2/types";
type SitemapScrapeOptions = {
    url: string;
    maxAge: number;
    zeroDataRetention: boolean;
    location: ScrapeOptions["location"];
    crawlId: string;
    logger?: Logger;
    isPreCrawl?: boolean;
};
type SitemapData = {
    urls: URL[];
    sitemaps: URL[];
};
export declare function scrapeSitemap(options: SitemapScrapeOptions): Promise<SitemapData>;
export {};
//# sourceMappingURL=sitemap.d.ts.map