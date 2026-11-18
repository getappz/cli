import { WorkerEntrypoint } from "cloudflare:workers";
import { type Converted, convertHtml } from "./convert";

export type { Converted };

/**
 * Service-bound RPC: serve calls `env.MD.convert(html, origin)`.
 * Resolves null on ANY failure so callers fall back to the HTML response.
 */
export class Html2Md extends WorkerEntrypoint {
  async convert(html: ArrayBuffer, origin: string): Promise<Converted | null> {
    try {
      return await convertHtml(html, origin);
    } catch {
      return null;
    }
  }
}

export default {
  fetch(): Response {
    return new Response("Not found", { status: 404 });
  },
};
