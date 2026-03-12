import type { Context, Next } from "hono";
import type { AppEnv } from "../types";

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isValidJobId(jobId: string | undefined): jobId is string {
  return typeof jobId === "string" && UUID_REGEX.test(jobId);
}

export function validateJobIdParam(c: Context<AppEnv>, next: Next) {
  const jobId = c.req.param("jobId");
  if (!isValidJobId(jobId)) {
    return c.json(
      {
        success: false,
        error: "Invalid job ID format. Job ID must be a valid UUID.",
      },
      400,
    );
  }
  return next();
}
