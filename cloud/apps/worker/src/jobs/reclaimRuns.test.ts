import { describe, expect, it } from "vitest";
import { prisma, Prisma } from "@ghost/core/db";
import type { RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { MAX_ATTEMPTS, STALL_GRACE_MS, reclaimStalledRuns } from "./reclaimRuns.js";

/**
 * Covers the gap that made a dead worker look like a dead product: a run left
 * `RUNNING` with an expired lease and no job, which nothing ever picked up.
 *
 * Requires DATABASE_URL; skips cleanly without it, like the other DB-backed
 * suites here.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

interface Added {
  jobId?: string;
  data: RunWorkflowJob;
}

/** Records what would have been enqueued instead of touching Redis. */
function fakeQueue() {
  const added: Added[] = [];
  return {
    added,
    add: async (_name: string, data: RunWorkflowJob, opts?: { jobId?: string }) => {
      added.push({ jobId: opts?.jobId, data });
      return {} as never;
    },
  };
}

const STEPS: WorkflowSteps = [
  { id: "nav", type: "navigate", url: "http://127.0.0.1/never-fetched", label: "Open" },
];

async function seedRun(args: {
  status: "QUEUED" | "RUNNING" | "AWAITING_APPROVAL";
  leaseExpiresAt: Date | null;
  createdAt: Date;
  attempt?: number;
}): Promise<{ runId: string; orgId: string }> {
  const slug = `reclaim-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const org = await prisma.organization.create({ data: { name: "Reclaim test", slug } });
  const workflow = await prisma.workflow.create({
    data: { orgId: org.id, name: "Reclaim test workflow" },
  });
  const version = await prisma.workflowVersion.create({
    data: {
      workflowId: workflow.id,
      version: 1,
      steps: STEPS as unknown as Prisma.InputJsonValue,
    },
  });
  const run = await prisma.run.create({
    data: {
      orgId: org.id,
      workflowVersionId: version.id,
      status: args.status,
      cursor: 0,
      attempt: args.attempt ?? 0,
      leaseOwner: args.leaseExpiresAt ? "dead-worker" : null,
      leaseExpiresAt: args.leaseExpiresAt,
      createdAt: args.createdAt,
    },
  });
  return { runId: run.id, orgId: org.id };
}

describe.skipIf(!hasDb)("reclaimStalledRuns (Postgres)", () => {
  const longAgo = () => new Date(Date.now() - 10 * 60_000);

  it("re-enqueues a RUNNING run whose lease expired", async () => {
    const { runId } = await seedRun({
      status: "RUNNING",
      leaseExpiresAt: longAgo(),
      createdAt: longAgo(),
    });

    const queue = fakeQueue();
    const result = await reclaimStalledRuns(queue);

    expect(result.requeued).toContain(runId);
    expect(queue.added.some((a) => a.data.runId === runId)).toBe(true);
  });

  it("leaves a run alone while its lease is still live", async () => {
    const { runId } = await seedRun({
      status: "RUNNING",
      // Comfortably inside the grace window, as a healthy worker's would be.
      leaseExpiresAt: new Date(Date.now() + 30_000),
      createdAt: longAgo(),
    });

    const queue = fakeQueue();
    const result = await reclaimStalledRuns(queue);

    expect(result.requeued).not.toContain(runId);
  });

  it("does not disturb a run waiting on a human", async () => {
    // The case that would be actively harmful: a gate is *meant* to sit for
    // days with no lease. Re-driving it would reopen an approval nobody touched.
    const { runId } = await seedRun({
      status: "AWAITING_APPROVAL",
      leaseExpiresAt: null,
      createdAt: longAgo(),
    });

    const queue = fakeQueue();
    const result = await reclaimStalledRuns(queue);

    expect(result.requeued).not.toContain(runId);
    expect(result.incidents).not.toContain(runId);
  });

  it("respects the grace period rather than racing a slow renewal", async () => {
    const { runId } = await seedRun({
      status: "RUNNING",
      // Expired, but only just — a worker mid-renewal looks like this.
      leaseExpiresAt: new Date(Date.now() - Math.floor(STALL_GRACE_MS / 2)),
      createdAt: longAgo(),
    });

    const queue = fakeQueue();
    const result = await reclaimStalledRuns(queue);

    expect(result.requeued).not.toContain(runId);
  });

  it("stops restarting after MAX_ATTEMPTS and raises an incident instead", async () => {
    const { runId } = await seedRun({
      status: "RUNNING",
      leaseExpiresAt: longAgo(),
      createdAt: longAgo(),
      attempt: MAX_ATTEMPTS,
    });

    const queue = fakeQueue();
    const result = await reclaimStalledRuns(queue);

    expect(result.incidents).toContain(runId);
    expect(result.requeued).not.toContain(runId);

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/stopped restarting it automatically/);
    // The lease must be released, or a human retry could not take the run.
    expect(run.leaseOwner).toBeNull();
  });

  it("gives two concurrent sweeps the same job id so they collapse", async () => {
    const { runId } = await seedRun({
      status: "RUNNING",
      leaseExpiresAt: longAgo(),
      createdAt: longAgo(),
    });

    const a = fakeQueue();
    const b = fakeQueue();
    await reclaimStalledRuns(a);
    await reclaimStalledRuns(b);

    const idA = a.added.find((x) => x.data.runId === runId)?.jobId;
    const idB = b.added.find((x) => x.data.runId === runId)?.jobId;
    expect(idA).toBeDefined();
    expect(idA).toBe(idB);
  });
});
