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
    alias: {
      // Mirror the `@/*` path alias from tsconfig; Next resolves it at build
      // time, vitest needs telling.
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      // `server-only` is a Next build-time guard with no runtime module for
      // vitest to resolve, so importing a module that declares it fails to
      // load rather than failing an assertion. Point it at an empty stub: the
      // guarantee is still enforced where it matters (a client component
      // importing such a module fails the Next build), and server modules
      // become directly testable instead of only reachable through a mock.
      "server-only": fileURLToPath(new URL("./src/test/server-only-stub.ts", import.meta.url)),
    },
  },
});
