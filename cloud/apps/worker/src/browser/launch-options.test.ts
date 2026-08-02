import { describe, expect, it } from "vitest";
import { mkdirSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { launchOptions } from "./driver.js";

/**
 * Pure unit test for browser-executable resolution. Deliberately its own file:
 * the rest of driver.test.ts launches a real browser in `beforeAll`, and a
 * launch failure there would mask exactly the assertions below.
 */
describe("launchOptions", () => {
  // The bug this guards: resolution used to be opt-in, via a
  // `useDiscoveredChromium()` call the test files made and the worker's
  // entrypoint did not. Both halves matter — a suite that only asserts "a
  // browser launched" passes either way, because the tests were the ones
  // opting in.
  const KEY = "PLAYWRIGHT_CHROMIUM_EXECUTABLE";
  const PATH_KEY = "PLAYWRIGHT_BROWSERS_PATH";

  function withEnv<T>(env: Record<string, string | undefined>, fn: () => T): T {
    const saved = Object.fromEntries(Object.keys(env).map((k) => [k, process.env[k]]));
    for (const [k, v] of Object.entries(env)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    try {
      return fn();
    } finally {
      for (const [k, v] of Object.entries(saved)) {
        if (v === undefined) delete process.env[k];
        else process.env[k] = v;
      }
    }
  }

  it("resolves a preinstalled Chromium with no env var set", () => {
    const root = mkdtempSync(join(tmpdir(), "ghost-browsers-"));
    mkdirSync(join(root, "chromium-1194", "chrome-linux"), { recursive: true });
    // A revision Playwright does not pin, alongside the headless-shell build it
    // would reach for — the exact shape that produced "Executable doesn't exist
    // at .../chromium_headless_shell-<rev>/..." on a fully green test run.
    mkdirSync(join(root, "chromium_headless_shell-1194"), { recursive: true });

    const opts = withEnv({ [KEY]: undefined, [PATH_KEY]: root }, launchOptions);

    expect(opts.executablePath).toBe(join(root, "chromium-1194", "chrome-linux", "chrome"));
  });

  it("lets an explicit executable path win over discovery", () => {
    const opts = withEnv({ [KEY]: "/custom/chrome", [PATH_KEY]: "/opt/pw-browsers" }, launchOptions);
    expect(opts.executablePath).toBe("/custom/chrome");
  });

  it("falls back to Playwright's own lookup when nothing is preinstalled", () => {
    const empty = mkdtempSync(join(tmpdir(), "ghost-browsers-empty-"));
    const opts = withEnv({ [KEY]: undefined, [PATH_KEY]: empty }, launchOptions);
    expect(opts.executablePath).toBeUndefined();
  });
});
