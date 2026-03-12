export async function creditUsageController(c) {
    const account = c.get("account");
    return c.json({
        success: true,
        creditsUsed: 0,
        remainingCredits: account?.remainingCredits ?? 0,
    });
}
export async function creditUsageHistoricalController(c) {
    return c.json({ success: true, data: [] });
}
//# sourceMappingURL=credit-usage.js.map