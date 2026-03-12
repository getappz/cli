import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
import { v2Router } from "./routes/v2";
import { provisionTeam } from "./services/key-management";
import type { AppEnv } from "./types";

const app = new Hono<AppEnv>();

app.use("*", logger());
app.use(
  "*",
  cors({
    origin: ["*"],
    allowMethods: ["GET", "POST", "DELETE", "OPTIONS"],
    allowHeaders: [
      "Content-Type",
      "Authorization",
      "x-idempotency-key",
      "X-Dev-Create-Key",
    ],
  }),
);

app.get("/", (c) => c.json({ name: "appzcrawl-api", version: "0.0.0" }));
app.get("/health", (c) => c.json({ ok: true }));

// Dev-only helper: provision a team + API key for local testing.
// When DEV_CREATE_KEY is set, caller must pass it in X-Dev-Create-Key header (allows remote curl).
app.post("/dev/create-api-key", async (c) => {
  if (c.env.ENVIRONMENT && c.env.ENVIRONMENT !== "devel") {
    return c.json({ success: false, error: "Forbidden outside devel" }, 403);
  }

  const devCreateKey = c.env.DEV_CREATE_KEY;
  if (devCreateKey) {
    const headerKey = c.req.header("X-Dev-Create-Key");
    if (headerKey !== devCreateKey) {
      return c.json(
        { success: false, error: "Invalid or missing X-Dev-Create-Key header" },
        401,
      );
    }
  }

  let body: { teamId?: string } = {};
  try {
    body = await c.req.json();
  } catch {
    // empty/invalid JSON is okay; defaults apply
  }

  const teamId = body.teamId?.trim() || "default-team";

  try {
    const result = await provisionTeam(c.env.DB, {
      teamId,
      teamName: teamId,
      initialCredits: 999_999,
      createdBy: "dev-endpoint",
    });

    return c.json({
      success: true,
      apiKey: result.fullKey,
      keyPrefix: result.keyPrefix,
      teamId,
      message: "Save this key securely. It will not be shown again.",
    });
  } catch (e) {
    return c.json(
      {
        success: false,
        error: e instanceof Error ? e.message : "Failed to provision team",
      },
      500,
    );
  }
});

app.route("/v2", v2Router);

app.onError((err, c) => {
  return c.json(
    {
      success: false,
      code: "INTERNAL_ERROR",
      error: err.message ?? "Internal error",
    },
    500,
  );
});

export { app };
