import { Queue } from "bullmq";
import {
  QUEUE_NAMES,
  runWorkflowJobId,
  compensateRunJobId,
  type CompensateRunJob,
  type RunWorkflowJob,
} from "@ghost/core/queue";
import { createRedisConnection } from "./redis.js";

/**
 * The worker's one producer.
 *
 * Until concurrency control existed the worker was purely a consumer, and that
 * was worth keeping. It enqueues now for a single reason: when a run releases
 * its concurrency slot, the next run waiting on that slot has to be woken, and
 * the worker is the only process that knows the slot freed.
 *
 * Lazy, so a worker that never throttles never opens a second connection —
 * and so the tests, which import this module transitively, do not need Redis.
 */
let queue: Queue<RunWorkflowJob> | undefined;
let compQueue: Queue<CompensateRunJob> | undefined;

function runQueue(): Queue<RunWorkflowJob> {
  if (!queue) {
    queue = new Queue<RunWorkflowJob>(QUEUE_NAMES.runWorkflow, {
      connection: createRedisConnection(),
    });
  }
  return queue;
}

function compensateQueue(): Queue<CompensateRunJob> {
  if (!compQueue) {
    compQueue = new Queue<CompensateRunJob>(QUEUE_NAMES.compensateRun, {
      connection: createRedisConnection(),
    });
  }
  return compQueue;
}

/**
 * Nudge a run, optionally after a delay.
 *
 * `resumeToken` is not decoration. Completed jobs are retained
 * (`removeOnComplete`), so re-adding a run under the id it already used is
 * silently dropped — the run would sit in QUEUED forever with the API happily
 * reporting success. Every nudge from here is a *deliberate* re-drive of a run
 * that has been enqueued before, so every one carries a distinct token.
 */
export async function nudgeRun(
  data: RunWorkflowJob & { resumeToken: string },
  opts: { delayMs?: number } = {},
): Promise<void> {
  await runQueue().add(QUEUE_NAMES.runWorkflow, data, {
    jobId: runWorkflowJobId(data.runId, data.fromStepIndex, data.resumeToken),
    delay: opts.delayMs,
    attempts: 3,
    backoff: { type: "exponential", delay: 5_000 },
    removeOnComplete: { age: 3_600 },
    removeOnFail: { age: 86_400 },
  });
}

/**
 * The same nudge, for a reversal.
 *
 * A compensation waiting on a concurrency slot cannot be woken through the run
 * queue: `runWorkflowJob` only leases QUEUED / RUNNING / AWAITING_APPROVAL, so
 * a job sent there for a COMPENSATING run returns immediately and the reversal
 * never resumes.
 */
export async function nudgeCompensation(
  data: CompensateRunJob & { resumeToken: string },
  opts: { delayMs?: number } = {},
): Promise<void> {
  await compensateQueue().add(QUEUE_NAMES.compensateRun, data, {
    jobId: compensateRunJobId(data.runId, data.resumeToken),
    delay: opts.delayMs,
    attempts: 3,
    backoff: { type: "exponential", delay: 5_000 },
    removeOnComplete: { age: 3_600 },
    removeOnFail: { age: 86_400 },
  });
}

/** For tests and graceful shutdown. */
export async function closeQueue(): Promise<void> {
  if (queue) {
    await queue.close();
    queue = undefined;
  }
  if (compQueue) {
    await compQueue.close();
    compQueue = undefined;
  }
}
