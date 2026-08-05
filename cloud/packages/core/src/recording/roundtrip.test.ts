import { describe, expect, it } from "vitest";
import { compileTrace } from "./compile.js";
import { parseRecordingTrace } from "./trace.js";
import { classifyStep } from "../classifier/sensitive.js";

/**
 * The capture → compile contract, exercised against a trace shaped exactly the
 * way `apps/extension/src/content.js` emits one.
 *
 * This is the seam most likely to rot: the extension is plain JavaScript with
 * no type checking against `@ghost/core`, so nothing but this test notices if
 * the recorder's output drifts from what the compiler expects. If it fails,
 * the extension and Ghost have disagreed about the format — fix one of them,
 * do not relax the test.
 *
 * The scenario is the wedge Ghost is aimed at: log into a portal, find a
 * record, change it, submit.
 */

/** Verbatim in the shape `describeTarget()` produces. */
const target = (over: Record<string, unknown>) => ({
  selectorCandidates: [],
  ...over,
});

const extensionTrace = {
  version: 1,
  sessionId: "rec_1735000000000",
  startUrl: "https://portal.test/login",
  capturedAt: 1735000000000,
  recorder: "ghost-chrome-extension/0.1.0",
  events: [
    { type: "navigate", url: "https://portal.test/login", timestamp: 1 },
    {
      type: "input",
      url: "https://portal.test/login",
      timestamp: 2,
      target: target({
        role: "textbox",
        name: "Username",
        selectorCandidates: ["#username", "input[name=\"user\"]"],
        tagName: "input",
        inputType: "text",
      }),
      value: "ops.team",
      redacted: false,
    },
    {
      // The recorder never read this value — the field was type=password.
      type: "input",
      url: "https://portal.test/login",
      timestamp: 3,
      target: target({
        role: "textbox",
        name: "Password",
        selectorCandidates: ["#password"],
        tagName: "input",
        inputType: "password",
      }),
      redacted: true,
    },
    {
      type: "click",
      url: "https://portal.test/login",
      timestamp: 4,
      target: target({ role: "button", name: "Sign in", selectorCandidates: ["#signin"] }),
    },
    { type: "navigate", url: "https://portal.test/customers/4821", timestamp: 5 },
    {
      type: "select",
      url: "https://portal.test/customers/4821",
      timestamp: 6,
      target: target({ role: "combobox", name: "Status", selectorCandidates: ["#status"] }),
      value: "active",
    },
    {
      type: "click",
      url: "https://portal.test/customers/4821",
      timestamp: 7,
      target: target({ role: "button", name: "Submit changes", selectorCandidates: ["#save"] }),
    },
    { type: "submit", url: "https://portal.test/customers/4821", timestamp: 8 },
  ],
};

describe("extension trace → workflow steps", () => {
  const parsed = parseRecordingTrace(extensionTrace);

  it("is accepted by the trace schema exactly as the extension emits it", () => {
    expect(parsed.ok).toBe(true);
  });

  if (!parsed.ok) throw new Error(`fixture must parse: ${parsed.error}`);
  const { steps, notes } = compileTrace(parsed.trace);

  it("opens the page the recording started on", () => {
    expect(steps[0]).toMatchObject({ type: "navigate", url: "https://portal.test/login" });
  });

  it("carries the password field through as sensitive, with no value", () => {
    const password = steps.find(
      (s) => s.type === "fill" && s.selector.name === "Password",
    ) as { value: string; sensitive: boolean } | undefined;

    expect(password).toBeDefined();
    expect(password!.sensitive).toBe(true);
    expect(password!.value).toMatch(/redacted/i);
    // The whole trace must be free of it — no field anywhere holds a password.
    expect(JSON.stringify(extensionTrace)).not.toMatch(/hunter2|correct-horse/i);
  });

  it("keeps the non-secret value it did capture", () => {
    const username = steps.find((s) => s.type === "fill" && s.selector.name === "Username") as
      | { value: string }
      | undefined;
    expect(username?.value).toBe("ops.team");
  });

  it("resolves elements semantically rather than by CSS", () => {
    const signIn = steps.find(
      (s) => s.type === "click" && s.selector.name === "Sign in",
    ) as { selector: Record<string, unknown> } | undefined;
    expect(signIn?.selector).toMatchObject({ role: "button", name: "Sign in" });
    expect(signIn?.selector.css).toBeUndefined();
  });

  it("stops for approval before submitting the change", () => {
    const submitIndex = steps.findIndex(
      (s) => s.type === "click" && s.selector.name === "Submit changes",
    );
    expect(submitIndex).toBeGreaterThan(0);
    expect(steps[submitIndex - 1]).toMatchObject({ type: "approval" });
  });

  it("gates every step the engine would gate, and no step is left ungated", () => {
    steps.forEach((step, i) => {
      if (step.type === "approval") return;
      if (classifyStep(step).sensitive) {
        expect(steps[i - 1], `step ${i} is sensitive but ungated`).toMatchObject({
          type: "approval",
        });
      }
    });
  });

  it("tells the reviewer the password needs setting before a run", () => {
    expect(notes.join(" ")).toMatch(/set it before running/i);
  });

  it("emits no coordinates anywhere", () => {
    const serialized = JSON.stringify(steps);
    expect(serialized).not.toMatch(/"x":\s*\d/);
    expect(serialized).not.toMatch(/"y":\s*\d/);
    expect(serialized).not.toMatch(/clientX|pageX|screenX/);
  });
});
