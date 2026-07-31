import type { NextConfig } from "next";

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

export default nextConfig;
