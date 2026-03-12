// Error tracking
export const errors = [];
export const recordError = (context, error) => {
    errors.push({
        context: context,
        message: error && error.message
            ? error.message
            : String(error),
        timestamp: Date.now(),
    });
};
// Use native getComputedStyle so page scripts (e.g. MooTools) overwriting
// window.getComputedStyle don't break us. Guard so we never call .bind on undefined.
let nativeGetComputedStyle;
try {
    const gcs = typeof Window !== "undefined" &&
        Window.prototype &&
        Window.prototype.getComputedStyle;
    if (gcs && typeof gcs.bind === "function" && typeof window !== "undefined") {
        nativeGetComputedStyle = gcs.bind(window);
    }
    else if (typeof window !== "undefined" &&
        typeof window.getComputedStyle === "function") {
        nativeGetComputedStyle = window.getComputedStyle.bind(window);
    }
    else {
        nativeGetComputedStyle = () => ({});
    }
}
catch {
    nativeGetComputedStyle = () => ({});
}
// Style caching
const styleCache = new WeakMap();
function getComputedStyleSafe(el) {
    try {
        return nativeGetComputedStyle(el);
    }
    catch (e) {
        recordError("getComputedStyle", e);
        recordError("getComputedStyle:diagnostic", String(JSON.stringify({
            nodeType: el?.nodeType,
            constructor: el?.constructor?.name,
            isElement: el instanceof Element,
            hasOwnerDocument: !!el
                ?.ownerDocument,
        })));
        return nativeGetComputedStyle(document.documentElement);
    }
}
export const getComputedStyleCached = (el) => {
    if (!el || typeof el !== "object" || !(el instanceof Element)) {
        return getComputedStyleSafe(document.documentElement);
    }
    if (styleCache.has(el)) {
        return styleCache.get(el);
    }
    const style = getComputedStyleSafe(el);
    styleCache.set(el, style);
    return style;
};
// Unit conversion
export const toPx = (v) => {
    if (!v || v === "auto")
        return null;
    if (v.endsWith("px"))
        return parseFloat(v);
    if (v.endsWith("rem"))
        return (parseFloat(v) *
            parseFloat(getComputedStyleSafe(document.documentElement).fontSize || "16"));
    if (v.endsWith("em"))
        return (parseFloat(v) *
            parseFloat(getComputedStyleSafe(document.body ?? document.documentElement).fontSize || "16"));
    if (v.endsWith("%"))
        return null;
    const num = parseFloat(v);
    return Number.isFinite(num) ? num : null;
};
// Class name extraction (handles SVG elements)
export const getClassNameString = (el) => {
    if (!el || !el.className)
        return "";
    try {
        const className = el.className;
        if (className && typeof className === "object" && "baseVal" in className) {
            return String(className.baseVal || "");
        }
        if (typeof className === "string") {
            return className;
        }
        if (className &&
            typeof className.toString === "function") {
            return String(className);
        }
        return String(className || "");
    }
    catch (e) {
        return "";
    }
};
//# sourceMappingURL=helpers.js.map