export async function concurrencyCheckController(c) {
    return c.json({
        success: true,
        canRun: true,
        current: 0,
        limit: 10,
    });
}
//# sourceMappingURL=concurrency-check.js.map