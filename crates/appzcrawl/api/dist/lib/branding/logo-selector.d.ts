/**
 * Heuristic logo selection - no LLM dependency.
 */
export interface LogoCandidate {
    src: string;
    alt: string;
    isSvg: boolean;
    isVisible: boolean;
    location: "header" | "body" | "footer";
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
}
interface LogoSelectionResult {
    selectedIndex: number;
    confidence: number;
    method: "heuristic" | "llm" | "fallback";
    reasoning: string;
}
export declare function selectLogoWithConfidence(candidates: LogoCandidate[], brandName?: string): LogoSelectionResult;
export declare function getTopCandidatesForLLM(candidates: LogoCandidate[], maxCandidates?: number): {
    filteredCandidates: LogoCandidate[];
    indexMap: Map<number, number>;
};
export {};
//# sourceMappingURL=logo-selector.d.ts.map