import { Logger } from "winston";
import { z } from "zod";
import { MockState } from "./mock";
type RobustFetchParams<Schema extends z.Schema<any>> = {
    url: string;
    logger: Logger;
    method: "GET" | "POST" | "DELETE" | "PUT";
    body?: any;
    headers?: Record<string, string>;
    schema?: Schema;
    dontParseResponse?: boolean;
    ignoreResponse?: boolean;
    ignoreFailure?: boolean;
    ignoreFailureStatus?: boolean;
    requestId?: string;
    tryCount?: number;
    tryCooldown?: number;
    mock: MockState | null;
    abort?: AbortSignal;
    useCacheableLookup?: boolean;
};
export declare function robustFetch<Schema extends z.Schema<any>, Output = z.infer<Schema>>({ url, logger, method, body, headers, schema, ignoreResponse, ignoreFailure, ignoreFailureStatus, requestId, tryCount, tryCooldown, mock, abort, useCacheableLookup, }: RobustFetchParams<Schema>): Promise<Output>;
export {};
//# sourceMappingURL=fetch.d.ts.map