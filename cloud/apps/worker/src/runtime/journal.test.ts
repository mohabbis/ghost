import { describe, expect, it } from "vitest";
import { journalFromEvents, journalFromLegacyRunSteps, type JournalEventRow } from "./journal.js";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";

function ev(
  seq: number,
  type: string,
  stepIndex: number | null = null,
  payload: unknown = null,
): JournalEventRow {
  return { seq, type, stepIndex, payload };
}

describe("journalFromEvents", () => {
  it("folds an empty journal", () => {
    const j = journalFromEvents([]);
    expect(j.completed.size).toBe(0);
    expect(j.inFlight.size).toBe(0);
  });

  it("marks a started-then-succeeded step complete, not in flight", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.runStarted),
      ev(2, RUN_EVENT_TYPES.stepStarted, 0),
      ev(3, RUN_EVENT_TYPES.stepSucceeded, 0),
    ]);
    expect([...j.completed]).toEqual([0]);
    expect(j.inFlight.size).toBe(0);
  });

  it("leaves a started-with-no-outcome step in flight", () => {
    // The crash window the state machine refuses to guess about.
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 0),
      ev(2, RUN_EVENT_TYPES.stepSucceeded, 0),
      ev(3, RUN_EVENT_TYPES.stepStarted, 1),
    ]);
    expect([...j.completed]).toEqual([0]);
    expect([...j.inFlight]).toEqual([1]);
  });

  it("records failures", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 0),
      ev(2, RUN_EVENT_TYPES.stepFailed, 0),
    ]);
    expect([...j.failed]).toEqual([0]);
    expect(j.inFlight.size).toBe(0);
  });

  it("treats a skipped step as complete", () => {
    const j = journalFromEvents([ev(1, RUN_EVENT_TYPES.stepSkipped, 2)]);
    expect([...j.completed]).toEqual([2]);
  });

  it("clears a prior failure when a retry later succeeds", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 0),
      ev(2, RUN_EVENT_TYPES.stepFailed, 0),
      ev(3, RUN_EVENT_TYPES.stepStarted, 0),
      ev(4, RUN_EVENT_TYPES.stepSucceeded, 0),
    ]);
    expect([...j.completed]).toEqual([0]);
    expect(j.failed.size).toBe(0);
  });

  it("counts attempts per step", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 0),
      ev(2, RUN_EVENT_TYPES.stepRetried, 0),
      ev(3, RUN_EVENT_TYPES.stepStarted, 0),
    ]);
    expect(j.attempts.get(0)).toBe(2);
  });

  // --- The regression that a verification retry must not cause --------------

  it("keeps a step in flight across an in-execution retry", () => {
    // THE bug this guards. A sensitive step's action runs, its verification
    // fails, and the engine appends `step.retried` to re-run the assertion. If
    // that cleared the in-flight marker, a crash mid-retry would leave the step
    // looking neither completed nor in flight — and since the approval is still
    // on record, the state machine would return `execute` and click Submit a
    // second time.
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 2),
      ev(2, RUN_EVENT_TYPES.stepRetried, 2, { phase: "verify", attempt: 2 }),
    ]);
    expect([...j.inFlight]).toEqual([2]);
    expect(j.completed.size).toBe(0);
  });

  it("clears state only for a human-requested retry", () => {
    // A person looked at the incident and decided — the one case where the
    // engine is entitled to consider the step no longer in flight.
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepStarted, 2),
      ev(2, RUN_EVENT_TYPES.stepFailed, 2),
      ev(3, RUN_EVENT_TYPES.stepRetryRequested, 2, { phase: "incident" }),
    ]);
    expect(j.failed.size).toBe(0);
    expect(j.inFlight.size).toBe(0);
    // ...and it grants a fresh retry budget, or an exhausted step could never
    // be retried from an incident.
    expect(j.attempts.get(2)).toBeUndefined();
  });

  it("collects extract outputs keyed by step id", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepSucceeded, 0, {
        stepId: "total",
        outputs: { amount: "250.00" },
      }),
    ]);
    expect(j.outputs).toEqual({ total: { amount: "250.00" } });
  });

  it("ignores non-string outputs", () => {
    const j = journalFromEvents([
      ev(1, RUN_EVENT_TYPES.stepSucceeded, 0, { stepId: "s", outputs: { n: 5 } }),
    ]);
    expect(j.outputs).toEqual({});
  });
});

describe("journalFromLegacyRunSteps", () => {
  it("derives completion from RunStep rows", () => {
    // Runs that were mid-flight when durable execution shipped have no journal.
    // Without this, they would look like "nothing has executed" and re-run from
    // zero — the exact double-submit the journal prevents.
    const j = journalFromLegacyRunSteps([
      { index: 0, status: "SUCCEEDED" },
      { index: 1, status: "SUCCEEDED" },
      { index: 2, status: "RUNNING" },
      { index: 3, status: "PENDING" },
    ]);
    expect([...j.completed]).toEqual([0, 1]);
    expect([...j.inFlight]).toEqual([2]);
    expect(j.failed.size).toBe(0);
  });

  it("treats FAILED and UNKNOWN as failures", () => {
    const j = journalFromLegacyRunSteps([
      { index: 0, status: "FAILED" },
      { index: 1, status: "UNKNOWN" },
    ]);
    expect([...j.failed].sort()).toEqual([0, 1]);
  });
});
