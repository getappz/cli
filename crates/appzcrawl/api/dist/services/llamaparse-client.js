/**
 * LlamaParse API client for PDF parsing.
 * Uses v2 API: upload → poll for completion → get markdown.
 * Set LLAMAPARSE_API_KEY in bindings to enable.
 */
function markdownToHtml(md) {
    const escapeHtml = (s) => s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
    const paragraphs = md.split(/\n\n+/).filter((p) => p.trim());
    const body = paragraphs.length > 0
        ? paragraphs
            .map((p) => `<p>${escapeHtml(p.trim()).replace(/\n/g, "<br>\n")}</p>`)
            .join("\n")
        : `<p>${escapeHtml(md.trim()) || " "}</p>`;
    return `<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Document</title></head><body><main>${body}</main></body></html>`;
}
const LLAMAPARSE_BASE = "https://api.cloud.llamaindex.ai/api/v2";
const POLL_INTERVAL_MS = 2000;
const MAX_POLL_ATTEMPTS = 90; // ~3 min at 2s interval
/**
 * Parse a PDF via LlamaParse v2 API.
 * Uploads the file, polls until COMPLETED, returns markdown, HTML, and optionally images.
 */
export async function parsePdfWithLlamaParse(apiKey, pdfData, opts = {}) {
    const { timeoutMs = 120_000, wantsImages = false } = opts;
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    const signal = controller.signal;
    try {
        // 1. Upload and start parsing
        const formData = new FormData();
        formData.append("file", new Blob([pdfData], { type: "application/pdf" }), "document.pdf");
        const configuration = {
            tier: "cost_effective",
            version: "latest",
        };
        if (wantsImages) {
            configuration.output_options = {
                images_to_save: ["embedded", "screenshot"],
            };
        }
        formData.append("configuration", JSON.stringify(configuration));
        const uploadRes = await fetch(`${LLAMAPARSE_BASE}/parse/upload`, {
            method: "POST",
            headers: { Authorization: `Bearer ${apiKey}` },
            body: formData,
            signal,
        });
        if (!uploadRes.ok) {
            const errBody = await uploadRes.text();
            throw new Error(`LlamaParse upload failed: ${uploadRes.status} ${errBody}`);
        }
        const uploadJson = (await uploadRes.json());
        const jobId = uploadJson.id ?? uploadJson.job?.id ?? uploadJson.job_id;
        if (!jobId || typeof jobId !== "string") {
            throw new Error(`LlamaParse upload response missing job id: ${JSON.stringify(uploadJson)}`);
        }
        // 2. Poll for completion
        for (let i = 0; i < MAX_POLL_ATTEMPTS; i++) {
            if (signal.aborted)
                throw new Error("LlamaParse aborted");
            const expand = wantsImages
                ? "markdown,images_content_metadata"
                : "markdown";
            const statusRes = await fetch(`${LLAMAPARSE_BASE}/parse/${jobId}?expand=${expand}`, {
                headers: { Authorization: `Bearer ${apiKey}` },
                signal,
            });
            if (!statusRes.ok) {
                const errBody = await statusRes.text();
                throw new Error(`LlamaParse status failed: ${statusRes.status} ${errBody}`);
            }
            const statusJson = (await statusRes.json());
            const job = statusJson.job ?? statusJson;
            const status = job.status ?? statusJson.status ?? "UNKNOWN";
            if (status === "COMPLETED") {
                const markdown = statusJson.markdown ??
                    job.markdown ??
                    "";
                clearTimeout(timeout);
                const html = markdownToHtml(markdown || " ");
                let images;
                const imgMeta = statusJson.images_content_metadata;
                if (wantsImages && imgMeta?.images?.length) {
                    images = imgMeta.images
                        .map((img) => img.presigned_url)
                        .filter((url) => typeof url === "string");
                }
                return { markdown: markdown || " ", html, images };
            }
            if (status === "FAILED" || status === "CANCELLED") {
                const errMsg = job.error_message ?? "Parse failed";
                throw new Error(`LlamaParse job ${status}: ${errMsg}`);
            }
            await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
        }
        clearTimeout(timeout);
        throw new Error("LlamaParse timed out waiting for job completion");
    }
    finally {
        clearTimeout(timeout);
    }
}
//# sourceMappingURL=llamaparse-client.js.map