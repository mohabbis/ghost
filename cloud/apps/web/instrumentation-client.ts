import * as Sentry from "@sentry/nextjs";

/**
 * Browser runtime. `NEXT_PUBLIC_SENTRY_DSN` absent means fully disabled — no
 * network calls, nothing captured — mirroring the worker's `SENTRY_DSN`
 * opt-in (`@ghost/core/sentry`). A deployment is not required to have a
 * Sentry account to boot or run correctly.
 */
Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,
  environment: process.env.NODE_ENV,

  // 100% in dev, 10% in production.
  tracesSampleRate: process.env.NODE_ENV === "development" ? 1.0 : 0.1,
});

// Hooks into App Router navigation transitions so route changes show up as
// tracing spans.
export const onRouterTransitionStart = Sentry.captureRouterTransitionStart;
