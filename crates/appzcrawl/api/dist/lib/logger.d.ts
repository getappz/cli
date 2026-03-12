/**
 * Workers-compatible logger. Uses console; no winston.
 */
export declare const logger: {
    child: (_meta: Record<string, unknown>) => /*elided*/ any;
    debug: (msg: string, meta?: Record<string, unknown>) => void;
    info: (msg: string, meta?: Record<string, unknown>) => void;
    warn: (msg: string, meta?: Record<string, unknown>) => void;
    error: (msg: string, meta?: Record<string, unknown>) => void;
};
//# sourceMappingURL=logger.d.ts.map