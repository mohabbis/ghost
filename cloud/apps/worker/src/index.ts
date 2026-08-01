import { Worker } from "bullmq";
import {
  QUEUE_NAMES,
  type CompensateRunJob,
  type NoopJob,
  type RunWorkflowJob,
} from "@ghost/core/queue";
import { createRedisConnection } from "./redis.js";
import { runWorkflowJob } from "./jobs/runWorkflow.js";
import { compensateRunJob } from "./jobs/compensateRun.js";

/**
 * Ghost worker entrypoint.
 *
 * `noop` proves the web ↔ Redis ↔ worker wiring (Phase 0). `run-workflow`
 * executes a workflow run via Playwright, halting at approval gates (Phase 1).
 * Each queue gets its own Worker so failures stay isolated.
 */

const connection = createRedisConnection();

const runWorker = new Worker<RunWorkflowJob>(QUEUE_NAMES.runWorkflow, runWorkflowJob, {
  connection,
  // A run holds a browser, so concurrency is memory-bound rather than
  // CPU-bound; keep it small and explicit rather than relying on the default.
  concurrency: Number(process.env.WORKER_CONCURRENCY ?? 2),
  // Must be >= the run lease in jobs/runWorkflow.ts, or BullMQ declares a job
  // stalled and redelivers it while the original worker still owns the run.
  lockDuration: 60_000,
  stalledInterval: 30_000,
  maxStalledCount: 1,
});
runWorker.on("failed", (job, err) => {
  console.error(`[worker] run-workflow ${job?.data.runId} failed:`, err);
});

// Reversal shares the run's lock duration but runs at lower concurrency: it is
// rare, and a half-finished undo is the worst state to pile more work onto.
const compensateWorker = new Worker<CompensateRunJob>(
  QUEUE_NAMES.compensateRun,
  compensateRunJob,
  { connection, concurrency: 1, lockDuration: 60_000, stalledInterval: 30_000, maxStalledCount: 1 },
);
compensateWorker.on("failed", (job, err) => {
  console.error(`[worker] compensate-run ${job?.data.runId} failed:`, err);
});

const noopWorker = new Worker<NoopJob>(
  QUEUE_NAMES.noop,
  async (job) => {
    console.log(
      `[worker] noop job ${job.id}: "${job.data.message}" (requested ${job.data.requestedAt})`,
    );
    return { ok: true, handledAt: new Date().toISOString() };
  },
  { connection },
);

noopWorker.on("completed", (job) => {
  console.log(`[worker] completed ${job.id}`);
});
noopWorker.on("failed", (job, err) => {
  console.error(`[worker] failed ${job?.id}:`, err);
});

console.log("[worker] Ghost worker started. Listening on queues:", Object.values(QUEUE_NAMES));

async function shutdown(signal: string): Promise<void> {
  console.log(`[worker] ${signal} received, shutting down…`);
  await Promise.all([noopWorker.close(), runWorker.close(), compensateWorker.close()]);
  await connection.quit();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));
