import { readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * Locate a preinstalled Chromium so a launch never downloads or hits a revision
 * mismatch.
 *
 * Playwright looks for the exact browser build its own version pins. In images
 * where the browsers are provisioned separately (CI runners, dev containers),
 * that revision often differs from what is on disk and the launch fails with
 * "Executable doesn't exist at .../chromium_headless_shell-<rev>/...". Pointing
 * at the full Chromium that *is* installed avoids the whole problem.
 *
 * Called from `launchOptions` in driver.ts — the single place a browser is
 * launched — so every consumer gets it. It was previously opt-in, and the four
 * test files opted in while the worker's entrypoint did not; the suite passed
 * green in the very environments where a real run could not start.
 */
export function discoverChromium(): string | undefined {
  const base = process.env.PLAYWRIGHT_BROWSERS_PATH;
  if (!base) return undefined;
  try {
    const dir = readdirSync(base).find((d) => /^chromium-\d+$/.test(d));
    if (dir) return join(base, dir, "chrome-linux", "chrome");
  } catch {
    /* fall through to Playwright's own lookup */
  }
  return undefined;
}
