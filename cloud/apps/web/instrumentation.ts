import * as Sentry from "@sentry/nextjs";

export async function register() {
  if (process.env.NEXT_RUNTIME === "nodejs") {
    await import("./sentry.server.config");
  }

  if (process.env.NEXT_RUNTIME === "edge") {
    await import("./sentry.edge.config");
  }
}

// Captures unhandled server-side request errors (server components, route
// handlers, server actions) that would otherwise never reach
// `Sentry.captureException`.
export const onRequestError = Sentry.captureRequestError;
