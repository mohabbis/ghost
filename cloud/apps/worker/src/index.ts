import { Worker } from "bullmq";
import { QUEUE_NAMES, type NoopJob } from "@ghost/core/queue";
import { createRedisConnection } from "./redis.js";

/**
 * Ghost worker entrypoint.
 *
 * Phase 0: consumes the `noop` queue to prove the web ↔ Redis ↔ worker wiring.
 * Phase 1 adds a `run-workflow` Worker that drives Playwright (see the project
 * plan). Each queue gets its own Worker so failures stay isolated.
 */

const connection = createRedisConnection();

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
  await noopWorker.close();
  await connection.quit();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));
