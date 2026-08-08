// Loads cloud/.env before Next reads anything. Next only looks for .env in its
// own project directory (apps/web), so the workspace-root file the README tells
// people to create was never picked up — see packages/core/src/env.ts.
import "@ghost/core/env";
import type { NextConfig } from "next";
import { withSentryConfig } from "@sentry/nextjs";

const nextConfig: NextConfig = {
  // @ghost/core ships TS source via subpath exports; Next transpiles it.
  transpilePackages: ["@ghost/core"],
  // Keep server-only native/heavy deps out of the bundle.
  serverExternalPackages: ["@prisma/client", ".prisma/client", "bullmq", "ioredis"],
  eslint: {
    // Lint is a separate CI step; don't fail the build on it.
    ignoreDuringBuilds: true,
  },
  webpack: (config) => {
    // @ghost/core is TypeScript written as ESM, so its *internal* relative
    // imports carry `.js` specifiers that point at `.ts` files on disk
    // (`./db.js` → `db.ts`). tsc, tsx and vitest all understand that; webpack
    // does not without being told. Until now the web app only imported core
    // modules that had no relative imports, so this went unnoticed — the first
    // one that did failed the build with "Can't resolve './db.js'".
    config.resolve.extensionAlias = {
      ...config.resolve.extensionAlias,
      ".js": [".ts", ".tsx", ".js"],
    };
    return config;
  },
};

// org/project fall back to the SENTRY_ORG/SENTRY_PROJECT env vars when unset
// here, and authToken to nothing — matching the rest of this file's
// opt-in-via-env-var pattern (see @ghost/core/sentry for the same contract on
// the worker side). Without SENTRY_AUTH_TOKEN the plugin skips source map
// upload rather than failing the build.
export default withSentryConfig(nextConfig, {
  authToken: process.env.SENTRY_AUTH_TOKEN,
  widenClientFileUpload: true,
  tunnelRoute: "/monitoring",
  silent: !process.env.CI,
});
