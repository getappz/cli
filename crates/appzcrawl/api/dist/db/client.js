import { drizzle } from "drizzle-orm/d1";
import * as schema from "./schema";
export function createDb(database) {
    return drizzle(database, { schema });
}
export { schema };
//# sourceMappingURL=client.js.map