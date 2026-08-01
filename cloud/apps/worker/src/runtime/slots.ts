import { prisma, Prisma } from "@ghost/core/db";
import { appendRunEvent } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import {
  admitRun,
  freeSlots,
  releasesSlot,
  throttleReason,
  type Admission,
} from "@ghost/core/concurrency";
import { nudgeCompensation, nudgeRun } from "../queue.js";

/**
 * The database half of per-workflow concurrency control.
 *
 * `@ghost/core/concurrency` decides; this commits the decision under a lock and
 * wakes whatever is waiting. Kept out of `runWorkflow.ts` because the
 * compensation worker admits and releases too, and two copies of a "who gets
 * the slot" rule is how caps stop being caps.
 */

/**
 * How long a throttled run waits before re-checking on its own.
 *
 * The fast path is `releaseSlot` — a finishing run wakes the next one directly.
 * This is the safety net for when that never happens: a worker killed between
 * finishing a run and nudging the queue would otherwise leave every waiting run
 * of that workflow asleep forever. Deliberately slow, because it is the
 * unlikely path and each wake-up costs a job.
 */
const RECHECK_MS = 5 * 60_000;

type Tx = Prisma.TransactionClient;

/** Serializes admission and hand-off per workflow. Transaction-scoped. */
async function lockWorkflow(tx: Tx, workflowId: string): Promise<void> {
  await tx.$executeRaw`SELECT pg_advisory_xact_lock(hashtextextended(${`ghost:concurrency:${workflowId}`}, 0))`;
}

/** Runs of this workflow currently holding a slot, oldest first. */
async function holdersOf(tx: Tx, workflowId: string): Promise<string[]> {
  const rows = await tx.run.findMany({
    where: { slotHeldAt: { not: null }, workflowVersion: { workflowId } },
    select: { id: true },
    orderBy: { slotHeldAt: "asc" },
  });
  return rows.map((r) => r.id);
}

export interface AdmissionRequest {
  runId: string;
  workflowId: string;
  /** Whether the run has executed before — decides `startedAt`. */
  alreadyStarted: boolean;
  /** Status to move to on admission. Compensation stays COMPENSATING. */
  runningStatus?: "RUNNING" | "COMPENSATING";
}

/**
 * Claim a concurrency slot for a run, or report that it must wait.
 *
 * The cap is read, the holders counted, and the slot taken inside one
 * advisory-locked transaction. Every part of that matters:
 *
 * - Reading the cap *outside* the lock would let a PATCH that lowers an
 *   uncapped workflow to a finite cap slip in between, so a stale `null` admits
 *   a run into an already-full workflow.
 * - Counting in one statement and writing the claim in another would let two
 *   workers each see room and both take the last slot — the cap would hold
 *   everywhere except under the load it exists for.
 *
 * The caller must already hold the run's lease, so this only ever races other
 * *runs*, never another worker on the same run.
 */
export async function claimSlot(req: AdmissionRequest): Promise<Admission> {
  return prisma.$transaction(async (tx) => {
    await lockWorkflow(tx, req.workflowId);

    const workflow = await tx.workflow.findUnique({
      where: { id: req.workflowId },
      select: { maxActiveRuns: true },
    });
    const holders = await holdersOf(tx, req.workflowId);

    const decision = admitRun({ cap: workflow?.maxActiveRuns, holders, runId: req.runId });
    if (decision.kind === "admit") {
      const status = req.runningStatus ?? "RUNNING";
      await tx.run.update({
        where: { id: req.runId },
        data: {
          status,
          // Idempotent: a run resuming from a gate already holds its slot, and
          // re-stamping the time would reorder it against its true peers.
          slotHeldAt: holders.includes(req.runId) ? undefined : new Date(),
          ...(req.alreadyStarted ? {} : { startedAt: new Date() }),
        },
      });
    }
    return decision;
  });
}

/**
 * Record that a run is waiting, and arrange for it to be re-checked.
 *
 * The journal entry is written again only when what the operator would read has
 * actually changed — a run held for a day would otherwise add hundreds of
 * identical events to its hash chain, but suppressing on "the last event was
 * also a throttle" would leave the timeline linking to a holder that finished
 * hours ago. The run stays QUEUED; throttling is not a failure.
 */
