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
};

export default nextConfig;
