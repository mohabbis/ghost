import * as Sentry from "@sentry/node";

/**
 * Thin, opt-in wrapper around `@sentry/node` for `apps/worker`.
 *
 * `SENTRY_DSN` absent means fully disabled: no network calls, no captured
 * data, nothing shipped anywhere. This mirrors every other optional
 * integration in this codebase (`HR_API_KEY`, `S3_BUCKET`) — presence enables
 * a capability and nothing else changes when it is unset. A deployment is not
 * required to have a Sentry account to boot or run correctly.
 *
 * `apps/web` is NOT wired to this module. `@sentry/node`'s auto-instrumentation
 * (`import-in-the-middle`, `@sentry/node-core`) needs `node:child_process` and
 * `require('module')` at module-load time, which Next.js's webpack build
 * cannot bundle even with the package marked `serverExternalPackages` — this
 * was tried and broke `pnpm build` outright (`UnhandledSchemeError` on
 * `node:child_process` reached from `instrumentation.ts`). The worker has no
 * such constraint: `apps/worker/tsup.config.ts` marks `@sentry/node` fully
 * external, so Node resolves it normally from `node_modules` at runtime
 * instead of webpack trying to statically bundle it. Wiring Sentry into
 * `apps/web` needs `@sentry/nextjs` (its own webpack/turbopack plugin exists
 * specifically to handle this), set up via `npx @sentry/wizard@latest -i
 * nextjs` against a real Sentry project — see the deploy checklist.
 */

let initialized = false;

export function initSentry(service: "worker"): void {
  const dsn = process.env.SENTRY_DSN;
  if (!dsn) return;
  if (initialized) return;
  Sentry.init({
    dsn,
    environment: process.env.NODE_ENV,
    // Tagged even though only one service calls this today, so an event is
    // never ambiguous about its source if `apps/web` later reports to the
    // same DSN through its own `@sentry/nextjs` setup.
    initialScope: { tags: { service } },
    // This wrapper turns on error capture, not performance tracing. Tracing
    // is a separate, higher-volume Sentry feature with its own cost profile;
    // an operator who wants it can enable it directly against the DSN this
    // reads, without this wrapper deciding a sample rate for them.
    tracesSampleRate: 0,
  });
  initialized = true;
}

/** Whether `initSentry` actually turned it on (`SENTRY_DSN` was set). Lets
 * call sites skip building context objects for a capture that will be
 * silently dropped anyway. */
export function isSentryEnabled(): boolean {
  return initialized;
}

/** No-ops until `initSentry` has run with a DSN configured. `context` lands
 * as Sentry's `extra` data — arbitrary, non-indexed fields for a human
 * reading the event, not for querying/alerting on. */
export function captureException(err: unknown, context?: Record<string, unknown>): void {
  if (!initialized) return;
  Sentry.captureException(err, context ? { extra: context } : undefined);
}

/** Test-only seam: clears the initialized flag so a test can exercise
 * `initSentry` more than once. Mirrors `resetArtifactStore` in
 * storage/artifacts.ts for the same reason — module-level state needs a way
 * back to its initial value between test cases. */
export function resetSentryForTests(): void {
  initialized = false;
}
