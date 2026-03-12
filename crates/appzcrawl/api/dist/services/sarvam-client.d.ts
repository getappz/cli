/**
 * Sarvam Vision (Document Intelligence) API client for PDF parsing.
 * Uses job-based flow: create → upload → start → poll → download ZIP → extract content.
 * Supports html and markdown output formats (API does not support json). Can run multiple jobs to fetch all.
 * Set SARVAM_API_KEY in bindings to enable.
 * @see https://docs.sarvam.ai/api-reference-docs/getting-started/models/sarvam-vision
 */
/** Sarvam Document Intelligence only supports html and md. JSON is not supported. */
export type SarvamOutputFormat = "html" | "md";
export interface SarvamResult {
    /** HTML content. Always present (derived from md if needed). */
    html: string;
    /** Markdown when output_format was "md". */
    markdown?: string;
}
/**
 * Parse a PDF via Sarvam Vision Document Intelligence API.
 * By default requests "md" (we derive html). Use requestedFormats to get html and/or md.
 * When multiple formats requested, runs parallel jobs and merges results.
 */
export declare function parsePdfWithSarvam(apiKey: string, pdfData: Uint8Array, opts?: {
    timeoutMs?: number;
    language?: string;
    /** Formats to fetch. Default ["md"]. Each format = one Sarvam job (runs in parallel). */
    requestedFormats?: SarvamOutputFormat[];
}): Promise<SarvamResult>;
//# sourceMappingURL=sarvam-client.d.ts.map