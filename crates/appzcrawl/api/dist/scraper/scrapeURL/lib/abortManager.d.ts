export type AbortInstance = {
    signal: AbortSignal;
    timesOutAt?: Date;
    tier: "external" | "scrape" | "engine";
    throwable: () => any;
};
export declare class AbortManager {
    private aborts;
    private mappedController;
    private listeners;
    constructor(...instances: (AbortInstance | undefined | null)[]);
    dispose(): void;
    private resolveInner;
    private register;
    add(...instances: (AbortInstance | undefined | null)[]): void;
    child(...instances: (AbortInstance | undefined | null)[]): AbortManager;
    isAborted(): boolean;
    throwIfAborted(): void;
    private _mapController;
    asSignal(): AbortSignal;
    scrapeTimeout(): number | undefined;
    engineNearestTimeout(): number | undefined;
}
export declare class AbortManagerThrownError extends Error {
    name: string;
    tier: AbortInstance["tier"];
    inner: any;
    constructor(tier: AbortInstance["tier"], inner: any);
}
//# sourceMappingURL=abortManager.d.ts.map