export interface Typography {
    stacks: {
        body: string[];
        heading: string[];
        paragraph: string[];
    };
    sizes: {
        h1: string;
        h2: string;
        body: string;
    };
}
export declare const getTypography: () => Typography;
export declare const detectFrameworkHints: () => string[];
export declare const detectColorScheme: () => "dark" | "light";
export declare const extractBrandName: () => string;
export declare const normalizeColor: (color: string | null | undefined) => string | null;
export declare const isValidBackgroundColor: (color: string | null | undefined) => boolean;
export interface BackgroundCandidate {
    color: string;
    source: string;
    priority: number;
    area?: number;
}
export declare const getBackgroundCandidates: () => BackgroundCandidate[];
//# sourceMappingURL=brand-utils.d.ts.map