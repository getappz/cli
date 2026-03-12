/**
 * Engine processor — calls appzcrawl-engine (workers-rs) via RPC Service Binding.
 *
 * Each function calls the corresponding `rpc_*` export on the engine Worker.
 * All data crosses the binding as JSON strings — no HTTP overhead.
 *
 * The engine Worker compiles firecrawl_rs + htmd to WASM and runs as a
 * standalone Worker, giving it its own CPU/memory budget separate from
 * the main appzcrawl API Worker.
 */
// ---------------------------------------------------------------------------
// Availability check
// ---------------------------------------------------------------------------
/** Returns `true` when the APPZCRAWL_ENGINE binding is present. */
export function isAvailable(env) {
    return !!env.APPZCRAWL_ENGINE;
}
function getEngine(env) {
    return env
        .APPZCRAWL_ENGINE;
}
// ---------------------------------------------------------------------------
// Helper to parse RPC result (JSON string) and check for errors
// ---------------------------------------------------------------------------
function parseResult(json) {
    const parsed = JSON.parse(json);
    if (parsed && typeof parsed === "object" && "error" in parsed) {
        throw new Error(parsed.error);
    }
    return parsed;
}
// ---------------------------------------------------------------------------
// Public API — matches wasm-processor.ts / native-container.ts signatures
// ---------------------------------------------------------------------------
export async function extractLinks(env, html) {
    const result = await getEngine(env).rpc_extract_links(html ?? "");
    return { links: parseResult(result) };
}
export async function extractBaseHref(env, html, baseUrl) {
    const result = await getEngine(env).rpc_extract_base_href(html, baseUrl);
    return { baseHref: parseResult(result) };
}
export async function extractMetadata(env, html) {
    const result = await getEngine(env).rpc_extract_metadata(html ?? "");
    return { metadata: parseResult(result) };
}
export async function transformHtml(env, params) {
    const opts = JSON.stringify({
        html: params.html,
        url: params.url,
        include_tags: params.include_tags ?? [],
        exclude_tags: params.exclude_tags ?? [],
        only_main_content: params.only_main_content ?? false,
        omce_signatures: params.omce_signatures ?? null,
    });
    const result = await getEngine(env).rpc_transform_html(opts);
    return { html: parseResult(result) };
}
export async function getInnerJson(env, html) {
    const result = await getEngine(env).rpc_get_inner_json(html);
    return { content: parseResult(result) };
}
export async function extractAttributes(env, html, options) {
    const result = await getEngine(env).rpc_extract_attributes(html, JSON.stringify({ selectors: options.selectors }));
    return { results: parseResult(result) };
}
export async function extractImages(env, html, baseUrl) {
    const result = await getEngine(env).rpc_extract_images(html, baseUrl);
    return { images: parseResult(result) };
}
export async function extractAssets(env, html, baseUrl, formats) {
    const result = await getEngine(env).rpc_extract_assets(html, baseUrl, JSON.stringify(formats ?? ["assets"]));
    return parseResult(result);
}
export async function postProcessMarkdown(env, markdown, options) {
    const result = await getEngine(env).rpc_post_process_markdown(markdown, options?.baseUrl ?? "", options?.citations ?? false);
    return { markdown: parseResult(result) };
}
export async function htmlToMarkdown(env, html) {
    const result = await getEngine(env).rpc_html_to_markdown(html);
    return { markdown: parseResult(result) };
}
export async function filterLinks(env, params) {
    const body = {
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
    };
    const result = await getEngine(env).rpc_filter_links(JSON.stringify(body));
    const parsed = parseResult(result);
    return { links: parsed.links, denialReasons: parsed.denial_reasons ?? {} };
}
export async function parseSitemap(env, xml) {
    const result = await getEngine(env).rpc_parse_sitemap(xml);
    return parseResult(result);
}
export async function nativeSearch(env, options) {
    const opts = JSON.stringify({
        num_results: options.num_results ?? 5,
        tbs: options.tbs,
        filter: options.filter,
        lang: options.lang ?? "en",
        country: options.country ?? "us",
        location: options.location,
        timeout_ms: options.timeout_ms ?? 5000,
    });
    const result = await getEngine(env).rpc_search(options.query, opts);
    return parseResult(result);
}
export async function convertDocument(env, params) {
    // Encode data to base64
    const bytes = new Uint8Array(params.data);
    let binary = "";
    const chunk = 8192;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    const dataBase64 = btoa(binary);
    const body = JSON.stringify({
        dataBase64,
        url: params.url,
        contentType: params.contentType,
    });
    const result = await getEngine(env).rpc_convert_document(body);
    return { html: parseResult(result) };
}
//# sourceMappingURL=engine-processor.js.map