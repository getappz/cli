import { errors } from "./helpers";
import { collectCSSData } from "./css-data";
import { sampleElements, getStyleSnapshot } from "./elements";
import { findImages } from "./images";
import { getTypography, detectFrameworkHints, detectColorScheme, extractBrandName, getBackgroundCandidates, } from "./brand-utils";
export const extractBrandDesign = () => {
    const cssData = collectCSSData();
    const elements = sampleElements();
    const snapshots = elements.map(getStyleSnapshot);
    const imageData = findImages();
    const typography = getTypography();
    const frameworkHints = detectFrameworkHints();
    const colorScheme = detectColorScheme();
    const brandName = extractBrandName();
    const backgroundCandidates = getBackgroundCandidates();
    const pageBackground = backgroundCandidates.length > 0 ? backgroundCandidates[0].color : null;
    const pageTitle = document.title || "";
    const pageUrl = typeof window !== "undefined" && window.location
        ? window.location.href
        : "";
    return {
        branding: {
            cssData,
            snapshots,
            images: imageData.images,
            logoCandidates: imageData.logoCandidates,
            brandName,
            pageTitle,
            pageUrl,
            typography,
            frameworkHints,
            colorScheme,
            pageBackground,
            backgroundCandidates,
            errors: errors.length > 0 ? errors : undefined,
        },
    };
};
// Auto-execute when loaded in browser context (IIFE pattern)
(function __extractBrandDesign() {
    return extractBrandDesign();
})();
//# sourceMappingURL=index.js.map