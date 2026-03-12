/**
 * Sarvam Vision (Document Intelligence) API client for PDF parsing.
 * Uses job-based flow: create → upload → start → poll → download ZIP → extract content.
 * Supports html and markdown output formats (API does not support json). Can run multiple jobs to fetch all.
 * Set SARVAM_API_KEY in bindings to enable.
 * @see https://docs.sarvam.ai/api-reference-docs/getting-started/models/sarvam-vision
 */

import { unzipSync } from "fflate";

function markdownToHtml(md: string): string {
  const escapeHtml = (s: string) =>
    s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  const paragraphs = md.split(/\n\n+/).filter((p) => p.trim());
  const body =
    paragraphs.length > 0
      ? paragraphs
          .map((p) => `<p>${escapeHtml(p.trim()).replace(/\n/g, "<br>\n")}</p>`)
          .join("\n")
      : `<p>${escapeHtml(md.trim()) || " "}</p>`;
  return `<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Document</title></head><body><main>${body}</main></body></html>`;
}

const SARVAM_BASE = "https://api.sarvam.ai";
const POLL_INTERVAL_MS = 2000;
const MAX_POLL_ATTEMPTS = 150; // ~5 min at 2s

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
export async function parsePdfWithSarvam(
  apiKey: string,
  pdfData: Uint8Array,
  opts: {
    timeoutMs?: number;
    language?: string;
    /** Formats to fetch. Default ["md"]. Each format = one Sarvam job (runs in parallel). */
    requestedFormats?: SarvamOutputFormat[];
  } = {},
): Promise<SarvamResult> {
  const {
    timeoutMs = 180_000,
    language = "en-IN",
    requestedFormats = ["md"],
  } = opts;
  const formats = [...new Set(requestedFormats)];
  if (formats.length === 0) formats.push("md");

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  const signal = controller.signal;

  const runOneJob = async (
    outputFormat: SarvamOutputFormat,
  ): Promise<Partial<SarvamResult>> => {
    const headers: Record<string, string> = {
      "api-subscription-key": apiKey,
      "Content-Type": "application/json",
    };

    const createRes = await fetch(`${SARVAM_BASE}/doc-digitization/job/v1`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        job_parameters: { language, output_format: outputFormat },
      }),
      signal,
    });
    if (!createRes.ok) {
      const errBody = await createRes.text();
      throw new Error(
        `Sarvam create job failed: ${createRes.status} ${errBody}`,
      );
    }
    const createJson = (await createRes.json()) as { job_id?: string };
    const jobId = createJson.job_id;
    if (!jobId || typeof jobId !== "string") {
      throw new Error(
        `Sarvam create response missing job_id: ${JSON.stringify(createJson)}`,
      );
    }

    const uploadLinksRes = await fetch(
      `${SARVAM_BASE}/doc-digitization/job/v1/upload-files`,
      {
        method: "POST",
        headers,
        body: JSON.stringify({ job_id: jobId, files: ["document.pdf"] }),
        signal,
      },
    );
    if (!uploadLinksRes.ok) {
      const errBody = await uploadLinksRes.text();
      throw new Error(
        `Sarvam upload links failed: ${uploadLinksRes.status} ${errBody}`,
      );
    }
    const uploadJson = (await uploadLinksRes.json()) as {
      upload_urls?: Record<
        string,
        { file_url?: string; file_metadata?: Record<string, string> }
      >;
    };
    const firstEntry =
      uploadJson.upload_urls && Object.values(uploadJson.upload_urls)[0];
    const fileUrl = firstEntry?.file_url;
    if (!fileUrl) {
      throw new Error(
        `Sarvam upload URLs invalid: ${JSON.stringify(uploadJson)}`,
      );
    }

    const uploadHeaders: Record<string, string> = {
      "x-ms-blob-type": "BlockBlob",
    };
    if (firstEntry?.file_metadata) {
      for (const [k, v] of Object.entries(firstEntry.file_metadata)) {
        if (typeof v === "string") uploadHeaders[k] = v;
      }
    }
    const uploadRes = await fetch(fileUrl, {
      method: "PUT",
      headers: uploadHeaders,
      body: pdfData,
      signal,
    });
    if (!uploadRes.ok) {
      throw new Error(
        `Sarvam file upload failed: ${uploadRes.status} ${uploadRes.statusText}`,
      );
    }

    const startRes = await fetch(
      `${SARVAM_BASE}/doc-digitization/job/v1/${encodeURIComponent(jobId)}/start`,
      { method: "POST", headers: { "api-subscription-key": apiKey }, signal },
    );
    if (!startRes.ok) {
      const errBody = await startRes.text();
      throw new Error(`Sarvam start failed: ${startRes.status} ${errBody}`);
    }

    const terminalStates = ["Completed", "PartiallyCompleted", "Failed"];
    let status: { job_state?: string; error_message?: string } = {};
    for (let i = 0; i < MAX_POLL_ATTEMPTS; i++) {
      if (signal.aborted) throw new Error("Sarvam aborted");
      const statusRes = await fetch(
        `${SARVAM_BASE}/doc-digitization/job/v1/${encodeURIComponent(jobId)}/status`,
        { headers: { "api-subscription-key": apiKey }, signal },
      );
      if (!statusRes.ok) {
        const errBody = await statusRes.text();
        throw new Error(`Sarvam status failed: ${statusRes.status} ${errBody}`);
      }
      status = (await statusRes.json()) as typeof status;
      if (terminalStates.includes(status.job_state ?? "")) break;
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
    }
    if (!terminalStates.includes(status.job_state ?? "")) {
      throw new Error("Sarvam timed out waiting for job completion");
    }
    if (status.job_state === "Failed") {
      const errMsg = status.error_message ?? "Parse failed";
      throw new Error(`Sarvam job failed: ${errMsg}`);
    }

    const downloadRes = await fetch(
      `${SARVAM_BASE}/doc-digitization/job/v1/${encodeURIComponent(jobId)}/download-files`,
      { method: "POST", headers: { "api-subscription-key": apiKey }, signal },
    );
    if (!downloadRes.ok) {
      const errBody = await downloadRes.text();
      throw new Error(
        `Sarvam download links failed: ${downloadRes.status} ${errBody}`,
      );
    }
    const downloadJson = (await downloadRes.json()) as {
      download_urls?: Record<string, { file_url?: string }>;
    };
    const dlEntry =
      downloadJson.download_urls &&
      Object.values(downloadJson.download_urls)[0];
    const dlUrl = dlEntry?.file_url;
    if (!dlUrl) {
      throw new Error(
        `Sarvam download URLs invalid: ${JSON.stringify(downloadJson)}`,
      );
    }

    const zipRes = await fetch(dlUrl, { signal });
    if (!zipRes.ok) {
      throw new Error(
        `Sarvam download failed: ${zipRes.status} ${zipRes.statusText}`,
      );
    }
    const zipBuf = new Uint8Array(await zipRes.arrayBuffer());
    const unzipped = unzipSync(zipBuf);
    const decoder = new TextDecoder("utf-8");

    const ext = outputFormat === "html" ? ".html" : ".md";
    const entry = Object.entries(unzipped).find(([name]) =>
      name.toLowerCase().endsWith(ext),
    );
    if (!entry) {
      throw new Error(
        `Sarvam output ZIP has no ${ext}: ${Object.keys(unzipped).join(", ")}`,
      );
    }
    const [, data] = entry;
    const text = decoder.decode(data as Uint8Array);

    if (outputFormat === "html") {
      return { html: text };
    }
    return { html: markdownToHtml(text), markdown: text };
  };

  try {
    const results = await Promise.all(formats.map((f) => runOneJob(f)));
    clearTimeout(timeout);

    let html = "";
    let markdown: string | undefined;
    for (const r of results) {
      if (r.html) html = r.html;
      if (r.markdown) markdown = r.markdown;
    }
    if (!html) {
      throw new Error("Sarvam returned no HTML (md or html format required)");
    }
    return {
      html,
      ...(markdown ? { markdown } : {}),
    };
  } finally {
    clearTimeout(timeout);
  }
}
