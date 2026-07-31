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
