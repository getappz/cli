import type { Context, Next } from "hono";
import type { AppEnv } from "../types";
/** Stub: no country restriction on Workers. Could use c.req.raw.cf?.country later. */
export declare function countryCheck(_c: Context<AppEnv>, next: Next): Promise<void>;
//# sourceMappingURL=country.d.ts.map