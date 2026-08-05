import { describe, expect, it } from "vitest";
import { compileTrace } from "./compile.js";
import { parseRecordingTrace, type RecordingTrace } from "./trace.js";
import { EDITABLE_STEP_TYPES, workflowSteps } from "../schema/step.js";
import { classifyStep } from "../classifier/sensitive.js";

/**
 * The deterministic path from a captured trace to typed steps.
 *
 * This is what makes recording work with no compiler configured — the case
 * production runs in. If these pass, a customer can record a workflow in a
 * deployment that has no AI dependency of any kind.
 */

function trace(events: RecordingTrace["events"], startUrl = "https://shop.test/order"): RecordingTrace {
  return { version: 1, sessionId: "rec_test", startUrl, events };
}

const target = (name: string, role = "button") => ({
  role,
  name,
  selectorCandidates: [`#${name.replace(/\s+/g, "-").toLowerCase()}`],
});

describe("compileTrace", () => {
  it("opens the starting page before anything else", () => {
    const { steps } = compileTrace(trace([]));
    expect(steps[0]).toMatchObject({ type: "navigate", url: "https://shop.test/order" });
  });

  it("produces steps that satisfy the published workflow schema", () => {
    const { steps } = compileTrace(
      trace([
        {
          type: "input",
          url: "https://shop.test/order",
          timestamp: 1,
          target: { ...target("Full name", "textbox") },
          value: "Ada Lovelace",
          redacted: false,
        },
        { type: "click", url: "https://shop.test/order", timestamp: 2, target: target("Submit order") },
      ]),
    );

    // The real gate: these steps go through the same validator the publish
    // route uses, so a compiled workflow cannot be unpublishable.
    expect(workflowSteps.safeParse(steps).success).toBe(true);

    // And every emitted type must have an executor. `apiCall`/`sendEmail`
    // parse and gate but do nothing, so a compiler that emitted one would
    // produce a workflow that reports SUCCEEDED having skipped the step —
    // exactly the failure `EDITABLE_STEP_TYPES` exists to prevent.
    const editable = new Set<string>(EDITABLE_STEP_TYPES);
    for (const step of steps) expect(editable.has(step.type)).toBe(true);
  });

  it("prefers role and name over the CSS fallback", () => {
    const { steps } = compileTrace(
      trace([{ type: "click", url: "https://shop.test/x", timestamp: 1, target: target("Submit order") }]),
    );
    const click = steps.find((s) => s.type === "click");
    expect(click).toMatchObject({ selector: { role: "button", name: "Submit order" } });
    expect(click).not.toHaveProperty("selector.css");
  });

  it("falls back to CSS only when nothing semantic was captured", () => {
    const { steps } = compileTrace(
      trace([
        {
          type: "click",
          url: "https://shop.test/x",
          timestamp: 1,
          target: { selectorCandidates: ["#mystery-button", ".btn"] },
        },
      ]),
    );
    expect(steps.find((s) => s.type === "click")).toMatchObject({
      selector: { css: "#mystery-button" },
    });
  });

  it("collapses a run of keystrokes on one field into a single fill", () => {
    const field = { ...target("Full name", "textbox") };
    const { steps } = compileTrace(
      trace([
        { type: "input", url: "https://shop.test/x", timestamp: 1, target: field, value: "A", redacted: false },
        { type: "input", url: "https://shop.test/x", timestamp: 2, target: field, value: "Ad", redacted: false },
        { type: "input", url: "https://shop.test/x", timestamp: 3, target: field, value: "Ada", redacted: false },
      ]),
    );
    const fills = steps.filter((s) => s.type === "fill");
    expect(fills).toHaveLength(1);
    expect(fills[0]).toMatchObject({ value: "Ada" });
  });

  it("never carries a redacted value, and marks the step sensitive", () => {
    const { steps, notes } = compileTrace(
      trace([
        {
          type: "input",
          url: "https://bank.test/login",
          timestamp: 1,
          target: { ...target("Password", "textbox"), inputType: "password" },
          redacted: true,
        },
      ]),
    );
    const fill = steps.find((s) => s.type === "fill") as { value: string; sensitive: boolean };
    expect(fill.sensitive).toBe(true);
    expect(fill.value).not.toContain("hunter2");
    expect(fill.value).toMatch(/redacted/i);
    expect(notes.join(" ")).toMatch(/never captured/i);
  });

  it("puts an approval in front of an irreversible click", () => {
    const { steps } = compileTrace(
      trace([{ type: "click", url: "https://shop.test/x", timestamp: 1, target: target("Submit order") }]),
    );
    const submitIndex = steps.findIndex((s) => s.type === "click");
    expect(submitIndex).toBeGreaterThan(0);
    expect(steps[submitIndex - 1]).toMatchObject({ type: "approval" });
  });

  it("agrees with the engine: every gated step is preceded by an approval", () => {
    const { steps } = compileTrace(
      trace([
        {
          type: "input",
          url: "https://shop.test/x",
          timestamp: 1,
          target: target("Full name", "textbox"),
          value: "Ada",
          redacted: false,
        },
        { type: "click", url: "https://shop.test/x", timestamp: 2, target: target("Delete account") },
        { type: "click", url: "https://shop.test/x", timestamp: 3, target: target("Submit order") },
      ]),
    );

    steps.forEach((step, i) => {
      if (step.type === "approval") return;
      if (classifyStep(step).sensitive) {
        expect(steps[i - 1], `step ${i} (${step.type}) is gated but has no approval before it`)
          .toMatchObject({ type: "approval" });
      }
    });
  });

  it("does not stack two approvals in a row", () => {
    const { steps } = compileTrace(
      trace([
        { type: "click", url: "https://shop.test/x", timestamp: 1, target: target("Submit order") },
        { type: "click", url: "https://shop.test/x", timestamp: 2, target: target("Confirm") },
      ]),
    );
    for (let i = 1; i < steps.length; i++) {
      expect(steps[i]!.type === "approval" && steps[i - 1]!.type === "approval").toBe(false);
    }
  });

  it("says so when nothing looked irreversible, rather than staying quiet", () => {
    const { notes } = compileTrace(
      trace([{ type: "click", url: "https://shop.test/x", timestamp: 1, target: target("Next") }]),
    );
    expect(notes.join(" ")).toMatch(/no approval gate was added/i);
  });

  it("reports an unidentifiable click instead of guessing at coordinates", () => {
    const { steps, notes } = compileTrace(
      trace([{ type: "click", url: "https://shop.test/x", timestamp: 1, target: { selectorCandidates: [] } }]),
    );
    expect(steps.filter((s) => s.type === "click")).toHaveLength(0);
    expect(notes.join(" ")).toMatch(/cannot be replayed/i);
  });

  it("ignores a navigation that did not change the page", () => {
    const { steps } = compileTrace(
      trace([
        { type: "navigate", url: "https://shop.test/order", timestamp: 1 },
        { type: "navigate", url: "https://shop.test/confirm", timestamp: 2 },
      ]),
    );
    const urls = steps.filter((s) => s.type === "navigate").map((s) => (s as { url: string }).url);
    expect(urls).toEqual(["https://shop.test/order", "https://shop.test/confirm"]);
  });
});

describe("parseRecordingTrace", () => {
  it("accepts a well-formed trace", () => {
    const result = parseRecordingTrace({ version: 1, sessionId: "rec_1", events: [] });
    expect(result.ok).toBe(true);
  });

  it("rejects a HAR or anything else that is not this format", () => {
    const result = parseRecordingTrace({ log: { version: "1.2", entries: [] } });
    expect(result.ok).toBe(false);
  });

  it("names what was wrong rather than failing opaquely", () => {
    const result = parseRecordingTrace({ version: 1, sessionId: "rec_1", events: [{ type: "click" }] });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toMatch(/events/);
  });
});
