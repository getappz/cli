/**
 * Workers-compatible logger. Uses console; no winston.
 */
export const logger = {
  child: (_meta: Record<string, unknown>) => logger,
  debug: (msg: string, meta?: Record<string, unknown>) => {
    if (meta) console.debug(msg, meta);
    else console.debug(msg);
  },
  info: (msg: string, meta?: Record<string, unknown>) => {
    if (meta) console.info(msg, meta);
    else console.info(msg);
  },
  warn: (msg: string, meta?: Record<string, unknown>) => {
    if (meta) console.warn(msg, meta);
    else console.warn(msg);
  },
  error: (msg: string, meta?: Record<string, unknown>) => {
    if (meta) console.error(msg, meta);
    else console.error(msg);
  },
};
