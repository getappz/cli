import { Container } from "@cloudflare/containers";
/**
 * Cloudflare Container that runs the appzcrawl native addon server (Node + firecrawl-rs).
 * Server listens on port 4000 and exposes POST /extract-links, /transform-html, etc.
 */
export class AppzcrawlContainer extends Container {
    defaultPort = 4000;
    sleepAfter = "40s";
    pingEndpoint = "/health";
}
//# sourceMappingURL=container.js.map