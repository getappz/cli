interface LogoCandidate {
    src: string;
    alt: string;
    ariaLabel?: string;
    title?: string;
    isSvg: boolean;
    isVisible: boolean;
    location: "header" | "body";
    position: {
        top: number;
        left: number;
        width: number;
        height: number;
    };
    indicators: {
        inHeader: boolean;
        altMatch: boolean;
        srcMatch: boolean;
        classMatch: boolean;
        hrefMatch: boolean;
    };
    href?: string;
    source: string;
    logoSvgScore: number;
}
interface ImageData {
    type: string;
    src: string;
}
export interface FindImagesResult {
    images: ImageData[];
    logoCandidates: LogoCandidate[];
}
export declare const findImages: () => FindImagesResult;
export {};
//# sourceMappingURL=images.d.ts.map