import { Robot } from "robots-parser";
import { ScrapeOptions } from "../../controllers/v2/types";
export declare const SITEMAP_LIMIT = 25;
interface FilterResult {
    allowed: boolean;
    url?: string;
    denialReason?: string;
}
interface FilterLinksResult {
    links: string[];
    denialReasons: Map<string, string>;
}
export declare class WebCrawler {
    private jobId;
    private initialUrl;
    private baseUrl;
    private includes;
    private excludes;
    private maxCrawledLinks;
    private maxCrawledDepth;
    private visited;
    private crawledUrls;
    private limit;
    private robotsTxt;
    private robotsTxtUrl;
    robots: Robot;
    private robotsCrawlDelay;
    private generateImgAltText;
    private allowBackwardCrawling;
    private allowExternalContentLinks;
    private allowSubdomains;
    private ignoreRobotsTxt;
    private regexOnFullURL;
    private logger;
    private sitemapsHit;
    private maxDiscoveryDepth;
    private currentDiscoveryDepth;
    private zeroDataRetention;
    private location?;
    private headers?;
    constructor({ jobId, initialUrl, baseUrl, includes, excludes, maxCrawledLinks, limit, generateImgAltText, maxCrawledDepth, allowBackwardCrawling, allowExternalContentLinks, allowSubdomains, ignoreRobotsTxt, regexOnFullURL, maxDiscoveryDepth, currentDiscoveryDepth, zeroDataRetention, location, headers, }: {
        jobId: string;
        initialUrl: string;
        baseUrl?: string;
        includes?: string[];
        excludes?: string[];
        maxCrawledLinks?: number;
        limit?: number;
        generateImgAltText?: boolean;
        maxCrawledDepth?: number;
        allowBackwardCrawling?: boolean;
        allowExternalContentLinks?: boolean;
        allowSubdomains?: boolean;
        ignoreRobotsTxt?: boolean;
        regexOnFullURL?: boolean;
        maxDiscoveryDepth?: number;
        currentDiscoveryDepth?: number;
        zeroDataRetention?: boolean;
        location?: ScrapeOptions["location"];
        headers?: Record<string, string>;
    });
    setBaseUrl(newBase: string): void;
    filterLinks(sitemapLinks: string[], limit: number, maxDepth: number, fromMap?: boolean, skipRobots?: boolean): Promise<FilterLinksResult>;
    getRobotsTxt(skipTlsVerification?: boolean, abort?: AbortSignal): Promise<string>;
    importRobotsTxt(txt: string): void;
    getRobotsCrawlDelay(): number | null;
    tryGetSitemap(urlsHandler: (urls: string[]) => unknown, fromMap?: boolean, onlySitemap?: boolean, timeout?: number, abort?: AbortSignal, mock?: string, maxAge?: number): Promise<number>;
    filterURL(href: string, url: string): Promise<FilterResult>;
    private extractLinksFromHTMLRust;
    private extractLinksFromHTMLCheerio;
    extractLinksFromHTML(html: string, url: string): Promise<string[]>;
    private isRobotsAllowed;
    isFile(url: string): boolean;
    private tryFetchSitemapLinks;
}
export {};
//# sourceMappingURL=crawler.d.ts.map