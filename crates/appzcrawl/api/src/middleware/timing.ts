import type { Context, Next } from "hono";
import type { AppEnv } from "../types";

export function requestTimingMiddleware(version: string) {
  return (c: Context<AppEnv>, next: Next) => {
    c.set("requestTiming", { startTime: Date.now(), version });
    return next();
  };
}
