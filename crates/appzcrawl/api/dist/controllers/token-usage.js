export async function tokenUsageController(c) {
    return c.json({
        success: true,
        tokensUsed: 0,
        remainingTokens: 0,
    });
}
export async function tokenUsageHistoricalController(c) {
    return c.json({ success: true, data: [] });
}
//# sourceMappingURL=token-usage.js.map