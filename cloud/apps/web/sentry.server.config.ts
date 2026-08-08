import * as Sentry from "@sentry/nextjs";

/**
 * Node.js server runtime. `SENTRY_DSN` absent means fully disabled — same
 * opt-in contract as `instrumentation-client.ts` and the worker's
 * `@ghost/core/sentry`.
 */
Sentry.init({
  dsn: process.env.SENTRY_DSN,
  environment: process.env.NODE_ENV,

  // 100% in dev, 10% in production.
  tracesSampleRate: process.env.NODE_ENV === "development" ? 1.0 : 0.1,
});
