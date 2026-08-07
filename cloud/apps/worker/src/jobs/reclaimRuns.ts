import type { Queue } from "bullmq";
import { prisma } from "@ghost/core/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { runWorkflowJobId, type RunWorkflowJob } from "@ghost/core/queue";
import { createLogger } from "@ghost/core/logger";

const log = createLogger("reclaim-runs");

/**
 * Restarts runs whose worker died.
 *
 * The lease in `runWorkflow` prevents two workers from executing one run at
 * once, and it correctly lets a *new* job take over a lease that has expired.
 * What was missing is the thing that produces that new job. When a worker
 * process disappeared mid-run — a crash, a redeploy, a laptop closing — the
 * BullMQ job died with it, so nothing ever re-enqueued the run. The row stayed
 * `RUNNING` with an expired lease **forever**, and the UI showed a run in
 * progress that no process was working on. That is indistinguishable, to the
 * person watching, from Ghost simply not working.
 *
 * Restarting is safe rather than merely convenient, and only because of how
 * the run journal is built: position is folded from an append-only,
 * hash-chained log, so a step that already completed is not executed again on
 * resume. This function does not re-run anything; it re-enters a run at the
 * position its own journal proves it reached.
 *
 * Runs that have stalled too many times are not restarted forever. Past
 * `MAX_ATTEMPTS` the run is moved to `INCIDENT`, where a human decides —
 * exactly what an incident is for. Looping instead would keep a workflow that
 * reliably kills its worker doing so.
 *
 * Touches: database (run rows, audit log), Redis (enqueue). No browser, no
 * network to customer systems.
 */

/**
 * How far past lease expiry a run must be before it is considered abandoned.
 *
 * The lease is renewed every LEASE_MS/3 (20s), so brief scheduling delays and
 * clock skew must not look like death. A live worker that is merely slow to
 * renew keeps its run; only one that has been silent well beyond its renewal
 * interval loses it.
 */
export const STALL_GRACE_MS = 90_000;

/** Restarts beyond this become an incident for a human instead. */
export const MAX_ATTEMPTS = 5;

/** How often the worker sweeps. Cheap: one indexed query. */
export const RECLAIM_INTERVAL_MS = 60_000;

export interface ReclaimResult {
  requeued: string[];
  incidents: string[];
}

export async function reclaimStalledRuns(
  queue: Pick<Queue<RunWorkflowJob>, "add">,
  now: Date = new Date(),
): Promise<ReclaimResult> {
  const cutoff = new Date(now.getTime() - STALL_GRACE_MS);

  const stalled = await prisma.run.findMany({
    where: {
      // Only states where a worker is supposed to be actively holding the run.
      // AWAITING_APPROVAL is deliberately absent: a run waiting on a human is
      // meant to sit for days with no lease, and re-driving one would reopen
      // gates nobody touched.
      status: { in: ["QUEUED", "RUNNING"] },
      OR: [{ leaseExpiresAt: null }, { leaseExpiresAt: { lt: cutoff } }],
      createdAt: { lt: cutoff },
    },
    select: { id: true, orgId: true, cursor: true, attempt: true, leaseExpiresAt: true },
    // Bounded so one sweep cannot enqueue an unbounded burst after a long
    // outage; the next sweep takes the rest.
    take: 50,
  });

  const result: ReclaimResult = { requeued: [], incidents: [] };

  for (const run of stalled) {
    if (run.attempt >= MAX_ATTEMPTS) {
      // updateMany with the status in the filter: if a worker revived and
      // finished this run since the query above, do not overwrite its outcome.
      const moved = await prisma.run.updateMany({
        where: { id: run.id, status: { in: ["QUEUED", "RUNNING"] } },
        data: {
          status: "INCIDENT",
          error:
            `Run stopped making progress ${MAX_ATTEMPTS} times without finishing. ` +
            "Ghost stopped restarting it automatically. Retry, skip the failing step, or cancel.",
          leaseOwner: null,
          leaseExpiresAt: null,
        },
      });
      if (moved.count === 1) {
        result.incidents.push(run.id);
        await appendAuditEvent(run.orgId, null, {
          action: "run.reclaim_exhausted",
          entityType: "Run",
          entityId: run.id,
          metadata: { attempts: run.attempt },
        }).catch((err) => log.error("audit failed for reclaim_exhausted", { runId: run.id, err }));
      }
      continue;
    }

    // Derived from the expired lease rather than from the clock, so two worker
    // replicas sweeping at the same moment produce the *same* job id and
    // collapse into one job instead of racing. A later stall of the same run
    // has a different expiry and so gets a fresh id, which is what stops the
    // retained completed job from swallowing it.
    const resumeToken = `reclaim-${run.leaseExpiresAt?.getTime() ?? "none"}`;

    await queue.add(
      "run-workflow",
      { runId: run.id, orgId: run.orgId, fromStepIndex: run.cursor },
      {
        jobId: runWorkflowJobId(run.id, run.cursor, resumeToken),
        removeOnComplete: 100,
        removeOnFail: 500,
      },
    );
    result.requeued.push(run.id);

    await appendAuditEvent(run.orgId, null, {
      action: "run.reclaimed",
      entityType: "Run",
      entityId: run.id,
      metadata: { cursor: run.cursor, attempt: run.attempt, reason: "lease expired" },
    }).catch((err) => log.error("audit failed for reclaim", { runId: run.id, err }));
  }

  if (result.requeued.length > 0 || result.incidents.length > 0) {
    log.info("reclaimed stalled runs", {
      requeued: result.requeued.length,
      incidents: result.incidents.length,
    });
  }
  return result;
}
