import type { Context } from "hono";
import type { AppEnv } from "../types";

export async function queueStatusController(c: Context<AppEnv>) {
  return c.json({
    success: true,
    queue: [],
  });
}
