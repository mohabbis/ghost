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
 * Pure: no Prisma, no Redis. The caller supplies the current holders and gets
 * back a decision it must then commit under a lock.
 */

/**
 * Statuses that hold a slot.
 *
 * The rule is *in flight*, and the two exclusions are deliberate:
 *
 * - `QUEUED` is the waiting pool itself. Counting it would deadlock instantly.
 * - `INCIDENT` is an indefinite park. A run that failed and is waiting on a
 *   human is not touching the customer's system, and one broken run must not
 *   wedge its workflow forever. It re-enters admission when a human retries it,
 *   so the cap still holds on what is actually executing.
 *
 * `AWAITING_APPROVAL` *does* hold its slot, unlike `INCIDENT`. An approval is
 * part of a run's normal flow and is expected to resolve in minutes, and
 * letting a later run overtake one parked at its gate would interleave two runs
 * against the same system — the exact thing the cap exists to prevent. The
 * cost is real: a cap of 1 plus an approval left sitting for a day stalls every
 * later run of that workflow. That is visible in the timeline (`run.throttled`
 * names the holder) and is the safer of the two failure modes.
 */
export const SLOT_HOLDING_STATUSES: readonly string[] = [
  "RUNNING",
  "AWAITING_APPROVAL",
  "COMPENSATING",
];

export function holdsSlot(status: string): boolean {
  return SLOT_HOLDING_STATUSES.includes(status);
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
