/**
 * Workers-compatible logger. Uses console; no winston.
 */
export const logger = {
    child: (_meta) => logger,
    debug: (msg, meta) => {
        if (meta)
            console.debug(msg, meta);
        else
            console.debug(msg);
    },
    info: (msg, meta) => {
        if (meta)
            console.info(msg, meta);
        else
            console.info(msg);
    },
    warn: (msg, meta) => {
        if (meta)
            console.warn(msg, meta);
        else
            console.warn(msg);
    },
    error: (msg, meta) => {
        if (meta)
            console.error(msg, meta);
        else
            console.error(msg);
    },
};
//# sourceMappingURL=logger.js.map