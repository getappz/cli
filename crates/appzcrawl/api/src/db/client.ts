import type { D1Database } from "@cloudflare/workers-types";
import { drizzle } from "drizzle-orm/d1";
import * as schema from "./schema";

export type DbSchema = typeof schema;
export type Database = ReturnType<typeof createDb>;

export function createDb(database: D1Database) {
  return drizzle(database, { schema });
}

export { schema };
