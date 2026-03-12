/**
 * Client for the fire-engine remote service (browser / TLS client scraping).
 * When FIRE_ENGINE_URL is set, the scrape runner can use this to fetch HTML
 * via fire-engine instead of Worker fetch (for JS-heavy or bot-protected sites).
 *
 * API: POST /scrape → { jobId, processing } or result; poll GET /scrape/:jobId; DELETE /scrape/:jobId.
 */
const POLL_INTERVAL_MS = 500;
const DEFAULT_TIMEOUT_MS = 60_000;
/**
 * Fetch HTML for a URL using the fire-engine service (tlsclient engine).
 * Returns HTML and status code, or an error. Call only when baseUrl is non-empty.
 */
export async function fireEngineFetchHtml(baseUrl, url, options = {}) {
    const timeout = options.timeout ?? DEFAULT_TIMEOUT_MS;
    const base = baseUrl.replace(/\/$/, "");
    const scrapeBody = {
        url,
        engine: "tlsclient",
        instantReturn: false,
        timeout,
        skipTlsVerification: options.skipTlsVerification ?? true,
    };
    let res;
    try {
        res = await fetch(`${base}/scrape`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(scrapeBody),
        });
    }
    catch (e) {
        return {
            success: false,
            error: e instanceof Error ? e.message : "Fire-engine request failed",
        };
    }
    let data;
    try {
        data = (await res.json());
    }
    catch {
        return {
            success: false,
            error: "Invalid JSON from fire-engine",
        };
    }
    if (typeof data.error === "string") {
        return {
            success: false,
            error: data.error,
            statusCode: typeof data.pageStatusCode === "number"
                ? data.pageStatusCode
                : undefined,
        };
    }
    if (data.processing === true && typeof data.jobId === "string") {
        const jobId = data.jobId;
        const pollUntil = Date.now() + timeout;
        while (Date.now() < pollUntil) {
            await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
            let pollRes;
            try {
                pollRes = await fetch(`${base}/scrape/${jobId}`);
            }
            catch (e) {
                await deleteJob(base, jobId);
                return {
                    success: false,
                    error: e instanceof Error ? e.message : "Fire-engine poll failed",
                };
            }
            let pollData;
            try {
                pollData = (await pollRes.json());
            }
            catch {
                await deleteJob(base, jobId);
                return {
                    success: false,
                    error: "Invalid JSON from fire-engine checkStatus",
                };
            }
            if (typeof pollData.error === "string") {
                await deleteJob(base, jobId);
                return {
                    success: false,
                    error: pollData.error,
                    statusCode: typeof pollData.pageStatusCode === "number"
                        ? pollData.pageStatusCode
                        : undefined,
                };
            }
            if (pollData.processing === false && pollData.state === "completed") {
                await deleteJob(base, jobId);
                const content = pollData.content;
                const statusCode = typeof pollData.pageStatusCode === "number"
                    ? pollData.pageStatusCode
                    : 200;
                if (typeof content !== "string") {
                    return {
                        success: false,
                        error: "Fire-engine returned non-string content",
                    };
                }
                return {
                    success: true,
                    html: content,
                    statusCode,
                    finalUrl: typeof pollData.url === "string" ? pollData.url : undefined,
                };
            }
        }
        await deleteJob(base, jobId);
        return {
            success: false,
            error: "Fire-engine scrape timed out (polling)",
        };
    }
    if (data.processing === false ||
        (data.content !== undefined && data.pageStatusCode !== undefined)) {
        const content = data.content;
        const statusCode = typeof data.pageStatusCode === "number" ? data.pageStatusCode : 200;
        if (typeof content !== "string") {
            return {
                success: false,
                error: "Fire-engine returned non-string content",
            };
        }
        return {
            success: true,
            html: content,
            statusCode,
            finalUrl: typeof data.url === "string" ? data.url : undefined,
        };
    }
    return {
        success: false,
        error: "Unexpected fire-engine response",
    };
}
async function deleteJob(base, jobId) {
    try {
        await fetch(`${base}/scrape/${jobId}`, { method: "DELETE" });
    }
    catch {
        // best-effort
    }
}
export function isFireEngineEnabled(env) {
    const url = env.FIRE_ENGINE_URL;
    return typeof url === "string" && url.trim().length > 0;
}
//# sourceMappingURL=fire-engine-client.js.map