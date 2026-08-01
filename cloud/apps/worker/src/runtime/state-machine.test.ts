import { describe, expect, it } from "vitest";
import { planNextAction, isNoopStep, type ApprovalState } from "./state-machine.js";
import { emptyJournal, type RunJournal } from "@ghost/core/journal";
import type { WorkflowStep } from "@ghost/core/schema/step";

const steps: WorkflowStep[] = [
  { id: "1", type: "navigate", url: "https://example.com" },
  { id: "2", type: "fill", selector: { name: "Full name" }, value: "Ada", sensitive: false },
  { id: "3", type: "click", selector: { role: "button", name: "Submit order" } }, // sensitive
  { id: "4", type: "verify", assertion: { kind: "url", expected: "https://example.com/done" } },
];

const NO_APPROVALS: ApprovalState = { approved: new Set(), rejected: new Set() };

function journal(over: Partial<RunJournal> = {}): RunJournal {
  return { ...emptyJournal(), ...over };
}

function approvals(over: Partial<ApprovalState> = {}): ApprovalState {
  return { ...NO_APPROVALS, ...over };
}

describe("planNextAction", () => {
  it("executes the first benign step of a fresh run", () => {
    expect(planNextAction(steps, journal(), NO_APPROVALS)).toMatchObject({
      kind: "execute",
      index: 0,
    });
  });

  it("skips completed steps and executes the next one", () => {
    expect(
      planNextAction(steps, journal({ completed: new Set([0]) }), NO_APPROVALS),
    ).toMatchObject({ kind: "execute", index: 1 });
  });

  it("gates at an unapproved sensitive step", () => {
    const action = planNextAction(steps, journal({ completed: new Set([0, 1]) }), NO_APPROVALS);
    expect(action.kind).toBe("gate");
    if (action.kind === "gate") {
      expect(action.index).toBe(2);
      expect(action.reason).toMatch(/submit/i);
    }
  });

  it("executes a sensitive step once its index is approved", () => {
    expect(
      planNextAction(
        steps,
        journal({ completed: new Set([0, 1]) }),
        approvals({ approved: new Set([2]) }),
      ),
    ).toMatchObject({ kind: "execute", index: 2 });
  });

  it("reports done when every step is complete", () => {
    expect(
      planNextAction(steps, journal({ completed: new Set([0, 1, 2, 3]) }), NO_APPROVALS),
    ).toEqual({ kind: "done" });
  });

  it("walks a full run: benign, benign, gate → approve → execute, verify, done", () => {
    const done = new Set<number>();
    expect(planNextAction(steps, journal({ completed: done }), NO_APPROVALS).kind).toBe("execute");
    done.add(0);
    expect(planNextAction(steps, journal({ completed: done }), NO_APPROVALS).kind).toBe("execute");
    done.add(1);
    expect(planNextAction(steps, journal({ completed: done }), NO_APPROVALS).kind).toBe("gate");

    const approved = approvals({ approved: new Set([2]) });
    expect(planNextAction(steps, journal({ completed: done }), approved).kind).toBe("execute");
    done.add(2);
    expect(planNextAction(steps, journal({ completed: done }), approved).kind).toBe("execute");
    done.add(3);
    expect(planNextAction(steps, journal({ completed: done }), approved)).toEqual({ kind: "done" });
  });

  it("treats an explicit approval step as a gate, then a no-op once approved", () => {
    const withApproval: WorkflowStep[] = [
      { id: "a", type: "approval", reason: "double-check totals" },
    ];
    expect(planNextAction(withApproval, journal(), NO_APPROVALS).kind).toBe("gate");
    const act = planNextAction(withApproval, journal(), approvals({ approved: new Set([0]) }));
    expect(act.kind).toBe("execute");
    if (act.kind === "execute") expect(isNoopStep(act.step)).toBe(true);
  });

  it("fails when an approval was rejected", () => {
    const action = planNextAction(
      steps,
      journal({ completed: new Set([0, 1]) }),
      approvals({ rejected: new Set([2]) }),
    );
    // A human decided; the incident flow must not offer to retry past it.
    expect(action).toMatchObject({ kind: "failed", index: 2, recoverable: false });
  });

  it("fails at a step the journal records as previously failed", () => {
    const action = planNextAction(
      steps,
      journal({ completed: new Set([0]), failed: new Set([1]) }),
      NO_APPROVALS,
    );
    // Recoverable: a human can retry or skip it through the incident flow.
    expect(action).toMatchObject({ kind: "failed", index: 1, recoverable: true });
  });

  // --- Durable execution: the regression that motivated the journal ---------

  it("never re-executes an approved sensitive step that already completed", () => {
    // The crash-resume scenario: the payment click at index 2 succeeded, the
    // worker died, and the job was redelivered. Previously the cursor was a
    // local variable persisted only at gates, so the run restarted at 0 and —
    // because the approval is still on record — re-planned index 2 as
    // `execute` rather than `gate`, submitting the order a second time.
    const action = planNextAction(
      steps,
      journal({ completed: new Set([0, 1, 2]) }),
      approvals({ approved: new Set([2]) }),
    );
    expect(action).toMatchObject({ kind: "execute", index: 3 });
  });

  it("stops at an interrupted at-most-once step rather than guessing", () => {
    // `step.started` recorded, no terminal outcome: the click may or may not
    // have reached the server. Re-running risks a double payment, skipping
    // risks a silent no-pay. Neither is the engine's call.
    const action = planNextAction(
      steps,
      journal({ completed: new Set([0, 1]), inFlight: new Set([2]) }),
      approvals({ approved: new Set([2]) }),
    );
    expect(action.kind).toBe("indeterminate");
    if (action.kind === "indeterminate") expect(action.index).toBe(2);
  });

  it("re-attempts an interrupted benign step", () => {
    // A `fill` that was interrupted is safe to redo, and the retry is visible
    // in the journal and the run timeline.
    const action = planNextAction(
      steps,
      journal({ completed: new Set([0]), inFlight: new Set([1]) }),
      NO_APPROVALS,
    );
    expect(action).toMatchObject({ kind: "execute", index: 1 });
  });

  // --- Expression resolution happens before the gate decision ---------------

  it("classifies the resolved step, so a human approves what will actually run", () => {
    const templated: WorkflowStep[] = [
      { id: "amt", type: "fill", selector: { name: "Amount" }, value: "{{ steps.total.amount }}", sensitive: false },
    ];
    const action = planNextAction(templated, journal(), NO_APPROVALS, (step) =>
      step.type === "fill" ? { ...step, value: "250.00" } : step,
    );
    expect(action).toMatchObject({ kind: "execute", index: 0 });
    if (action.kind === "execute" && action.step.type === "fill") {
      expect(action.step.value).toBe("250.00");
    }
  });

  it("fails the run when a reference cannot be resolved", () => {
    const templated: WorkflowStep[] = [
      { id: "amt", type: "fill", selector: { name: "Amount" }, value: "{{ steps.nope.missing }}", sensitive: false },
    ];
    const action = planNextAction(templated, journal(), NO_APPROVALS, () => {
      throw new Error('no value named "missing" was captured by step "nope"');
    });
    expect(action.kind).toBe("failed");
    if (action.kind === "failed") {
      expect(action.reason).toMatch(/missing/);
      // An authoring error: retrying cannot change the outcome.
      expect(action.recoverable).toBe(false);
    }
  });
});
