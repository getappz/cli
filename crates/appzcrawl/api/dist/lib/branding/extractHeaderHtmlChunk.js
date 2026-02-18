/**
 * Extract a small chunk of HTML from the top/header area for LLM context when
 * no logo candidates were found.
 */
const MAX_HEADER_CHUNK_CHARS = 5500;
function stripNoise(html) {
    let out = html;
    out = out.replace(/<!--[\s\S]*?-->/g, "");
    out = out.replace(/<script\b[\s\S]*?<\/script>/gi, "");
    out = out.replace(/<style\b[\s\S]*?<\/style>/gi, "");
    return out;
}
function findHeaderStart(html) {
    const lower = html.toLowerCase();
    const header = lower.indexOf("<header");
    const nav = lower.indexOf("<nav");
    const body = lower.indexOf("<body");
    const indices = [header, nav, body].filter((i) => i >= 0);
    return indices.length > 0 ? Math.min(...indices) : 0;
}
export function extractHeaderHtmlChunk(html) {
    if (!html || typeof html !== "string")
        return "";
    const stripped = stripNoise(html);
    const start = findHeaderStart(stripped);
    const chunk = stripped.slice(start, start + MAX_HEADER_CHUNK_CHARS);
    return chunk.replace(/\s+/g, " ").trim();
}
//# sourceMappingURL=extractHeaderHtmlChunk.js.map