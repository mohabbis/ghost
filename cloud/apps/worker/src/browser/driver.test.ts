import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { WorkflowStep } from "@ghost/core/schema/step";
import { BrowserSession, restoreBrowserPrefix, runStep } from "./driver.js";

/**
 * Hermetic integration test: drives real Chromium against a local file fixture.
 * No database, no network — exercises selector resolution, actions, screenshots,
 * and verification end to end.
 */

const here = dirname(fileURLToPath(import.meta.url));
const fixtureUrl = "file://" + join(here, "__fixtures__", "form.html");

/** Find the preinstalled full Chromium so launch never downloads or mismatches. */
function discoverChromium(): string | undefined {
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

let session: BrowserSession;

beforeAll(async () => {
  if (!process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE) {
    const exe = discoverChromium();
    if (exe) process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE = exe;
  }
  session = await BrowserSession.launch();
});

afterAll(async () => {
  await session?.close();
});

describe("Playwright driver", () => {
  it("navigates, fills semantically, clicks, and verifies the outcome", async () => {
    const page = session.page;

    await runStep(page, { id: "1", type: "navigate", url: fixtureUrl });

    const fill = await runStep(page, {
      id: "2",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "Ada Lovelace",
      sensitive: false,
    } satisfies WorkflowStep);
    expect(fill.screenshot.length).toBeGreaterThan(0);

    await runStep(page, {
      id: "3",
      type: "click",
      selector: { role: "button", name: "Submit order" },
    } satisfies WorkflowStep);

    const verify = await runStep(page, {
      id: "4",
      type: "verify",
      assertion: { kind: "textPresent", expected: "Order submitted" },
    } satisfies WorkflowStep);
    expect(verify.verification?.passed).toBe(true);
  });

  it("returns a failed verification instead of throwing", async () => {
    const res = await runStep(session.page, {
      id: "x",
      type: "verify",
      assertion: { kind: "textPresent", expected: "this text is not on the page" },
    } satisfies WorkflowStep);
    expect(res.verification?.passed).toBe(false);
  });

  it("restoreBrowserPrefix rebuilds page state so a gated click can resume", async () => {
    // Simulate approval resume: a fresh page has no form; replaying the prefix
    // (navigate + fill) must leave Submit order clickable.
    const fresh = await BrowserSession.launch();
    try {
      const prefix: WorkflowStep[] = [
        { id: "1", type: "navigate", url: fixtureUrl },
        {
          id: "2",
          type: "fill",
          selector: { role: "textbox", name: "Full name" },
          value: "Ada Lovelace",
          sensitive: false,
        },
      ];
      await restoreBrowserPrefix(fresh.page, prefix);
      await runStep(fresh.page, {
        id: "3",
        type: "click",
        selector: { role: "button", name: "Submit order" },
      } satisfies WorkflowStep);
      const verify = await runStep(fresh.page, {
        id: "4",
        type: "verify",
        assertion: { kind: "textPresent", expected: "Order submitted" },
      } satisfies WorkflowStep);
      expect(verify.verification?.passed).toBe(true);
    } finally {
      await fresh.close();
    }
  });
});
