import type { D1Database } from "@cloudflare/workers-types";
import * as schema from "./schema";
export type DbSchema = typeof schema;
export type Database = ReturnType<typeof createDb>;
export declare function createDb(database: D1Database): import("drizzle-orm/d1").DrizzleD1Database<typeof schema> & {
    $client: D1Database;
};
export { schema };
//# sourceMappingURL=client.d.ts.map