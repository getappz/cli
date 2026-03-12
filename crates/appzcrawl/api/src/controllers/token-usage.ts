import type { Context } from "hono";
import type { AppEnv } from "../types";

export async function tokenUsageController(c: Context<AppEnv>) {
  return c.json({
    success: true,
    tokensUsed: 0,
    remainingTokens: 0,
  });
}

export async function tokenUsageHistoricalController(c: Context<AppEnv>) {
  return c.json({ success: true, data: [] });
}
