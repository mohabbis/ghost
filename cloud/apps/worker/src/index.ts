import { Worker } from "bullmq";
import { QUEUE_NAMES, type NoopJob, type RunWorkflowJob } from "@ghost/core/queue";
import { createRedisConnection } from "./redis.js";
import { runWorkflowJob } from "./jobs/runWorkflow.js";

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
});
runWorker.on("failed", (job, err) => {
  console.error(`[worker] run-workflow ${job?.data.runId} failed:`, err);
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
  await Promise.all([noopWorker.close(), runWorker.close()]);
  await connection.quit();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));
