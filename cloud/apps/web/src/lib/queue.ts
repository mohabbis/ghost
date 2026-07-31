import "server-only";
import { Queue } from "bullmq";
import IORedis from "ioredis";
import {
  QUEUE_NAMES,
  runWorkflowJobId,
  type NoopJob,
  type RunWorkflowJob,
} from "@ghost/core/queue";

/**
 * Queue producers for the web app. The web side only *enqueues*; the worker
 * consumes. Connections are cached on globalThis to survive dev hot-reload.
 */
const globalForQueue = globalThis as unknown as {
  ghostRedis?: IORedis;
  ghostQueues?: Map<string, Queue>;
};

function connection(): IORedis {
  if (!globalForQueue.ghostRedis) {
    const url = process.env.REDIS_URL;
    if (!url) throw new Error("REDIS_URL is not set — see cloud/.env.example");
    globalForQueue.ghostRedis = new IORedis(url, { maxRetriesPerRequest: null });
  }
  return globalForQueue.ghostRedis;
}

function getQueue(name: string): Queue {
  if (!globalForQueue.ghostQueues) globalForQueue.ghostQueues = new Map();
  let q = globalForQueue.ghostQueues.get(name);
  if (!q) {
    q = new Queue(name, { connection: connection() });
    globalForQueue.ghostQueues.set(name, q);
  }
  return q;
}

export function enqueueNoop(data: NoopJob) {
  return getQueue(QUEUE_NAMES.noop).add(QUEUE_NAMES.noop, data);
}

export function enqueueRunWorkflow(data: RunWorkflowJob) {
  return getQueue(QUEUE_NAMES.runWorkflow).add(QUEUE_NAMES.runWorkflow, data, {
    jobId: runWorkflowJobId(data.runId, data.fromStepIndex),
    // Queue-level retries are only safe because re-entry is idempotent: the run
    // lease blocks a concurrent worker and the journal blocks re-running a
    // completed step. `runWorkflowJob` records business failures rather than
    // throwing, so only infrastructure errors reach these attempts.
    attempts: 3,
    backoff: { type: "exponential", delay: 5_000 },
    removeOnComplete: { age: 3_600 },
    removeOnFail: { age: 86_400 },
  });
}
