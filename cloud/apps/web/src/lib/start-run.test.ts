import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * A run that cannot be queued must not look like one that is waiting its turn.
 *
 * The row has to exist before there is an id to enqueue, so the two writes
 * cannot be atomic. If the enqueue then throws, the old code left QUEUED — and
 * a QUEUED run is exactly what a run waiting behind a concurrency cap looks
 * like, so it reads as "starting soon" indefinitely. Seen for real: Redis died,
 * the route 500'd, and the run sat there looking pending forever.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

// Fail the enqueue the way a dead Redis does, without needing a dead Redis.
const enqueue = vi.hoisted(() => ({ fail: false }));
vi.mock("@/lib/queue", () => ({
  enqueueRunWorkflow: async () => {
    if (enqueue.fail) throw new Error("connect ECONNREFUSED 127.0.0.1:6379");
  },
  enqueueCompensateRun: async () => undefined,
}));

const steps: WorkflowSteps = [{ id: "nav", type: "navigate", url: "https://example.com" }];

describe.skipIf(!hasDb)("startRun (Postgres)", () => {
  let orgId: string;
  let versionId: string;
  const slug = `startrun-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "S", slug } });
    orgId = org.id;
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: "Start run",
        versions: { create: { version: 1, steps: steps as never } },
      },
      include: { versions: true },
    });
    versionId = wf.versions[0]!.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
  });

  it("leaves the run QUEUED when the enqueue succeeds", async () => {
    enqueue.fail = false;
    const { startRun } = await import("./start-run.js");
    const res = await startRun({ orgId, workflowVersionId: versionId });

    expect(res.ok).toBe(true);
    const run = await prisma.run.findUniqueOrThrow({ where: { id: res.runId } });
    expect(run.status).toBe("QUEUED");
    expect(run.error).toBeNull();
  });

  it("marks the run FAILED — not QUEUED — when the enqueue throws", async () => {
    enqueue.fail = true;
    const { startRun } = await import("./start-run.js");
    const res = await startRun({ orgId, workflowVersionId: versionId });

    expect(res.ok).toBe(false);

    const run = await prisma.run.findUniqueOrThrow({ where: { id: res.runId } });
    // The assertion that matters: QUEUED here is the bug, because no worker
    // will ever pick it up and nothing in the UI distinguishes it from waiting.
    expect(run.status).toBe("FAILED");
    expect(run.error).toMatch(/could not be queued/i);
    expect(run.endedAt).not.toBeNull();
  });

  it("writes no journal entries for a run that never started", async () => {
    enqueue.fail = true;
    const { startRun } = await import("./start-run.js");
    const res = await startRun({ orgId, workflowVersionId: versionId });

    // A `run.started` here would be a false statement in the hash chain: the
    // run did not start, and the journal is supposed to be the record of what
    // actually happened.
    const events = await prisma.runEvent.count({ where: { runId: res.runId } });
    expect(events).toBe(0);
  });
});
