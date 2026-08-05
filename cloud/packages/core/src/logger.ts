/**
 * A minimal structured logger — one JSON object per line, to stdout (info/
 * debug) or stderr (warn/error).
 *
 * The worker runs as a bare container process with no built-in request
 * logging the way `apps/web` gets from Vercel; whatever it prints to stdout
 * is whatever the host's log driver captures, and nothing before this parsed
 * it as anything but text. A JSON line per event is the minimum a host log
 * aggregator (Fly, Railway, Render, `docker logs`, CloudWatch) needs to filter
 * and query by field instead of grepping strings — without adding a network
 * dependency or an account this code cannot provision on its own.
 *
 * Deliberately not a wrapper around pino/winston: the worker's entire logging
 * surface is a handful of call sites (see index.ts and jobs/purgeArtifacts.ts),
 * and every field a log aggregator needs (level, time, a structured message,
 * bound context) fits in about thirty lines with zero dependencies. Reach for
 * a real logging library if that surface grows enough to need one.
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

export type LogFields = Record<string, unknown>;

export interface Logger {
  debug(msg: string, fields?: LogFields): void;
  info(msg: string, fields?: LogFields): void;
  warn(msg: string, fields?: LogFields): void;
  error(msg: string, fields?: LogFields): void;
  /** Returns a logger that merges `fields` into every line it writes, without
   * mutating this one. For binding context (a runId, a jobId) once at the top
   * of a job rather than repeating it at every call site. */
  child(fields: LogFields): Logger;
}

/** Turns an unknown caught value into log-safe fields. `Error.message`/`.stack`
 * are own accessors, not enumerable own-properties, so spreading an Error
 * directly into a log line silently drops them — this is the fix, not a
 * style preference. */
export function serializeError(err: unknown): LogFields {
  if (err instanceof Error) {
    return { errorName: err.name, errorMessage: err.message, stack: err.stack };
  }
  return { error: err };
}

function write(
  level: LogLevel,
  service: string,
  bound: LogFields,
  msg: string,
  fields: LogFields | undefined,
): void {
  const line = JSON.stringify({
    time: new Date().toISOString(),
    level,
    service,
    msg,
    ...bound,
    ...fields,
  });
  // Split by stream, not just by convention: a host that separates stdout from
  // stderr (most container platforms, `docker logs` with `2>`) can then filter
  // on warn/error without parsing the JSON payload at all.
  if (level === "warn" || level === "error") {
    console.error(line);
  } else {
    console.log(line);
  }
}

/** Creates a logger. `service` identifies the process (e.g. "worker") in
 * every line, so a shared log stream from multiple processes stays
 * distinguishable. */
export function createLogger(service: string, bound: LogFields = {}): Logger {
  return {
    debug: (msg, fields) => write("debug", service, bound, msg, fields),
    info: (msg, fields) => write("info", service, bound, msg, fields),
    warn: (msg, fields) => write("warn", service, bound, msg, fields),
    error: (msg, fields) => write("error", service, bound, msg, fields),
    child: (fields) => createLogger(service, { ...bound, ...fields }),
  };
}
