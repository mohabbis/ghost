import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright";
import { CAPTURE_BINDING, recorderInitScript } from "@ghost/core/recording/recorder";
import { parseRecordingTrace, type TraceEvent } from "@ghost/core/recording/trace";
import { compileTrace } from "@ghost/core/recording/compile";
import { launchOptions } from "../browser/driver.js";

/**
 * The recorder, injected into a real page exactly the way a capture session
 * injects it, driven by real input.
 *
 * This is the only place the two rules that matter can actually be checked.
 * "A secret never enters the trace" and "no coordinates, ever" are properties
 * of what the recorder *produces*, and reading the source cannot tell you
 * whether Chromium fires `change` on a password field with the value attached
 * — it does, which is why the check has to be against a browser.
 */

const here = dirname(fileURLToPath(import.meta.url));
const fixtureUrl = "file://" + join(here, "__fixtures__", "signin.html");

let browser: Browser;
let context: BrowserContext;
let page: Page;
let events: unknown[];

beforeAll(async () => {
  browser = await chromium.launch(launchOptions());
  context = await browser.newContext();
  events = [];
  // The same two calls, in the same order, that `CaptureSession.open` makes.
  await context.exposeBinding(CAPTURE_BINDING, (_source, event: unknown) => {
    events.push(event);
  });
  await context.addInitScript({ content: recorderInitScript(CAPTURE_BINDING) });
  page = await context.newPage();
  await page.goto(fixtureUrl);

  await page.fill("#email", "ops@example.com");
  await page.fill("#password", "hunter2-not-in-the-trace");
  await page.fill("#otp", "123456");
  await page.selectOption("#plan", "pro");
  await page.click("#submit");
  // The recorder queues and flushes; give the binding a beat to drain.
  await page.waitForTimeout(250);
});

afterAll(async () => {
  await context?.close().catch(() => undefined);
  await browser?.close().catch(() => undefined);
});

/** The trace as a capture session would assemble it before ingesting. */
function trace() {
  const parsed = parseRecordingTrace({
    version: 1,
    sessionId: "test",
    startUrl: fixtureUrl,
    recorder: "ghost-cloud-browser",
    events,
  });
  if (!parsed.ok) throw new Error(`recorder produced an invalid trace: ${parsed.error}`);
  return parsed.trace;
}

function inputFor(name: string): Extract<TraceEvent, { type: "input" }> {
  const found = trace().events.find(
    (e): e is Extract<TraceEvent, { type: "input" }> =>
      e.type === "input" && e.target.selectorCandidates.some((s) => s.includes(name)),
  );
  if (!found) throw new Error(`no input event for ${name}`);
  return found;
}

describe("the injected recorder", () => {
  it("emits a trace that validates against the capture contract", () => {
    expect(trace().events.length).toBeGreaterThan(0);
  });

  it("never records the value of a password field", () => {
    const pw = inputFor("password");
    expect(pw.redacted).toBe(true);
    expect(pw.value).toBeUndefined();
    // Belt and braces: the secret must not appear anywhere in the trace, not
    // merely be absent from the field it was typed into.
    expect(JSON.stringify(trace())).not.toContain("hunter2");
  });

  it("redacts a one-time-code field that is not type=password", () => {
    // `type="text"` — only the name gives it away, which is the case a
    // type-only check would wave through.
    const otp = inputFor("otp");
    expect(otp.redacted).toBe(true);
    expect(otp.value).toBeUndefined();
    expect(JSON.stringify(trace())).not.toContain("123456");
  });

  it("does record ordinary field values", () => {
    // The counterweight: over-redaction would make recording useless, so an
    // email address must still be captured.
    const email = inputFor("email");
    expect(email.redacted).toBe(false);
    expect(email.value).toBe("ops@example.com");
  });

  it("reads accessible role and name off the element, not its position", () => {
    const click = trace().events.find((e) => e.type === "click");
    expect(click).toBeDefined();
    if (!click || click.type !== "click") return;
    expect(click.target.role).toBe("button");
    expect(click.target.name).toBe("Sign in");
    expect(click.target.testId).toBe("signin-submit");
  });

  it("puts no coordinates in the trace at all", () => {
    // The property, stated as a property: nothing in the serialised trace may
    // look like a screen position. A step resolved by coordinate is not a step
    // that survives the page moving.
    const json = JSON.stringify(trace());
    for (const key of ['"x"', '"y"', '"clientX"', '"clientY"', '"pageX"', '"pageY"']) {
      expect(json).not.toContain(key);
    }
  });

  it("records a select as its value", () => {
    const select = trace().events.find((e) => e.type === "select");
    expect(select).toBeDefined();
    if (!select || select.type !== "select") return;
    expect(select.value).toBe("pro");
  });

  it("compiles into steps deterministically, with the secret still absent", () => {
    // The end-to-end claim: a cloud capture reaches the review page as typed
    // steps with no model in the path.
    const { steps } = compileTrace(trace());
    expect(steps.length).toBeGreaterThan(1);
    expect(JSON.stringify(steps)).not.toContain("hunter2");
    expect(JSON.stringify(steps)).not.toContain("123456");
  });
});
