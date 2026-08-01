/**
 * Per-workflow concurrency control — Ghost's take on Airflow's `max_active_runs`.
 *
 * Airflow caps how many runs of a DAG may be active at once; extra runs stay
 * queued until a slot frees rather than being dropped. Ghost has had no
 * concurrency control at all, which matters more here than it does in a data
 * pipeline: a workflow that fires on every inbound email can put ten browser
 * sessions into a customer's ERP at the same time, and the systems Ghost drives
 * are exactly the ones that do not tolerate that.
 *
 * The cap is per **workflow**, not per version — bumping a workflow to v4 must
 * not double the load it is allowed to put on the system it operates.
 *
 * **A slot is taken and given back, not inferred.** `Run.slotHeldAt` records
 * ownership directly. Deriving it from `Run.status` looks simpler and is wrong:
 * status moves for reasons that have nothing to do with whether the run is
 * still touching the customer's system. A cancel marks a run CANCELED while the
 * worker is deliberately letting the current browser action finish, and an
 * approved run passes through QUEUED on its way back to RUNNING. Each of those
 * windows would let a second run be admitted alongside one that is still
 * executing — the one thing the cap exists to prevent.
 *
 * Pure: no Prisma, no Redis. The caller supplies the current holders and gets
 * back a decision it must then commit under a lock.
 */

/**
 * Statuses at which a run has stopped occupying the customer's system for good,
 * so its slot should be handed on.
 *
 * This is a *release* rule, not a hold rule, and the difference is the point. A
 * run holds its slot from admission until one of these is reached, whatever the
 * status does in between.
 *
 * `INCIDENT` is here deliberately. A run that failed and is waiting on a human
 * is not touching anything, and one broken run must not wedge its workflow
 * forever; it re-enters admission when a human retries it, so the cap still
 * governs what is actually executing.
 *
 * `AWAITING_APPROVAL` is deliberately *not* here. An approval is part of a
 * run's normal flow and is expected to resolve in minutes, and letting a later
 * run overtake one parked at its gate would interleave two runs against the
 * same system. The cost is real: a cap of 1 plus an approval left sitting for a
 * day stalls every later run of that workflow. That is visible in the timeline
 * (`run.throttled` names the holder) and is the safer of the two failure modes.
 */
export const SLOT_RELEASING_STATUSES: readonly string[] = [
  "SUCCEEDED",
  "FAILED",
  "CANCELED",
  "COMPENSATED",
  "INCIDENT",
];

/** Whether reaching this status means the run should give its slot back. */
export function releasesSlot(status: string): boolean {
  return SLOT_RELEASING_STATUSES.includes(status);
}

export interface AdmissionInput {
  /** `Workflow.maxActiveRuns`. `null` means no cap. */
  cap: number | null | undefined;
  /** Ids of runs of this workflow currently holding a slot. */
  holders: readonly string[];
  /** The run asking to execute. */
  runId: string;
}

export type Admission =
  | { kind: "admit" }
  /** `holders` is included so the operator can see what to look at. */
  | { kind: "throttle"; cap: number; holders: readonly string[] };

/**
 * Decide whether a run may start or resume.
 *
 * A run already holding a slot is always admitted. Resuming after an approval
 * is not a new arrival, and making it re-queue behind other runs would let a
 * cap of 1 deadlock a run against itself.
 */
export function admitRun({ cap, holders, runId }: AdmissionInput): Admission {
  if (cap === null || cap === undefined) return { kind: "admit" };
  if (holders.includes(runId)) return { kind: "admit" };
  if (holders.length < cap) return { kind: "admit" };
  return { kind: "throttle", cap, holders };
}

/**
 * How many further runs may be admitted right now. Never negative.
 *
 * Used when handing slots on: waking exactly one run per release leaves
 * capacity idle when two holders finish at once, because both releases pick the
 * same oldest waiting run.
 */
export function freeSlots(cap: number | null | undefined, holderCount: number): number {
  if (cap === null || cap === undefined) return Number.MAX_SAFE_INTEGER;
  return Math.max(0, cap - holderCount);
}

/** One line an operator can act on, for the journal payload and the timeline. */
export function throttleReason(cap: number, holders: readonly string[]): string {
  const n = holders.length;
  return `Waiting for a free slot: this workflow allows ${cap} active run${
    cap === 1 ? "" : "s"
  } and ${n} ${n === 1 ? "is" : "are"} in flight.`;
}

/**
 * Validate a cap coming from an API or a form.
 *
 * `null` clears it. Zero is rejected rather than read as "pause the workflow":
 * a cap that silently stops every run is a disable switch wearing a
 * concurrency setting's clothes, and it belongs behind its own explicit
 * control.
 */
export function parseMaxActiveRuns(value: unknown): number | null {
  if (value === null || value === undefined || value === "") return null;
  const n = typeof value === "string" ? Number(value) : value;
  if (typeof n !== "number" || !Number.isInteger(n)) {
    throw new Error("maxActiveRuns must be an integer or null");
  }
  if (n < 1) throw new Error("maxActiveRuns must be at least 1, or null for no cap");
  if (n > 100) throw new Error("maxActiveRuns must be 100 or less");
  return n;
}
