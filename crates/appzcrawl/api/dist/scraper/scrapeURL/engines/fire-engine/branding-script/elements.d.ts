export declare const sampleElements: () => Element[];
export interface StyleSnapshot {
    tag: string;
    classes: string;
    text: string;
    rect: {
        w: number;
        h: number;
    };
    colors: {
        text: string;
        background: string;
        border: string;
        borderWidth: number | null;
        borderTop: string;
        borderTopWidth: number | null;
        borderRight: string;
        borderRightWidth: number | null;
        borderBottom: string;
        borderBottomWidth: number | null;
        borderLeft: string;
        borderLeftWidth: number | null;
    };
    typography: {
        fontStack: string[];
        size: string | null;
        weight: number | null;
    };
    radius: number | null;
    borderRadius: {
        topLeft: number | null;
        topRight: number | null;
        bottomRight: number | null;
        bottomLeft: number | null;
    };
    shadow: string | null;
    isButton: boolean;
    isNavigation: boolean;
    hasCTAIndicator: boolean;
    isInput: boolean;
    inputMetadata: InputMetadata | null;
    isLink: boolean;
}
interface InputMetadata {
    type: string;
    placeholder: string;
    value: string;
    required: boolean;
    disabled: boolean;
    name: string;
    id: string;
    label: string;
}
export declare const getStyleSnapshot: (el: Element) => StyleSnapshot;
export {};
//# sourceMappingURL=elements.d.ts.map