export async function recordThrottle(
  runId: string,
  orgId: string,
  cap: number,
  holders: readonly string[],
  phase: "forward" | "compensation" = "forward",
): Promise<void> {
  const last = await prisma.runEvent.findFirst({
    where: { runId },
    orderBy: { seq: "desc" },
    select: { type: true, payload: true },
  });

  const previous =
    last?.type === RUN_EVENT_TYPES.runThrottled
      ? ((last.payload ?? {}) as { cap?: number; activeRunIds?: string[] })
      : null;
  const unchanged =
    previous !== null &&
    previous.cap === cap &&
    (previous.activeRunIds ?? []).join(",") === [...holders].join(",");

  if (!unchanged) {
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.runThrottled,
      payload: { cap, activeRunIds: [...holders], reason: throttleReason(cap, holders) },
    });
  }

  // Back to the queue the run is actually waiting on. A throttled reversal
  // nudged through the run queue would be dropped on the floor: that worker
  // only leases QUEUED / RUNNING / AWAITING_APPROVAL, so the reversal would sit
  // in COMPENSATING with nothing scheduled.
  const token = `throttled-${Date.now()}`;
  if (phase === "compensation") {
    await nudgeCompensation({ runId, orgId, resumeToken: token }, { delayMs: RECHECK_MS });
  } else {
    await nudgeRun(
      { runId, orgId, resumeToken: token, throttleRecheck: true },
      { delayMs: RECHECK_MS },
    );
  }
}

/**
 * Give a slot back and hand the freed capacity to whoever is waiting.
 *
 * Called when a run reaches a status at which it has stopped occupying the
 * customer's system. A no-op for a run that holds no slot — which is what makes
 * it safe to call unconditionally, and closes the loop where a *throttled* run
 * (still QUEUED, and usually the oldest queued run of its workflow) would
 * nominate itself and re-enqueue with no delay, spinning as fast as Postgres
 * and Redis would answer.
 *
 * The pick happens under the same lock as admission and wakes as many runs as
 * there is capacity for. Waking exactly one per release is not enough: two
 * holders finishing at once both select the same oldest waiting run, so one
 * slot sits idle until a five-minute re-check notices.
 */
export async function releaseSlot(runId: string): Promise<void> {
  const run = await prisma.run.findUnique({
    where: { id: runId },
    select: {
      status: true,
      slotHeldAt: true,
      workflowVersion: { select: { workflowId: true } },
    },
  });
  if (!run || run.slotHeldAt === null) return;
  // A run parked at its approval gate keeps its slot — that is the deliberate
  // trade-off in `SLOT_RELEASING_STATUSES`. This is called from a `finally`
  // that also runs on the gate path, so the rule has to be checked here rather
  // than assumed by the caller.
  if (!releasesSlot(run.status)) return;

  const workflowId = run.workflowVersion.workflowId;

  const toWake = await prisma.$transaction(async (tx) => {
    await lockWorkflow(tx, workflowId);
    await tx.run.updateMany({ where: { id: runId }, data: { slotHeldAt: null } });
    return nextInLine(tx, workflowId);
  });

  await wake(toWake);
}

/**
 * Wake waiting runs to fill whatever capacity a workflow currently has.
 *
 * Also the path used when an operator raises or clears a cap: the new capacity
 * is worthless if it is not offered to the runs already waiting on it.
 */
export async function fillFreeSlots(workflowId: string): Promise<void> {
  const toWake = await prisma.$transaction(async (tx) => {
    await lockWorkflow(tx, workflowId);
    return nextInLine(tx, workflowId);
  });
  await wake(toWake);
}

interface Waiting {
  id: string;
  orgId: string;
  status: string;
}

/**
 * The oldest runs waiting on a slot, up to the number free. Caller holds the lock.
 *
 * "Waiting" is two shapes, not one: a forward run sits in QUEUED, and a
 * reversal refused a slot sits in COMPENSATING holding none. Looking only for
 * QUEUED would leave a throttled undo asleep until its own re-check.
 */
async function nextInLine(tx: Tx, workflowId: string): Promise<Waiting[]> {
  const workflow = await tx.workflow.findUnique({
    where: { id: workflowId },
    select: { maxActiveRuns: true },
  });
  const cap = workflow?.maxActiveRuns ?? null;
  if (cap === null) return []; // nothing was ever held back

  const holders = await holdersOf(tx, workflowId);
  const free = freeSlots(cap, holders.length);
  if (free === 0) return [];

  return tx.run.findMany({
    where: {
      slotHeldAt: null,
      status: { in: ["QUEUED", "COMPENSATING"] },
      workflowVersion: { workflowId },
    },
    orderBy: { createdAt: "asc" },
    select: { id: true, orgId: true, status: true },
    take: free,
  });
}

/**
 * Over-waking is cheap — a second job finds the run leased, or throttled again,
 * and returns. Under-waking stalls a workflow until a re-check fires.
 */
async function wake(runs: readonly Waiting[]): Promise<void> {
  for (const next of runs) {
    const token = `admit-${Date.now()}-${next.id.slice(-6)}`;
    const send =
      next.status === "COMPENSATING"
        ? nudgeCompensation({ runId: next.id, orgId: next.orgId, resumeToken: token })
        : nudgeRun({ runId: next.id, orgId: next.orgId, resumeToken: token });
    await send.catch(() => undefined);
  }
}

/** Whether reaching this status means the run should hand its slot on. */
export { releasesSlot };
