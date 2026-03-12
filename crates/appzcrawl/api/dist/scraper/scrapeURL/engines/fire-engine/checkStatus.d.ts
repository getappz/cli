import { Logger } from "winston";
import { z } from "zod";
import { MockState } from "../../lib/mock";
import { Meta } from "../..";
declare const successSchema: z.ZodObject<{
    jobId: z.ZodString;
    state: z.ZodLiteral<"completed">;
    processing: z.ZodLiteral<false>;
    content: z.ZodString;
    url: z.ZodOptional<z.ZodString>;
    pageStatusCode: z.ZodNumber;
    pageError: z.ZodOptional<z.ZodString>;
    responseHeaders: z.ZodOptional<z.ZodRecord<z.ZodString, z.ZodString>>;
    screenshot: z.ZodOptional<z.ZodString>;
    screenshots: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    actionContent: z.ZodOptional<z.ZodArray<z.ZodObject<{
        url: z.ZodString;
        html: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        html: string;
        url: string;
    }, {
        html: string;
        url: string;
    }>, "many">>;
    actionResults: z.ZodOptional<z.ZodArray<z.ZodUnion<[z.ZodObject<{
        idx: z.ZodNumber;
        type: z.ZodLiteral<"screenshot">;
        result: z.ZodObject<{
            path: z.ZodString;
        }, "strip", z.ZodTypeAny, {
            path: string;
        }, {
            path: string;
        }>;
    }, "strip", z.ZodTypeAny, {
        type: "screenshot";
        result: {
            path: string;
        };
        idx: number;
    }, {
        type: "screenshot";
        result: {
            path: string;
        };
        idx: number;
    }>, z.ZodObject<{
        idx: z.ZodNumber;
        type: z.ZodLiteral<"scrape">;
        result: z.ZodUnion<[z.ZodObject<{
            url: z.ZodString;
            html: z.ZodString;
        }, "strip", z.ZodTypeAny, {
            html: string;
            url: string;
        }, {
            html: string;
            url: string;
        }>, z.ZodObject<{
            url: z.ZodString;
            accessibility: z.ZodString;
        }, "strip", z.ZodTypeAny, {
            url: string;
            accessibility: string;
        }, {
            url: string;
            accessibility: string;
        }>]>;
    }, "strip", z.ZodTypeAny, {
        type: "scrape";
        result: {
            html: string;
            url: string;
        } | {
            url: string;
            accessibility: string;
        };
        idx: number;
    }, {
        type: "scrape";
        result: {
            html: string;
            url: string;
        } | {
            url: string;
            accessibility: string;
        };
        idx: number;
    }>, z.ZodObject<{
        idx: z.ZodNumber;
        type: z.ZodLiteral<"executeJavascript">;
        result: z.ZodObject<{
            return: z.ZodString;
        }, "strip", z.ZodTypeAny, {
            return: string;
        }, {
            return: string;
        }>;
    }, "strip", z.ZodTypeAny, {
        type: "executeJavascript";
        result: {
            return: string;
        };
        idx: number;
    }, {
        type: "executeJavascript";
        result: {
            return: string;
        };
        idx: number;
    }>, z.ZodObject<{
        idx: z.ZodNumber;
        type: z.ZodLiteral<"pdf">;
        result: z.ZodObject<{
            link: z.ZodString;
        }, "strip", z.ZodTypeAny, {
            link: string;
        }, {
            link: string;
        }>;
    }, "strip", z.ZodTypeAny, {
        type: "pdf";
        result: {
            link: string;
        };
        idx: number;
    }, {
        type: "pdf";
        result: {
            link: string;
        };
        idx: number;
    }>]>, "many">>;
    file: z.ZodUnion<[z.ZodOptional<z.ZodObject<{
        name: z.ZodString;
        content: z.ZodString;
    }, "strip", z.ZodTypeAny, {
        content: string;
        name: string;
    }, {
        content: string;
        name: string;
    }>>, z.ZodNull]>;
    docUrl: z.ZodOptional<z.ZodString>;
    usedMobileProxy: z.ZodOptional<z.ZodBoolean>;
    youtubeTranscriptContent: z.ZodOptional<z.ZodAny>;
    timezone: z.ZodOptional<z.ZodString>;
}, "strip", z.ZodTypeAny, {
    jobId: string;
    pageStatusCode: number;
    processing: false;
    state: "completed";
    content: string;
    screenshot?: string | undefined;
    url?: string | undefined;
    youtubeTranscriptContent?: any;
    timezone?: string | undefined;
    pageError?: string | undefined;
    responseHeaders?: Record<string, string> | undefined;
    screenshots?: string[] | undefined;
    actionContent?: {
        html: string;
        url: string;
    }[] | undefined;
    actionResults?: ({
        type: "screenshot";
        result: {
            path: string;
        };
        idx: number;
    } | {
        type: "scrape";
        result: {
            html: string;
            url: string;
        } | {
            url: string;
            accessibility: string;
        };
        idx: number;
    } | {
        type: "executeJavascript";
        result: {
            return: string;
        };
        idx: number;
    } | {
        type: "pdf";
        result: {
            link: string;
        };
        idx: number;
    })[] | undefined;
    file?: {
        content: string;
        name: string;
    } | null | undefined;
    docUrl?: string | undefined;
    usedMobileProxy?: boolean | undefined;
}, {
    jobId: string;
    pageStatusCode: number;
    processing: false;
    state: "completed";
    content: string;
    screenshot?: string | undefined;
    url?: string | undefined;
    youtubeTranscriptContent?: any;
    timezone?: string | undefined;
    pageError?: string | undefined;
    responseHeaders?: Record<string, string> | undefined;
    screenshots?: string[] | undefined;
    actionContent?: {
        html: string;
        url: string;
    }[] | undefined;
    actionResults?: ({
        type: "screenshot";
        result: {
            path: string;
        };
        idx: number;
    } | {
        type: "scrape";
        result: {
            html: string;
            url: string;
        } | {
            url: string;
            accessibility: string;
        };
        idx: number;
    } | {
        type: "executeJavascript";
        result: {
            return: string;
        };
        idx: number;
    } | {
        type: "pdf";
        result: {
            link: string;
        };
        idx: number;
    })[] | undefined;
    file?: {
        content: string;
        name: string;
    } | null | undefined;
    docUrl?: string | undefined;
    usedMobileProxy?: boolean | undefined;
}>;
export type FireEngineCheckStatusSuccess = z.infer<typeof successSchema>;
export declare class StillProcessingError extends Error {
    constructor(jobId: string);
}
export declare function fireEngineCheckStatus(meta: Meta, logger: Logger, jobId: string, mock: MockState | null, abort?: AbortSignal, production?: boolean): Promise<FireEngineCheckStatusSuccess>;
export {};
//# sourceMappingURL=checkStatus.d.ts.map