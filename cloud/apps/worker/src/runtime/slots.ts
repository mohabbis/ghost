import { prisma, Prisma, type RunStatus } from "@ghost/core/db";
import { appendRunEvent } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import {
  admitRun,
  holdsSlot,
  throttleReason,
  SLOT_HOLDING_STATUSES,
  type Admission,
} from "@ghost/core/concurrency";
import { nudgeRun } from "../queue.js";

/**
 * The database half of per-workflow concurrency control.
 *
 * `@ghost/core/concurrency` decides; this commits the decision under a lock and
 * wakes whatever is waiting. Kept out of `runWorkflow.ts` because the release
 * path also runs from the compensation worker, and two copies of a
 * "who gets the slot" rule is how caps stop being caps.
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

/**
 * The same set, in the shape Prisma's generated `RunStatus` filter wants.
 *
 * Derived from the core constant rather than retyped, so a status added there
 * fails to compile here instead of quietly dropping out of the count.
 */
const HOLDING = [...SLOT_HOLDING_STATUSES] as RunStatus[];

type Tx = Prisma.TransactionClient;

/** Serializes admission per workflow. Transaction-scoped: released on commit. */
async function lockWorkflow(tx: Tx, workflowId: string): Promise<void> {
  await tx.$executeRaw`SELECT pg_advisory_xact_lock(hashtextextended(${`ghost:concurrency:${workflowId}`}, 0))`;
}

export interface AdmissionRequest {
  runId: string;
  workflowId: string;
  cap: number | null;
  /** Whether the run has executed before — decides `startedAt`. */
  alreadyStarted: boolean;
}

/**
 * Claim a concurrency slot for a run, or report that it must wait.
 *
 * The count and the claim happen inside one advisory-locked transaction. Doing
 * the count in one statement and the `status = RUNNING` write in another would
 * let two workers each see room and both take the last slot — the cap would
 * hold everywhere except under the load it exists for.
 *
 * The caller must already hold the run's lease, so this only ever races other
 * *runs*, never another worker on the same run.
 */
export async function claimSlot(req: AdmissionRequest): Promise<Admission> {
  return prisma.$transaction(async (tx) => {
    await lockWorkflow(tx, req.workflowId);

    const holders = await tx.run.findMany({
      where: {
        status: { in: HOLDING },
        workflowVersion: { workflowId: req.workflowId },
      },
      select: { id: true },
      orderBy: { createdAt: "asc" },
    });

    const decision = admitRun({ cap: req.cap, holders: holders.map((h) => h.id), runId: req.runId });
    if (decision.kind === "admit") {
      await tx.run.update({
        where: { id: req.runId },
        data: req.alreadyStarted
          ? { status: "RUNNING" }
          : { status: "RUNNING", startedAt: new Date() },
      });
    }
    return decision;
  });
}

/**
 * Record that a run is waiting, and arrange for it to be re-checked.
 *
 * The journal entry is written at most once per wait rather than once per
 * re-check: a run held for a day would otherwise add hundreds of identical
 * events to its hash chain, burying the events that mean something. The run
 * stays QUEUED — throttling is not a failure and must not read as one.
 */
export async function recordThrottle(
  runId: string,
  orgId: string,
  cap: number,
  holders: readonly string[],
): Promise<void> {
  const last = await prisma.runEvent.findFirst({
    where: { runId },
    orderBy: { seq: "desc" },
    select: { type: true },
  });
  if (last?.type !== RUN_EVENT_TYPES.runThrottled) {
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.runThrottled,
      payload: {
        cap,
        activeRunIds: [...holders],
        reason: throttleReason(cap, holders),
      },
    });
  }

  await nudgeRun(
    { runId, orgId, resumeToken: `throttled-${Date.now()}` },
    { delayMs: RECHECK_MS },
  );
}

/**
 * Hand a freed slot to the next run waiting on it.
 *
 * Called when a run stops holding its slot, for any reason — finished, failed,
 * canceled, or parked as an incident. A no-op when the workflow has no cap,
 * since nothing was ever held back.
 *
 * Waking one run per release is correct even under a race: two releases that
 * both wake the same run produce two jobs, and the second finds the run already
 * leased and returns. Over-waking is cheap; under-waking stalls a workflow.
 */
export async function releaseSlot(runId: string): Promise<void> {
  const run = await prisma.run.findUnique({
    where: { id: runId },
    select: {
      status: true,
      workflowVersion: {
        select: { workflowId: true, workflow: { select: { maxActiveRuns: true } } },
      },
    },
  });
  if (!run) return;
  if (holdsSlot(run.status)) return; // still holding it
  const cap = run.workflowVersion.workflow.maxActiveRuns;
  if (cap === null) return;

  const next = await prisma.run.findFirst({
    where: {
      status: "QUEUED",
      workflowVersion: { workflowId: run.workflowVersion.workflowId },
    },
    orderBy: { createdAt: "asc" },
    select: { id: true, orgId: true },
  });
  if (!next) return;

  await nudgeRun({ runId: next.id, orgId: next.orgId, resumeToken: `admit-${Date.now()}` });
}
