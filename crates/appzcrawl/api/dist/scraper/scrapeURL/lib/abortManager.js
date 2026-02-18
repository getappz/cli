export class AbortManager {
    aborts = [];
    mappedController = null;
    listeners = [];
    constructor(...instances) {
        this.aborts = instances.filter(x => x !== undefined && x !== null);
    }
    dispose() {
        for (const { signal, handler } of this.listeners) {
            signal.removeEventListener("abort", handler);
        }
        this.listeners = [];
        this.aborts = [];
        this.mappedController = null;
    }
    resolveInner(abort) {
        try {
            return abort.throwable();
        }
        catch (err) {
            return err;
        }
    }
    register(abort) {
        const handler = () => {
            if (!this.mappedController)
                return;
            const inner = this.resolveInner(abort);
            const reason = new AbortManagerThrownError(abort.tier, inner);
            this.mappedController.abort(reason);
        };
        abort.signal.addEventListener("abort", handler);
        this.listeners.push({ signal: abort.signal, handler });
    }
    add(...instances) {
        const pureInstances = instances.filter(x => x !== undefined && x !== null);
        this.aborts.push(...pureInstances);
        if (this.mappedController !== null) {
            for (const abort of pureInstances) {
                this.register(abort);
            }
        }
    }
    child(...instances) {
        const manager = new AbortManager(...this.aborts, ...instances.filter(x => x !== undefined && x !== null));
        return manager;
    }
    isAborted() {
        return this.aborts.some(x => x.signal.aborted);
    }
    throwIfAborted() {
        for (const abort of this.aborts) {
            if (abort.signal.aborted) {
                const inner = this.resolveInner(abort);
                throw new AbortManagerThrownError(abort.tier, inner);
            }
        }
    }
    _mapController() {
        this.mappedController = new AbortController();
        for (const abort of this.aborts) {
            this.register(abort);
        }
    }
    asSignal() {
        if (this.mappedController === null) {
            this._mapController();
        }
        return this.mappedController.signal;
    }
    scrapeTimeout() {
        const timeouts = this.aborts
            .filter(x => x.tier === "scrape")
            .map(x => x.timesOutAt)
            .filter(x => x !== undefined);
        if (timeouts.length === 0) {
            return undefined;
        }
        return Math.min(...timeouts.map(x => x.getTime())) - Date.now();
    }
    engineNearestTimeout() {
        const timeouts = this.aborts
            .filter(x => x.tier === "engine")
            .map(x => x.timesOutAt)
            .filter(x => x !== undefined);
        if (timeouts.length === 0) {
            return undefined;
        }
        return Math.min(...timeouts.map(x => x.getTime())) - Date.now();
    }
}
export class AbortManagerThrownError extends Error {
    name = "AbortManagerThrownError";
    tier;
    inner;
    constructor(tier, inner) {
        super("AbortManagerThrownError: " + tier);
        this.tier = tier;
        this.inner = inner;
    }
}
//# sourceMappingURL=abortManager.js.map