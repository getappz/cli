import { collectCSSData } from "./css-data";
import { getStyleSnapshot } from "./elements";
import { findImages } from "./images";
import { getTypography, getBackgroundCandidates } from "./brand-utils";
interface BrandingResult {
    branding: {
        cssData: ReturnType<typeof collectCSSData>;
        snapshots: ReturnType<typeof getStyleSnapshot>[];
        images: ReturnType<typeof findImages>["images"];
        logoCandidates: ReturnType<typeof findImages>["logoCandidates"];
        brandName: string;
        pageTitle: string;
        pageUrl: string;
        typography: ReturnType<typeof getTypography>;
        frameworkHints: string[];
        colorScheme: "dark" | "light";
        pageBackground: string | null;
        backgroundCandidates: ReturnType<typeof getBackgroundCandidates>;
        errors?: Array<{
            context: string;
            message: string;
            timestamp: number;
        }>;
    };
}
export declare const extractBrandDesign: () => BrandingResult;
export {};
//# sourceMappingURL=index.d.ts.map