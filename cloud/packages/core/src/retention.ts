/**
 * Pure eligibility rules for the artifact retention job (`purge-artifacts`
 * queue, apps/worker/src/jobs/purgeArtifacts.ts).
 *
 * Nothing here touches storage or the database — it only decides, given a
 * run's own timestamps, whether it is old enough to purge. Keeping the
 * decision pure and separate from the Prisma query means the actual retention
 * *window* logic can be tested without a database.
 */

/** Default when `ARTIFACT_RETENTION_DAYS` is unset. Generous on purpose: this
 * deletes evidence permanently, and a deployment that never configured a
 * retention window should not silently lose everything after the shortest
 * value the code happens to default to. */
export const DEFAULT_ARTIFACT_RETENTION_DAYS = 90;

export interface RetentionEligible {
  /** Null while a run is still active in any sense — QUEUED, RUNNING,
   * AWAITING_APPROVAL, COMPENSATING, or an unresolved INCIDENT never sets
   * this (see apps/worker/src/jobs/{runWorkflow,compensateRun}.ts). Using
   * `endedAt` rather than `status` means the eligibility rule does not need
   * its own copy of which statuses count as "done" — it inherits whatever
   * the engine already decided by setting or clearing this field. */
  endedAt: Date | null;
  /** Non-null once a prior purge cycle already deleted this run's artifact
   * prefix. Excluding these makes the job idempotent: re-running it (or
   * running it more than once a day) does not re-attempt a deleted prefix or
   * write a duplicate audit event. */
  artifactsPurgedAt: Date | null;
}

/** The cutoff instant: runs that ended before this are eligible. */
export function retentionCutoff(now: Date, retentionDays: number): Date {
  if (!Number.isFinite(retentionDays) || retentionDays <= 0) {
    throw new Error(`retentionDays must be a positive number, got ${retentionDays}`);
  }
  return new Date(now.getTime() - retentionDays * 24 * 60 * 60 * 1000);
}

/** Whether a single run's artifacts are due for deletion. */
export function isEligibleForArtifactPurge(run: RetentionEligible, cutoff: Date): boolean {
  if (run.artifactsPurgedAt !== null) return false;
  if (run.endedAt === null) return false;
  return run.endedAt.getTime() < cutoff.getTime();
}

/** Reads `ARTIFACT_RETENTION_DAYS` from the environment, falling back to the
 * default. Centralized so the job and anything documenting the value (e.g. a
 * future settings UI) read it the same way. */
export function artifactRetentionDaysFromEnv(
  env: Record<string, string | undefined> = process.env,
): number {
  const raw = env.ARTIFACT_RETENTION_DAYS;
  if (!raw) return DEFAULT_ARTIFACT_RETENTION_DAYS;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`ARTIFACT_RETENTION_DAYS must be a positive number, got ${JSON.stringify(raw)}`);
  }
  return parsed;
}
