/**
 * Shared queue contract between apps/web (producer) and apps/worker (consumer).
 * Names and payload shapes live here so both sides stay in sync. The actual
 * BullMQ Queue/Worker construction lives in each app (they own their Redis
 * connection lifecycle).
 */

export const QUEUE_NAMES = {
  /** Executes a workflow run (Phase 1). */
  runWorkflow: "run-workflow",
  /** Phase 0 wiring smoke test. */
  noop: "noop",
} as const;

export type QueueName = (typeof QUEUE_NAMES)[keyof typeof QUEUE_NAMES];

/** Payload for a `runWorkflow` job (Phase 1). */
export interface RunWorkflowJob {
  runId: string;
  orgId: string;
  /**
   * The gate index this job resumes from.
   *
   * Feeds the job id ONLY — it is never used for positioning. Execution
   * position is derived by folding the run journal, so a wrong or stale value
   * here cannot cause a step to run twice.
   */
  fromStepIndex?: number;
}

/**
 * Deterministic job id, so a double-clicked Run button or a double-submitted
 * approval collapses to one job instead of racing two workers over one run.
 *
 * Keyed on the resume point as well as the run: a resume after approval is a
 * *legitimate* second job for the same run, and BullMQ rejects a duplicate id
 * while the job still exists.
 *
 * This is a first line of defence with a time window, not a guarantee —
 * `removeOnComplete` eventually frees the id. The guarantees are the run lease
 * and journal-derived position.
 */
export function runWorkflowJobId(runId: string, fromStepIndex?: number): string {
  return `run:${runId}:${fromStepIndex ?? "start"}`;
}

/** Payload for the Phase 0 no-op wiring test. */
export interface NoopJob {
  message: string;
  requestedAt: string;
}

export function redisConnectionFromEnv(): { url: string } {
  const url = process.env.REDIS_URL;
  if (!url) {
    throw new Error("REDIS_URL is not set");
  }
  return { url };
}
