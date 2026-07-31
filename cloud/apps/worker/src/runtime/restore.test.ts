import { describe, expect, it } from "vitest";
import { planRestore, type ExecutedRecord } from "./restore.js";
import type { WorkflowStep } from "@ghost/core/schema/step";

const PAGE1 = "https://shop.example.com/order";
const PAGE2 = "https://shop.example.com/confirm";

const steps: WorkflowStep[] = [
  { id: "nav", type: "navigate", url: PAGE1 },
  { id: "name", type: "fill", selector: { name: "Full name" }, value: "Ada", sensitive: false },
  { id: "submit", type: "click", selector: { role: "button", name: "Submit order" } }, // sensitive
  { id: "nav2", type: "navigate", url: PAGE2 },
  { id: "amount", type: "fill", selector: { name: "Amount" }, value: "250", sensitive: false },
  { id: "confirm", type: "click", selector: { role: "button", name: "Confirm payment" } }, // sensitive
];

function rec(index: number, url: string | null): ExecutedRecord {
  return { index, url };
}

describe("planRestore", () => {
  it("starts cold when nothing has executed", () => {
    expect(planRestore(steps, [], null)).toEqual({ kind: "cold" });
  });

  it("refuses when there is no captured page URL", () => {
    const plan = planRestore(steps, [rec(0, PAGE1), rec(1, PAGE1)], null);
    expect(plan.kind).toBe("unsafe");
    if (plan.kind === "unsafe") expect(plan.reason).toMatch(/no captured page URL/i);
  });

  it("re-applies restorative steps recorded on the page being restored", () => {
    const plan = planRestore(steps, [rec(0, PAGE1), rec(1, PAGE1)], PAGE1);
    expect(plan).toMatchObject({ kind: "restore", url: PAGE1 });
    if (plan.kind === "restore") {
      // The fill comes back; `navigate` is subsumed by going to the URL.
      expect(plan.reapply).toEqual([1]);
    }
  });

  it("never re-applies a click", () => {
    const plan = planRestore(
      steps,
      [rec(0, PAGE1), rec(1, PAGE1), rec(2, PAGE1)],
      PAGE1,
    );
    expect(plan.kind).toBe("restore");
    if (plan.kind === "restore") expect(plan.reapply).not.toContain(2);
  });

  it("drops a restorative step recorded on a different URL", () => {
    // The heart of it: re-applying a fill from page 1 while sitting on page 2
    // would resolve {name: "Full name"} against whatever page 2 happens to
    // have — typing into the wrong form instead of the intended one.
    const plan = planRestore(
      steps,
      [rec(0, PAGE1), rec(1, PAGE1), rec(2, PAGE2), rec(3, PAGE2), rec(4, PAGE2)],
      PAGE2,
    );
    expect(plan.kind).toBe("restore");
    if (plan.kind === "restore") {
      expect(plan.reapply).toEqual([4]); // the page-2 fill only
      expect(plan.reapply).not.toContain(1); // the page-1 fill is dropped
    }
  });

  it("refuses when a prior at-most-once action happened on another page", () => {
    // Two gates: the approved submit landed on page 1, and we are resuming on
    // page 2. We can neither reproduce that state nor confirm it, and we will
    // not re-run the action to find out.
    const plan = planRestore(
      steps,
      [rec(0, PAGE1), rec(1, PAGE1), rec(2, PAGE1)],
      PAGE2,
    );
    expect(plan.kind).toBe("unsafe");
    if (plan.kind === "unsafe") {
      expect(plan.blockingIndex).toBe(2);
      expect(plan.reason).toMatch(/must not be repeated/i);
    }
  });

  it("allows the common single-gate case to resume", () => {
    // A benign click before the gate must not trip the refusal, or the demo
    // workflow and every single-gate workflow would stop working.
    const benign: WorkflowStep[] = [
      { id: "nav", type: "navigate", url: PAGE1 },
      { id: "next", type: "click", selector: { role: "button", name: "Next" } }, // not sensitive
      { id: "pay", type: "click", selector: { role: "button", name: "Pay now" } },
    ];
    const plan = planRestore(benign, [rec(0, PAGE1), rec(1, PAGE1)], PAGE1);
    expect(plan.kind).toBe("restore");
    if (plan.kind === "restore") expect(plan.reapply).toEqual([]);
  });
});
