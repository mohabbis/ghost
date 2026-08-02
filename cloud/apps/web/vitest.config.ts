import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

export default defineConfig({
  test: {
    // Authoritative so vitest never climbs to the repo root's (Tauri) vite config.
    root: __dirname,
    include: ["src/**/*.test.ts"],
    environment: "node",
    testTimeout: 30_000,
  },
  resolve: {
    // Mirror the `@/*` path alias from tsconfig; Next resolves it at build time,
    // vitest needs telling.
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
});
