/**
 * Client for the appzcrawl native addon running in a Cloudflare Container.
 * Uses getRandom() for stateless load balancing across container instances.
 */
import { getRandom } from "@cloudflare/containers";
const INSTANCE_COUNT = 1;
function getBinding(env) {
    return env.APPZCRAWL_CONTAINER;
}
/** Base URL for container requests; host is ignored, path is forwarded. */
const CONTAINER_ORIGIN = "http://native";
async function fetchNative(env, path, options) {
    const binding = getBinding(env);
    const container = await getRandom(binding, INSTANCE_COUNT);
    const url = `${CONTAINER_ORIGIN}${path}`;
    const request = new Request(url, {
        method: options.method,
        headers: { "Content-Type": "application/json" },
        body: options.method === "POST" ? options.body : undefined,
    });
    return container.fetch(request);
}
async function postJson(env, path, body) {
    const res = await fetchNative(env, path, {
        method: "POST",
        body: JSON.stringify(body),
    });
    // Read body as text first to handle empty/non-JSON responses gracefully
    const text = await res.text();
    if (!text) {
        throw new Error(`Native container ${path}: empty response (HTTP ${res.status}). Container may need rebuild.`);
    }
    let data;
    try {
        data = JSON.parse(text);
    }
    catch {
        throw new Error(`Native container ${path}: invalid JSON (HTTP ${res.status}): ${text.slice(0, 200)}`);
    }
    if (!res.ok) {
        const err = data && typeof data === "object" && "error" in data
            ? data.error
            : res.statusText;
        throw new Error(err || `Native container ${path}: ${res.status}`);
    }
    return data;
}
export async function nativeHealth(env) {
    const binding = getBinding(env);
    const container = await getRandom(binding, 1);
    const res = await container.fetch(new Request(`${CONTAINER_ORIGIN}/health`, { method: "GET" }));
    const data = await res.json();
    if (!res.ok)
        throw new Error(String(data?.error ?? res.statusText));
    return data;
}
export async function extractLinks(env, html) {
    return postJson(env, "/extract-links", { html });
}
/** Filter discovered links using the native Rust crawler (includePaths, excludePaths, robots, depth, etc.). */
export async function filterLinks(env, params) {
    return postJson(env, "/filter-links", {
        links: params.links,
        limit: params.limit ?? null,
        max_depth: params.maxDepth,
        base_url: params.baseUrl,
        initial_url: params.initialUrl,
        regex_on_full_url: params.regexOnFullUrl ?? false,
        excludes: params.excludes ?? [],
        includes: params.includes ?? [],
        allow_backward_crawling: params.allowBackwardCrawling ?? false,
        ignore_robots_txt: params.ignoreRobotsTxt ?? false,
        robots_txt: params.robotsTxt ?? "",
        allow_external_content_links: params.allowExternalContentLinks ?? false,
        allow_subdomains: params.allowSubdomains ?? false,
    });
}
/** Extract base href from HTML (firecrawl_rs). Used to resolve relative links. */
export async function extractBaseHref(env, html, baseUrl) {
    return postJson(env, "/extract-base-href", { html, url: baseUrl });
}
export async function transformHtml(env, params) {
    const body = {
        html: params.html,
        url: params.url,
        include_tags: params.include_tags,
        exclude_tags: params.exclude_tags,
        only_main_content: params.only_main_content,
        omce_signatures: params.omce_signatures,
    };
    return postJson(env, "/transform-html", body);
}
export async function getInnerJson(env, html) {
    return postJson(env, "/get-inner-json", { html });
}
export async function extractMetadata(env, html) {
    return postJson(env, "/extract-metadata", { html });
}
export async function extractAttributes(env, html, options) {
    return postJson(env, "/extract-attributes", {
        html,
        options: { selectors: options.selectors },
    });
}
export async function extractImages(env, html, baseUrl) {
    return postJson(env, "/extract-images", { html, base_url: baseUrl });
}
export async function extractAssets(env, html, baseUrl, formats) {
    return postJson(env, "/extract-assets", {
        html,
        base_url: baseUrl,
        formats: formats ?? ["assets"],
    });
}
export async function postProcessMarkdown(env, markdown, options) {
    const body = {
        markdown,
    };
    if (options?.baseUrl)
        body.baseUrl = options.baseUrl;
    if (options?.citations !== undefined)
        body.citations = options.citations;
    return postJson(env, "/post-process-markdown", body);
}
/** Parse sitemap XML via native container (Rust quick-xml). Returns page URLs and child sitemap URLs. */
export async function parseSitemap(env, xml) {
    return postJson(env, "/parse-sitemap", { xml });
}
/**
 * Perform web search using native Rust implementation.
 * - Uses SearXNG if SEARXNG_ENDPOINT is configured (env var)
 * - Falls back to DuckDuckGo if SearXNG is not available or returns no results
 * - Adapted from firecrawl/apps/api/src/search/v2/index.ts
 */
export async function nativeSearch(env, options) {
    const body = {
        query: options.query,
        num_results: options.num_results ?? 5,
        tbs: options.tbs,
        filter: options.filter,
        lang: options.lang ?? "en",
        country: options.country ?? "us",
        location: options.location,
        timeout_ms: options.timeout_ms ?? 5000,
    };
    return postJson(env, "/search", body);
}
/** Convert HTML to markdown via native container (Go library). */
export async function htmlToMarkdown(env, html) {
    return postJson(env, "/html-to-markdown", { html });
}
/**
 * Convert Office documents (docx, xlsx, odt, rtf, doc, xls) to HTML.
 * Firecrawl-compatible: same flow as firecrawl document engine.
 */
export async function convertDocument(env, params) {
    const { data, url, contentType } = params;
    const bytes = new Uint8Array(data);
    let binary = "";
    const chunk = 8192;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    const dataBase64 = btoa(binary);
    const body = { dataBase64 };
    if (url)
        body.url = url;
    if (contentType)
        body.contentType = contentType;
    return postJson(env, "/convert-document", body);
}
/**
 * Convert PDF to HTML.
 * Firecrawl-compatible: same flow as Firecrawl PDF engine (text → HTML for pipeline).
 */
export async function convertPdf(env, params) {
    const bytes = new Uint8Array(params.data);
    let binary = "";
    const chunk = 8192;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    const dataBase64 = btoa(binary);
    return postJson(env, "/convert-pdf", { dataBase64 });
}
export async function getPdfMetadata(env, input) {
    const binding = getBinding(env);
    const container = await getRandom(binding, 1);
    const body = typeof input === "object" && "pdfBase64" in input
        ? JSON.stringify({ pdfBase64: input.pdfBase64 })
        : new Uint8Array(input);
    const headers = typeof input === "object" && "pdfBase64" in input
        ? { "Content-Type": "application/json" }
        : {};
    const res = await container.fetch(new Request(`${CONTAINER_ORIGIN}/get-pdf-metadata`, {
        method: "POST",
        headers,
        body,
    }));
    const data = await res.json();
    if (!res.ok)
        throw new Error(String(data?.error ?? res.statusText));
    return data;
}
//# sourceMappingURL=native-container.js.map