import type { Job } from "bullmq";
import { prisma } from "@ghost/core/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import type { PurgeArtifactsJob } from "@ghost/core/queue";
import { createLogger, serializeError } from "@ghost/core/logger";
import { captureException } from "@ghost/core/sentry";
import {
  artifactRetentionDaysFromEnv,
  isEligibleForArtifactPurge,
  retentionCutoff,
} from "@ghost/core/retention";
import { artifactStore } from "../storage/artifacts.js";

const log = createLogger("worker", { job: "purge-artifacts" });

/**
 * Deletes the artifact prefix (screenshots + any residual session blobs) for
 * runs old enough that they are past the retention window, so a deployment's
 * object store does not grow forever. Scheduled as a repeatable BullMQ job
 * (see index.ts's `upsertJobScheduler` call) rather than enqueued per-run.
 *
 * Deliberately not part of the run's own lifecycle (unlike the session-blob
 * cleanup in runWorkflow.ts, which fires the moment a run stops needing to
 * resume): screenshots are the evidence a human reviews at an approval gate
 * and in the run timeline, and this job is what eventually lets that evidence
 * go, on a schedule an operator controls via `ARTIFACT_RETENTION_DAYS` — not
 * the moment the run itself ends.
 *
 * A run whose deletePrefix call fails (a storage outage, wrong bucket
 * permissions) is left unmarked and picked up again on the next run of this
 * job — see the catch below. `Run.artifactsPurgedAt` is only ever set after
 * the delete has actually succeeded.
 */

/** Bounds how many runs one job invocation processes, so a large backlog
 * (e.g. the first run of this job against an old deployment that never had
 * one) does not hold the job's BullMQ lock open indefinitely. The scheduler
 * re-fires on its own schedule, so the backlog drains over several ticks. */
const BATCH_SIZE = 200;

export interface PurgeArtifactsResult {
  purged: number;
  failed: number;
}

export async function purgeArtifactsJob(
  _job: Job<PurgeArtifactsJob>,
): Promise<PurgeArtifactsResult> {
  const retentionDays = artifactRetentionDaysFromEnv();
  const cutoff = retentionCutoff(new Date(), retentionDays);

  let purged = 0;
  let failed = 0;

  // Re-query each iteration rather than paginating: every run this loop
  // successfully processes gets `artifactsPurgedAt` set, which removes it
  // from the next query's result set. A run that fails is left unmarked and
  // would be re-selected forever within one job invocation, so it is
  // excluded explicitly via `id: { notIn }` for the rest of this run only.
  const failedIds = new Set<string>();
  for (;;) {
    const candidates = await prisma.run.findMany({
      where: {
        artifactsPurgedAt: null,
        endedAt: { not: null, lt: cutoff },
        ...(failedIds.size > 0 ? { id: { notIn: [...failedIds] } } : {}),
      },
      select: { id: true, orgId: true, endedAt: true, artifactsPurgedAt: true },
      take: BATCH_SIZE,
    });
    if (candidates.length === 0) break;

    for (const run of candidates) {
      if (!isEligibleForArtifactPurge(run, cutoff)) continue;
      try {
        await artifactStore().deletePrefix(`runs/${run.id}`);
        await prisma.run.update({
          where: { id: run.id },
          data: { artifactsPurgedAt: new Date() },
        });
        await appendAuditEvent(run.orgId, null, {
          action: "run.artifacts_purged",
          entityType: "Run",
          entityId: run.id,
          metadata: { retentionDays, endedAt: run.endedAt?.toISOString() ?? null },
        });
        purged += 1;
      } catch (err) {
        failed += 1;
        failedIds.add(run.id);
        log.error("failed to purge run artifacts", { runId: run.id, ...serializeError(err) });
        // Caught here rather than rethrown, so this job's own BullMQ
        // "failed" handler (index.ts) never fires for a single bad run — the
        // batch keeps going. That means this is the only place that can
        // report it to Sentry at all.
        captureException(err, { runId: run.id, job: "purge-artifacts" });
      }
    }

    // Every candidate in this batch either got purged or was added to
    // failedIds; if none were purged, looping again would just refetch the
    // same failed set forever.
    if (candidates.every((r) => failedIds.has(r.id))) break;
  }

  log.info("purge cycle complete", { purged, failed, retentionDays });

  return { purged, failed };
}
