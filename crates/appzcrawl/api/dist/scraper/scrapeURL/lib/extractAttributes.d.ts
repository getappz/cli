type AttributeResult = {
    selector: string;
    attribute: string;
    values: string[];
};
type AttributeSelector = {
    selector: string;
    attribute: string;
};
/**
 * Extracts attributes from HTML using Rust html-transformer (with Cheerio fallback)
 * @param html - The HTML content to extract from
 * @param selectors - Array of selector/attribute pairs to extract
 * @returns Array of extracted attribute results
 */
export declare function extractAttributes(html: string, selectors: AttributeSelector[]): Promise<AttributeResult[]>;
export {};
//# sourceMappingURL=extractAttributes.d.ts.map