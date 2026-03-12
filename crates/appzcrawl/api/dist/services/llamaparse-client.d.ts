/**
 * LlamaParse API client for PDF parsing.
 * Uses v2 API: upload → poll for completion → get markdown.
 * Set LLAMAPARSE_API_KEY in bindings to enable.
 */
export interface LlamaParseResult {
    markdown: string;
    html: string;
    /** Extracted image URLs (presigned). Only set when wantsImages and output_options.images_to_save is used. */
    images?: string[];
}
/**
 * Parse a PDF via LlamaParse v2 API.
 * Uploads the file, polls until COMPLETED, returns markdown, HTML, and optionally images.
 */
export declare function parsePdfWithLlamaParse(apiKey: string, pdfData: Uint8Array, opts?: {
    timeoutMs?: number;
    wantsImages?: boolean;
}): Promise<LlamaParseResult>;
//# sourceMappingURL=llamaparse-client.d.ts.map