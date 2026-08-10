import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { WorkflowStep } from "@ghost/core/schema/step";
import {
  BrowserSession,
  applyRestorePlan,
  applyRestorativeStep,
  captureSessionState,
  launchOptions,
  runStep,
} from "./driver.js";

/**
 * Hermetic integration test: drives real Chromium against a local HTTP fixture.
 * No database — exercises selector resolution, actions, screenshots, and
 * verification end to end. Served over loopback http (not `file:`) so the
 * navigate URL policy matches production.
 */

const here = dirname(fileURLToPath(import.meta.url));
const fixtureHtml = readFileSync(join(here, "__fixtures__", "form.html"), "utf8");
const OPTS = { timeoutMs: 15_000 };

let session: BrowserSession;
let server: Server;
let fixtureUrl: string;

beforeAll(async () => {
  server = createServer((_req, res) => {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(fixtureHtml);
  });
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve());
  });
  const addr = server.address();
  if (!addr || typeof addr === "string") throw new Error("no listen address");
  fixtureUrl = `http://127.0.0.1:${addr.port}/`;

  // No browser-path setup: `launchOptions` resolves a preinstalled Chromium
  // itself. That used to live here, which is exactly why the worker shipped
  // without it.
  session = await BrowserSession.launch();
});

afterAll(async () => {
  await session?.close();
  await new Promise<void>((resolve, reject) => {
    server.close((err) => (err ? reject(err) : resolve()));
  });
});

async function submitCount(page: BrowserSession["page"]): Promise<number> {
  return page.evaluate(() => Number(localStorage.getItem("ghost:submits") || "0"));
}

describe("Playwright driver", () => {
  it("navigates, fills semantically, clicks, and verifies the outcome", async () => {
    const page = session.page;

    await runStep(page, { id: "1", type: "navigate", url: fixtureUrl }, OPTS);

    const fill = await runStep(
      page,
      {
        id: "2",
        type: "fill",
        selector: { role: "textbox", name: "Full name" },
        value: "Ada Lovelace",
        sensitive: false,
      } satisfies WorkflowStep,
      OPTS,
    );
    expect(fill.screenshot).not.toBeNull();
    expect(fill.screenshot!.length).toBeGreaterThan(0);

    const sensitive = await runStep(
      page,
      {
        id: "2b",
        type: "fill",
        selector: { role: "textbox", name: "Full name" },
        value: "4111111111111111",
        sensitive: true,
      } satisfies WorkflowStep,
      OPTS,
    );
    expect(sensitive.screenshot).toBeNull();

    await runStep(
      page,
      { id: "3", type: "click", selector: { role: "button", name: "Submit order" } },
      OPTS,
    );

    const verify = await runStep(
      page,
      {
        id: "4",
        type: "verify",
        assertion: { kind: "textPresent", expected: "Order submitted" },
      } satisfies WorkflowStep,
      OPTS,
    );
    expect(verify.verification?.passed).toBe(true);
  });

  it("returns a failed verification instead of throwing", async () => {
    const res = await runStep(
      session.page,
      {
        id: "x",
        type: "verify",
        assertion: { kind: "textPresent", expected: "this text is not on the page" },
      } satisfies WorkflowStep,
      OPTS,
    );
    expect(res.verification?.passed).toBe(false);
  });

  it("captures extract output so later steps can reference it", async () => {
    await runStep(session.page, { id: "n", type: "navigate", url: fixtureUrl }, OPTS);
    await runStep(
      session.page,
      { id: "c", type: "click", selector: { role: "button", name: "Submit order" } },
      OPTS,
    );
    const res = await runStep(
      session.page,
      { id: "e", type: "extract", selector: { css: "#result" }, name: "status" },
      OPTS,
    );
    expect(res.outputs).toEqual({ status: "Order submitted" });
  });

  it("threads a per-step timeout into the action", async () => {
    const started = Date.now();
    await expect(
      runStep(
        session.page,
        { id: "w", type: "waitFor", selector: { css: "#never-appears" } },
        { timeoutMs: 500 },
      ),
    ).rejects.toThrow();
    // Proves the timeout is actually plumbed through rather than merely
    // accepted by the schema — Playwright's own default is 30s.
    expect(Date.now() - started).toBeLessThan(5_000);
  });

  // --- Restoration must never repeat an action ------------------------------

  it("restores page state from a plan without replaying any click", async () => {
    const steps: WorkflowStep[] = [
      { id: "1", type: "navigate", url: fixtureUrl },
      {
        id: "2",
        type: "fill",
        selector: { role: "textbox", name: "Full name" },
        value: "Ada Lovelace",
        sensitive: false,
      },
      { id: "3", type: "click", selector: { role: "button", name: "Submit order" } },
    ];

    const fresh = await BrowserSession.launch();
    try {
      // Establish a submit count of 1, as a pre-gate click would have done.
      await runStep(fresh.page, steps[0]!, OPTS);
      await runStep(fresh.page, steps[2]!, OPTS);
      expect(await submitCount(fresh.page)).toBe(1);

      // Restoring re-applies only the fill; index 2 is deliberately absent
      // from `reapply` because the planner classifies a click as mutating.
      await applyRestorePlan(fresh.page, steps, { kind: "restore", url: fixtureUrl, reapply: [1] }, OPTS);

      // The click was NOT repeated.
      expect(await submitCount(fresh.page)).toBe(1);
      // ...and the form state a gated step depends on is back.
      const value = await fresh.page.getByRole("textbox", { name: "Full name" }).inputValue();
      expect(value).toBe("Ada Lovelace");
    } finally {
      await fresh.close();
    }
  });

  it("refuses to re-apply a mutating step even if one reaches it", async () => {
    // Defence in depth: the planner filters clicks out, and the restorative
    // applier has no code path that performs one. Both must hold.
    await expect(
      applyRestorativeStep(
        session.page,
        { id: "3", type: "click", selector: { role: "button", name: "Submit order" } },
        OPTS,
      ),
    ).rejects.toThrow(/refusing to replay/i);
  });

  it("round-trips session state into a fresh browser", async () => {
    // Served over HTTP, not file://. `storageState` only captures localStorage
    // for real origins — a file:// page has origin "null" and is skipped — so a
    // file-based fixture would silently prove nothing here.
    const html = readFileSync(join(here, "__fixtures__", "form.html"), "utf8");
    const server = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(html);
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()));
    const addr = server.address();
    if (!addr || typeof addr === "string") throw new Error("no listen address");
    const httpUrl = `http://127.0.0.1:${addr.port}/`;

    const first = await BrowserSession.launch();
    let captured: { state: Buffer; url: string };
    try {
      await runStep(first.page, { id: "1", type: "navigate", url: httpUrl }, OPTS);
      await runStep(
        first.page,
        { id: "3", type: "click", selector: { role: "button", name: "Submit order" } },
        OPTS,
      );
      expect(await submitCount(first.page)).toBe(1);
      captured = await captureSessionState(first.context, first.page);
      expect(captured.url).toBe(httpUrl);
    } finally {
      await first.close();
    }

    // A brand-new browser, seeded only with the captured state, sees the
    // localStorage the first one wrote — which is how a resumed run recovers
    // session/auth without replaying the actions that established it.
    const second = await BrowserSession.launch(captured.state);
    try {
      await second.page.goto(captured.url);
      expect(await submitCount(second.page)).toBe(1);
    } finally {
      await second.close();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
  });
});
