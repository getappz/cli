export function requestTimingMiddleware(version) {
    return (c, next) => {
        c.set("requestTiming", { startTime: Date.now(), version });
        return next();
    };
}
//# sourceMappingURL=timing.js.map