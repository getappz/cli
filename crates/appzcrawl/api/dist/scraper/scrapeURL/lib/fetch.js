import { ZodError } from "zod";
import * as Sentry from "@sentry/node";
import { saveMock } from "./mock";
import { fireEngineURL } from "../engines/fire-engine/scrape";
import { fetch, FormData, Agent } from "undici";
import { cacheableLookup } from "./cacheableLookup";
import dns from "dns";
import { AbortManagerThrownError } from "./abortManager";
const robustAgent = new Agent({
    headersTimeout: 0,
    bodyTimeout: 0,
    connect: {
        lookup: cacheableLookup.lookup,
    },
});
const robustAgentNoLookup = new Agent({
    headersTimeout: 0,
    bodyTimeout: 0,
    connect: {
        lookup: dns.lookup,
    },
});
export async function robustFetch({ url, logger, method = "GET", body, headers, schema, ignoreResponse = false, ignoreFailure = false, ignoreFailureStatus = false, requestId = crypto.randomUUID(), tryCount = 1, tryCooldown, mock, abort, useCacheableLookup = true, }) {
    abort?.throwIfAborted();
    const params = {
        url,
        logger,
        method,
        body,
        headers,
        schema,
        ignoreResponse,
        ignoreFailure,
        ignoreFailureStatus,
        tryCount,
        tryCooldown,
        abort,
    };
    // omit pdf file content from logs
    const logParams = {
        ...params,
        body: body?.input
            ? {
                ...body,
                input: {
                    ...body.input,
                    file_content: undefined,
                },
            }
            : body,
        logger: undefined,
    };
    let response;
    if (mock === null) {
        let request;
        try {
            request = await fetch(url, {
                method,
                headers: {
                    ...(body instanceof FormData
                        ? {}
                        : body !== undefined
                            ? {
                                "Content-Type": "application/json",
                            }
                            : {}),
                    ...(headers !== undefined ? headers : {}),
                },
                signal: abort,
                dispatcher: useCacheableLookup ? robustAgent : robustAgentNoLookup,
                ...(body instanceof FormData
                    ? {
                        body,
                    }
                    : body !== undefined
                        ? {
                            body: JSON.stringify(body),
                        }
                        : {}),
            });
        }
        catch (error) {
            if (error instanceof AbortManagerThrownError) {
                throw error;
            }
            else if (!ignoreFailure) {
                Sentry.captureException(error);
                if (tryCount > 1) {
                    logger.debug("Request failed, trying " + (tryCount - 1) + " more times", { params: logParams, error, requestId });
                    return await robustFetch({
                        ...params,
                        requestId,
                        tryCount: tryCount - 1,
                        mock,
                    });
                }
                else {
                    logger.debug("Request failed", {
                        params: logParams,
                        error,
                        requestId,
                    });
                    throw new Error("Request failed", {
                        cause: {
                            params,
                            requestId,
                            error,
                        },
                    });
                }
            }
            else {
                return null;
            }
        }
        if (ignoreResponse === true) {
            return null;
        }
        const resp = await request.text();
        response = {
            status: request.status,
            headers: request.headers,
            body: resp, // NOTE: can this throw an exception?
        };
    }
    else {
        if (ignoreResponse === true) {
            return null;
        }
        const makeRequestTypeId = (request) => {
            let trueUrl = request.url.startsWith(fireEngineURL)
                ? request.url.replace(fireEngineURL, "<fire-engine>")
                : request.url;
            let out = trueUrl + ";" + request.method;
            if (trueUrl.startsWith("<fire-engine>") && request.method === "POST") {
                out += "f-e;" + request.body?.engine + ";" + request.body?.url;
            }
            return out;
        };
        const thisId = makeRequestTypeId(params);
        const matchingMocks = mock.requests
            .filter(x => makeRequestTypeId(x.options) === thisId)
            .sort((a, b) => a.time - b.time);
        const nextI = mock.tracker[thisId] ?? 0;
        mock.tracker[thisId] = nextI + 1;
        if (!matchingMocks[nextI]) {
            throw new Error("Failed to mock request -- no mock targets found.");
        }
        response = {
            ...matchingMocks[nextI].result,
            headers: new Headers(matchingMocks[nextI].result.headers),
        };
    }
    if (response.status >= 300 && !ignoreFailureStatus) {
        if (tryCount > 1) {
            logger.debug("Request sent failure status, trying " + (tryCount - 1) + " more times", {
                params: logParams,
                response: { status: response.status, body: response.body },
                requestId,
            });
            if (tryCooldown !== undefined) {
                let timeoutHandle = null;
                try {
                    await new Promise(resolve => {
                        timeoutHandle = setTimeout(() => resolve(null), tryCooldown);
                    });
                }
                finally {
                    if (timeoutHandle) {
                        clearTimeout(timeoutHandle);
                    }
                }
            }
            return await robustFetch({
                ...params,
                requestId,
                tryCount: tryCount - 1,
                mock,
            });
        }
        else {
            logger.debug("Request sent failure status", {
                params: logParams,
                response: { status: response.status, body: response.body },
                requestId,
            });
            throw new Error("Request sent failure status", {
                cause: {
                    params: logParams,
                    response: { status: response.status, body: response.body },
                    requestId,
                },
            });
        }
    }
    if (mock === null) {
        await saveMock({
            ...params,
            logger: undefined,
            schema: undefined,
            headers: undefined,
        }, response);
    }
    let data;
    try {
        data = JSON.parse(response.body);
    }
    catch (error) {
        logger.debug("Request sent malformed JSON", {
            params: logParams,
            response: { status: response.status, body: response.body },
            requestId,
        });
        throw new Error("Request sent malformed JSON", {
            cause: {
                params: logParams,
                response,
                requestId,
            },
        });
    }
    if (schema) {
        try {
            data = schema.parse(data);
        }
        catch (error) {
            if (error instanceof ZodError) {
                logger.debug("Response does not match provided schema", {
                    params: logParams,
                    response: { status: response.status, body: response.body },
                    requestId,
                    error,
                    schema,
                });
                throw new Error("Response does not match provided schema", {
                    cause: {
                        params: logParams,
                        response,
                        requestId,
                        error,
                        schema,
                    },
                });
            }
            else {
                logger.debug("Parsing response with provided schema failed", {
                    params: logParams,
                    response: { status: response.status, body: response.body },
                    requestId,
                    error,
                    schema,
                });
                throw new Error("Parsing response with provided schema failed", {
                    cause: {
                        params: logParams,
                        response,
                        requestId,
                        error,
                        schema,
                    },
                });
            }
        }
    }
    return data;
}
//# sourceMappingURL=fetch.js.map