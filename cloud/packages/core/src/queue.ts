/**
 * Shared queue contract between apps/web (producer) and apps/worker (consumer).
 * Names and payload shapes live here so both sides stay in sync. The actual
 * BullMQ Queue/Worker construction lives in each app (they own their Redis
 * connection lifecycle).
 */

export const QUEUE_NAMES = {
  /** Executes a workflow run (Phase 1). */
  runWorkflow: "run-workflow",
  /** Reverses a run's completed side effects (BPMN compensation). */
  compensateRun: "compensate-run",
  /** Deletes artifact prefixes for runs past the retention window. Scheduled
   * as a repeatable job (see apps/worker/src/index.ts); never enqueued by the
   * web app. */
  purgeArtifacts: "purge-artifacts",
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
  /**
   * Distinguishes repeated *legitimate* resumes from the same point.
   *
   * A run that gates at step 3, is approved, fails, and is then retried by a
   * human from an incident resumes from step 3 twice. Without this, both jobs
   * share an id — and because completed jobs are retained (`removeOnComplete`),
   * the second `add` is silently dropped: the API reports success, the run
   * sits in QUEUED, and no worker ever picks it up.
   */
  resumeToken?: string;
  /**
   * This job is a throttled run's own safety-net re-check, not a real resume.
   *
   * It sits in Redis for five minutes, so a run that gets handed a slot
   * directly in the meantime — and has since reached an approval gate — would
   * otherwise be re-driven by this stale job: back to RUNNING, another
   * `run.resumed`, another `gate.opened`, another approval audit entry, none of
   * it corresponding to anything a human did. Marked so it can no-op once the
   * run is no longer waiting.
   */
  throttleRecheck?: boolean;
}

/**
 * Deterministic job id, so a double-clicked Run button or a double-submitted
 * approval collapses to one job instead of racing two workers over one run.
 *
 * Keyed on the resume point *and* an optional resume token. The step index
 * alone is not enough: an approval resume and a later incident retry both
 * resume from the same index, and the second would be swallowed as a duplicate.
 * Callers that are re-driving a run deliberately pass a token; callers
 * deduplicating a user double-click pass none, and keep collapsing.
 *
 * This is a first line of defence with a time window, not a guarantee —
 * `removeOnComplete` eventually frees the id. The guarantees are the run lease
 * and journal-derived position.
 *
 * **No colons.** BullMQ rejects a custom job id unless it contains either no
 * colon or exactly two (it splits on `:` to stay compatible with repeatable-job
 * keys). The previous `run:<id>:<from>:<token>` form has three, so every call
 * that passed a resume token threw before reaching Redis — which is every
 * incident retry, every incident skip, and the cleanup enqueue on cancel and
 * on a rejected approval. Keeping the separator out of the alphabet entirely
 * means no future segment can re-break it.
 */
export function runWorkflowJobId(
  runId: string,
  fromStepIndex?: number,
  resumeToken?: string,
): string {
  const base = `run-${runId}-${fromStepIndex ?? "start"}`;
  return resumeToken ? `${base}-${resumeToken}` : base;
}

/** Payload for a `compensateRun` job. */
export interface CompensateRunJob {
  runId: string;
  orgId: string;
  /**
   * Who asked for the reversal.
   *
   * Not the same person as `Run.triggeredById`, and the difference is the whole
   * point: attributing a reversal to whoever started the original run — often
   * an agent, or a colleague — puts the wrong name against a mutation in the
   * audit log.
   */
  requestedById?: string | null;
  /**
   * Distinguishes a resume from the initial request, for the same reason
   * `RunWorkflowJob.resumeToken` does. A compensation that gates and is then
   * approved re-enters through this queue, and without a fresh token the
   * retained completed job swallows the resume — leaving the run COMPENSATING
   * with nothing scheduled.
   */
  resumeToken?: string;
}

/**
 * Deterministic id so a double-clicked Undo collapses to a single job.
 *
 * Colon-free for the same reason as `runWorkflowJobId` — one colon is as
 * illegal as three.
 */
export function compensateRunJobId(runId: string, resumeToken?: string): string {
  const base = `compensate-${runId}`;
  return resumeToken ? `${base}-${resumeToken}` : base;
}

/** Payload for the Phase 0 no-op wiring test. */
export interface NoopJob {
  message: string;
  requestedAt: string;
}

/** Payload for a `purgeArtifacts` run. Empty: the job re-derives everything
 * it needs (the retention window, which runs are eligible) at execution
 * time rather than trusting a scheduled-at snapshot, since a purge cycle can
 * be delayed behind other work. */
export type PurgeArtifactsJob = Record<string, never>;

export function redisConnectionFromEnv(): { url: string } {
  const url = process.env.REDIS_URL;
  if (!url) {
    throw new Error("REDIS_URL is not set");
  }
  return { url };
}
