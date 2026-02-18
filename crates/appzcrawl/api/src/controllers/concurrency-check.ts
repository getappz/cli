import type { Context } from "hono";
import type { AppEnv } from "../types";

export async function concurrencyCheckController(c: Context<AppEnv>) {
  return c.json({
    success: true,
    canRun: true,
    current: 0,
    limit: 10,
  });
}
