import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Keep this config authoritative so vitest never climbs to the repo root's
    // (Tauri) vite.config.js.
    root: __dirname,
    include: ["src/**/*.test.ts"],
    environment: "node",
    // Playwright launches a real browser; give integration tests room.
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
