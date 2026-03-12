const regex = /(!\[.*?\])\(data:image\/.*?;base64,.*?\)/g;
export function removeBase64Images(meta, document) {
    if (meta.options.removeBase64Images && document.markdown !== undefined) {
        document.markdown = document.markdown.replace(regex, "$1(<Base64-Image-Removed>)");
    }
    return document;
}
//# sourceMappingURL=removeBase64Images.js.map