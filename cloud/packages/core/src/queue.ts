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
  /** Resume an approval-halted run from this step index. */
  fromStepIndex?: number;
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
