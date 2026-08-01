import { describe, expect, it } from "vitest";
import {
  admitRun,
  holdsSlot,
  parseMaxActiveRuns,
  throttleReason,
  SLOT_HOLDING_STATUSES,
} from "./concurrency.js";

describe("admitRun", () => {
  it("admits everything when no cap is set", () => {
    expect(admitRun({ cap: null, holders: ["a", "b", "c"], runId: "d" })).toEqual({
      kind: "admit",
    });
    expect(admitRun({ cap: undefined, holders: ["a"], runId: "b" })).toEqual({ kind: "admit" });
  });

  it("admits while there is room under the cap", () => {
    expect(admitRun({ cap: 2, holders: ["a"], runId: "b" })).toEqual({ kind: "admit" });
  });

  it("throttles once the cap is full", () => {
    expect(admitRun({ cap: 2, holders: ["a", "b"], runId: "c" })).toEqual({
      kind: "throttle",
      cap: 2,
      holders: ["a", "b"],
    });
  });

  it("admits a run that already holds a slot", () => {
    // The deadlock this avoids: a cap of 1, a run parked at its approval gate,
    // and the resume job queuing behind the slot the run itself is holding.
    expect(admitRun({ cap: 1, holders: ["a"], runId: "a" })).toEqual({ kind: "admit" });
  });

  it("throttles a run that does not hold a slot even when others of its workflow do", () => {
    expect(admitRun({ cap: 1, holders: ["a"], runId: "b" })).toEqual({
      kind: "throttle",
      cap: 1,
      holders: ["a"],
    });
  });

  it("admits when the cap is somehow already exceeded but the run is a holder", () => {
    // Over-admission is recoverable; refusing to resume a run that is already
    // executing is not.
    expect(admitRun({ cap: 1, holders: ["a", "b"], runId: "b" })).toEqual({ kind: "admit" });
  });
});

describe("holdsSlot", () => {
  it("counts runs that are executing or parked mid-flow", () => {
    expect(holdsSlot("RUNNING")).toBe(true);
    expect(holdsSlot("AWAITING_APPROVAL")).toBe(true);
    expect(holdsSlot("COMPENSATING")).toBe(true);
  });

  it("does not count the waiting pool or terminal runs", () => {
    for (const s of ["QUEUED", "SUCCEEDED", "FAILED", "CANCELED", "COMPENSATED"]) {
      expect(holdsSlot(s)).toBe(false);
    }
  });

  it("does not count an incident, so one broken run cannot wedge a workflow", () => {
    expect(holdsSlot("INCIDENT")).toBe(false);
  });

  it("exposes the holding set for the query that counts them", () => {
    // The worker builds its `status: { in: ... }` filter from this, so the
    // constant and the predicate must not drift apart.
    for (const s of SLOT_HOLDING_STATUSES) expect(holdsSlot(s)).toBe(true);
  });
});

describe("throttleReason", () => {
  it("reads as a sentence at a cap of one", () => {
    expect(throttleReason(1, ["a"])).toBe(
      "Waiting for a free slot: this workflow allows 1 active run and 1 is in flight.",
    );
  });

  it("reads as a sentence above one", () => {
    expect(throttleReason(3, ["a", "b", "c"])).toBe(
      "Waiting for a free slot: this workflow allows 3 active runs and 3 are in flight.",
    );
  });
});

describe("parseMaxActiveRuns", () => {
  it("clears the cap on null, undefined, or empty string", () => {
    expect(parseMaxActiveRuns(null)).toBeNull();
    expect(parseMaxActiveRuns(undefined)).toBeNull();
    expect(parseMaxActiveRuns("")).toBeNull();
  });

  it("accepts a positive integer, as a number or a form string", () => {
    expect(parseMaxActiveRuns(3)).toBe(3);
    expect(parseMaxActiveRuns("3")).toBe(3);
  });

  it("rejects zero rather than reading it as a pause switch", () => {
    expect(() => parseMaxActiveRuns(0)).toThrow(/at least 1/);
    expect(() => parseMaxActiveRuns(-1)).toThrow(/at least 1/);
  });

  it("rejects non-integers and absurd values", () => {
    expect(() => parseMaxActiveRuns(1.5)).toThrow(/integer/);
    expect(() => parseMaxActiveRuns("many")).toThrow(/integer/);
    expect(() => parseMaxActiveRuns(1000)).toThrow(/100 or less/);
  });
});
