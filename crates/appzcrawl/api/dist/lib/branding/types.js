/**
 * Branding types - adapted from Firecrawl for Workers.
 */
export function calculateLogoArea(position) {
    if (!position)
        return 0;
    return (position.width || 0) * (position.height || 0);
}
//# sourceMappingURL=types.js.map