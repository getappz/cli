import { Container } from "@cloudflare/containers";
/**
 * Cloudflare Container that runs the appzcrawl native addon server (Node + firecrawl-rs).
 * Server listens on port 4000 and exposes POST /extract-links, /transform-html, etc.
 */
export declare class AppzcrawlContainer extends Container {
    defaultPort: number;
    sleepAfter: string;
    pingEndpoint: string;
}
//# sourceMappingURL=container.d.ts.map