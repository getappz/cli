import type { Context } from "hono";
import type { AppEnv } from "../types";

export async function creditUsageController(c: Context<AppEnv>) {
  const account = c.get("account");
  return c.json({
    success: true,
    creditsUsed: 0,
    remainingCredits: account?.remainingCredits ?? 0,
  });
}

export async function creditUsageHistoricalController(c: Context<AppEnv>) {
  return c.json({ success: true, data: [] });
}
