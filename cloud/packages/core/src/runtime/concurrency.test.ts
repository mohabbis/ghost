import { describe, expect, it } from "vitest";
import {
  admitRun,
  freeSlots,
  releasesSlot,
  parseMaxActiveRuns,
  throttleReason,
  SLOT_RELEASING_STATUSES,
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

describe("releasesSlot", () => {
  it("gives the slot back once the run is finished with the system", () => {
    for (const s of ["SUCCEEDED", "FAILED", "CANCELED", "COMPENSATED"]) {
      expect(releasesSlot(s)).toBe(true);
    }
  });

  it("gives it back on an incident, so one broken run cannot wedge a workflow", () => {
    expect(releasesSlot("INCIDENT")).toBe(true);
  });

  it("keeps it while the run is working or parked at its gate", () => {
    // The deliberate trade-off: an approval holds its slot so a later run
    // cannot overtake one parked mid-flow and interleave against the same
    // system.
    expect(releasesSlot("RUNNING")).toBe(false);
    expect(releasesSlot("AWAITING_APPROVAL")).toBe(false);
    expect(releasesSlot("COMPENSATING")).toBe(false);
  });

  it("keeps it while the run is queued, because ownership is not status", () => {
    // A run passes through QUEUED on its way back from an approval. Reading
    // ownership off the status would drop the slot in that gap and let another
    // run take it, stranding the approved mid-flow run behind it.
    expect(releasesSlot("QUEUED")).toBe(false);
  });

  it("exposes the releasing set for anything that needs to enumerate it", () => {
    for (const s of SLOT_RELEASING_STATUSES) expect(releasesSlot(s)).toBe(true);
  });
});

describe("freeSlots", () => {
  it("reports what is left under the cap", () => {
    expect(freeSlots(3, 1)).toBe(2);
    expect(freeSlots(3, 3)).toBe(0);
  });

  it("never goes negative when the cap was lowered under live runs", () => {
    expect(freeSlots(1, 3)).toBe(0);
  });

  it("is unbounded with no cap", () => {
    expect(freeSlots(null, 100)).toBeGreaterThan(1000);
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
