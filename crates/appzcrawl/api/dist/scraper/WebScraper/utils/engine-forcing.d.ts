import { Engine } from "../../scrapeURL/engines";
/**
 * Initialize the engine forcing mappings from environment variable
 * Expected format: JSON object with domain patterns as keys and engines as values
 * Example: {"example.com": "playwright", "*.google.com": ["fire-engine;chrome-cdp", "playwright"]}
 */
export declare function initializeEngineForcing(): void;
/**
 * Get the forced engine(s) for a given URL based on domain mappings
 * @param url The URL to check
 * @returns The forced engine(s) if a match is found, undefined otherwise
 */
export declare function getEngineForUrl(url: string): Engine | Engine[] | undefined;
//# sourceMappingURL=engine-forcing.d.ts.map