// First, before anything reads process.env: load cloud/.env. The worker runs
// from apps/worker and nothing else puts that file into its environment.
import "@ghost/core/env";
import { Queue, Worker } from "bullmq";
import {
  QUEUE_NAMES,
  type CompensateRunJob,
  type NoopJob,
  type PurgeArtifactsJob,
  type RunWorkflowJob,
} from "@ghost/core/queue";
import { createLogger, serializeError } from "@ghost/core/logger";
import { initSentry, captureException } from "@ghost/core/sentry";
import { createRedisConnection } from "./redis.js";
import { runWorkflowJob } from "./jobs/runWorkflow.js";
import { compensateRunJob } from "./jobs/compensateRun.js";
import { purgeArtifactsJob } from "./jobs/purgeArtifacts.js";
import { reclaimStalledRuns, RECLAIM_INTERVAL_MS } from "./jobs/reclaimRuns.js";

/**
 * Ghost worker entrypoint.
 *
 * `noop` proves the web ↔ Redis ↔ worker wiring (Phase 0). `run-workflow`
 * executes a workflow run via Playwright, halting at approval gates (Phase 1).
 * Each queue gets its own Worker so failures stay isolated.
 */

initSentry("worker");
const log = createLogger("worker");
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
  log.error("run-workflow job failed", { runId: job?.data.runId, ...serializeError(err) });
  captureException(err, { runId: job?.data.runId, queue: QUEUE_NAMES.runWorkflow });
});

// Reversal shares the run's lock duration but runs at lower concurrency: it is
// rare, and a half-finished undo is the worst state to pile more work onto.
const compensateWorker = new Worker<CompensateRunJob>(
  QUEUE_NAMES.compensateRun,
  compensateRunJob,
  { connection, concurrency: 1, lockDuration: 60_000, stalledInterval: 30_000, maxStalledCount: 1 },
);
compensateWorker.on("failed", (job, err) => {
  log.error("compensate-run job failed", { runId: job?.data.runId, ...serializeError(err) });
  captureException(err, { runId: job?.data.runId, queue: QUEUE_NAMES.compensateRun });
});

const noopWorker = new Worker<NoopJob>(
  QUEUE_NAMES.noop,
  async (job) => {
    log.info("noop job received", { jobId: job.id, message: job.data.message, requestedAt: job.data.requestedAt });
    return { ok: true, handledAt: new Date().toISOString() };
  },
  { connection },
);

noopWorker.on("completed", (job) => {
  log.info("noop job completed", { jobId: job.id });
});
noopWorker.on("failed", (job, err) => {
  log.error("noop job failed", { jobId: job?.id, ...serializeError(err) });
});

const purgeWorker = new Worker<PurgeArtifactsJob>(
  QUEUE_NAMES.purgeArtifacts,
  purgeArtifactsJob,
  { connection, concurrency: 1 },
);
purgeWorker.on("failed", (job, err) => {
  log.error("purge-artifacts job failed", { jobId: job?.id, ...serializeError(err) });
  captureException(err, { jobId: job?.id, queue: QUEUE_NAMES.purgeArtifacts });
});

// Schedules the recurring purge itself, rather than relying on an external
// cron to enqueue it. `upsertJobScheduler` is idempotent on its scheduler id,
// so every boot (including a redeploy) re-asserts the same daily schedule
// instead of accumulating a duplicate one — this line runs on every worker
// start, not just the first.
const purgeQueue = new Queue<PurgeArtifactsJob>(QUEUE_NAMES.purgeArtifacts, { connection });
await purgeQueue.upsertJobScheduler(
  "purge-artifacts-daily",
  { every: 24 * 60 * 60 * 1000 },
  { name: "purge-artifacts", data: {} },
);

// Pick up runs whose worker died. Immediately on boot — a redeploy is the
// commonest way to orphan a run, and the new process is standing right where
// the old one fell — and then on an interval for crashes that happen while
// this process is up. Safe to run in every replica: the job id is derived from
// the expired lease, so concurrent sweeps collapse into one job, and the run
// lease still admits exactly one executor.
const reclaimQueue = new Queue<RunWorkflowJob>(QUEUE_NAMES.runWorkflow, { connection });
const sweepStalledRuns = async (): Promise<void> => {
  try {
    await reclaimStalledRuns(reclaimQueue);
  } catch (err) {
    // A failed sweep must never take the worker down with it: the queues above
    // are still executing real runs.
    log.error("reclaim sweep failed", serializeError(err));
    captureException(err, { queue: QUEUE_NAMES.runWorkflow, phase: "reclaim" });
  }
};
await sweepStalledRuns();
const reclaimTimer = setInterval(() => void sweepStalledRuns(), RECLAIM_INTERVAL_MS);
// Do not hold the process open on this timer alone.
reclaimTimer.unref();

log.info("Ghost worker started", { queues: Object.values(QUEUE_NAMES) });

async function shutdown(signal: string): Promise<void> {
  log.info("shutting down", { signal });
  clearInterval(reclaimTimer);
  await Promise.all([
    reclaimQueue.close(),
    noopWorker.close(),
    runWorker.close(),
    compensateWorker.close(),
    purgeWorker.close(),
    purgeQueue.close(),
  ]);
  await connection.quit();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));
