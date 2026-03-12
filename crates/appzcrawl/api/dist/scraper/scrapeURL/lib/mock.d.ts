import { Logger } from "winston";
export declare function saveMock(options: unknown, result: unknown): Promise<void>;
export type MockState = {
    requests: {
        time: number;
        options: {
            url: string;
            method: string;
            body?: any;
            ignoreResponse: boolean;
            ignoreFailure: boolean;
            tryCount: number;
            tryCooldown?: number;
        };
        result: any;
    }[];
    tracker: Record<string, number>;
};
export declare function loadMock(name: string, logger?: Logger): Promise<MockState | null>;
//# sourceMappingURL=mock.d.ts.map