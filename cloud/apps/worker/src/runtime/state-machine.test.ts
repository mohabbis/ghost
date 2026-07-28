import { describe, expect, it } from "vitest";
import { planNextAction, isNoopStep } from "./state-machine.js";
import type { WorkflowStep } from "@ghost/core/schema/step";

const steps: WorkflowStep[] = [
  { id: "1", type: "navigate", url: "https://example.com" },
  { id: "2", type: "fill", selector: { name: "Full name" }, value: "Ada", sensitive: false },
  { id: "3", type: "click", selector: { role: "button", name: "Submit order" } }, // sensitive
  { id: "4", type: "verify", assertion: { kind: "url", expected: "https://example.com/done" } },
];

const none: ReadonlySet<number> = new Set();

describe("planNextAction", () => {
  it("executes a benign step at the cursor", () => {
    expect(planNextAction(steps, 0, none)).toMatchObject({ kind: "execute", index: 0 });
    expect(planNextAction(steps, 1, none)).toMatchObject({ kind: "execute", index: 1 });
  });

  it("gates at an unapproved sensitive step", () => {
    const action = planNextAction(steps, 2, none);
    expect(action.kind).toBe("gate");
    if (action.kind === "gate") {
      expect(action.index).toBe(2);
      expect(action.reason).toMatch(/submit/i);
    }
  });

  it("executes a sensitive step once its index is approved", () => {
    expect(planNextAction(steps, 2, new Set([2]))).toMatchObject({ kind: "execute", index: 2 });
  });

  it("reports done past the end", () => {
    expect(planNextAction(steps, steps.length, none)).toEqual({ kind: "done" });
  });

  it("walks a full run: benign, benign, gate → approve → execute, verify, done", () => {
    // 0,1 execute
    expect(planNextAction(steps, 0, none).kind).toBe("execute");
    expect(planNextAction(steps, 1, none).kind).toBe("execute");
    // 2 gates without approval
    expect(planNextAction(steps, 2, none).kind).toBe("gate");
    // approve index 2 → execute
    const approved = new Set([2]);
    expect(planNextAction(steps, 2, approved).kind).toBe("execute");
    // 3 verify executes
    expect(planNextAction(steps, 3, approved).kind).toBe("execute");
    // done
    expect(planNextAction(steps, 4, approved).kind).toBe("done");
  });

  it("treats an explicit approval step as a gate, then a no-op once approved", () => {
    const withApproval: WorkflowStep[] = [
      { id: "a", type: "approval", reason: "double-check totals" },
    ];
    expect(planNextAction(withApproval, 0, none).kind).toBe("gate");
    const act = planNextAction(withApproval, 0, new Set([0]));
    expect(act.kind).toBe("execute");
    if (act.kind === "execute") expect(isNoopStep(act.step)).toBe(true);
  });
});
