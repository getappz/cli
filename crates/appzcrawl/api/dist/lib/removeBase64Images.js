/**
 * Firecrawl-compatible base64 image removal from markdown.
 * Replaces data:image/*;base64,... URLs with a placeholder while keeping alt text.
 */
const REGEX = /(!\[.*?\])\(data:image\/.*?;base64,.*?\)/g;
const PLACEHOLDER = "$1(<Base64-Image-Removed>)";
export function removeBase64ImagesFromMarkdown(markdown) {
    return markdown.replace(REGEX, PLACEHOLDER);
}
//# sourceMappingURL=removeBase64Images.js.map