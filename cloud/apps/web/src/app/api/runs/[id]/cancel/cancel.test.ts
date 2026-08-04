import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { prisma } from "@ghost/core/db";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Lease-aware cancellation.
 *
 * The route used to always append `run.canceled` and anchor the journal head
 * itself, regardless of whether a worker currently held the run's lease. If a
 * step was mid-execution, that worker would still append `step.succeeded`
 * *after* the anchor was taken — leaving the run's true final head unmatched
 * by what got sealed into the org chain, which `/api/audit/verify` then
 * reports as tampering on a run nothing actually touched.
 *
 * These tests pin the fix: the route only finalizes (appends + anchors) when
 * no worker currently owns the lease. When a lease is active, it flips status
 * only and defers to the worker's own loop-top CANCELED check — and delays its
 * cleanup re-enqueue past the lease TTL instead of racing it immediately.
 *
 * Requires DATABASE_URL; skips cleanly without one.
 */

const hasDb = Boolean(process.env.DATABASE_URL);

const enqueueRunWorkflow = vi.hoisted(() =>
  vi.fn(async (_data: unknown, _opts?: { delayMs?: number }) => undefined),
);
vi.mock("@/lib/queue", () => ({
  enqueueRunWorkflow,
  enqueueCompensateRun: async () => undefined,
}));

const session = vi.hoisted(() => ({
  current: null as null | { user: { id: string; orgId: string; email?: string } },
}));
vi.mock("@/auth", () => ({ auth: async () => session.current }));

const steps: WorkflowSteps = [
  { id: "nav", type: "navigate", url: "https://example.com" },
  { id: "submit", type: "click", selector: { role: "button", name: "Submit order" } },
];

describe.skipIf(!hasDb)("run cancellation is lease-aware (Postgres)", () => {
  let orgId: string;
  let userId: string;
  const slug = `cancel-${Date.now()}`;

  beforeAll(async () => {
    const org = await prisma.organization.create({ data: { name: "Cancel", slug } });
    orgId = org.id;
    const user = await prisma.user.create({
      data: { email: `${slug}@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
  });

  beforeEach(() => {
    session.current = { user: { id: userId, orgId } };
    enqueueRunWorkflow.mockClear();
  });

  /** A queued run, optionally leased as if a worker currently owns it. */
  async function runIn(status: "QUEUED" | "RUNNING", lease?: { expiresInMs: number }) {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: `wf-${Math.random().toString(36).slice(2, 8)}`,
        versions: { create: { version: 1, steps: steps as never } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: wf.versions[0]!.id,
        triggeredById: userId,
        status,
        ...(lease
          ? {
              leaseOwner: "test-worker:1",
              leaseExpiresAt: new Date(Date.now() + lease.expiresInMs),
            }
          : {}),
      },
    });
    return run.id;
  }

  async function cancel(runId: string) {
    const { POST } = await import("./route.js");
    return POST(new Request(`http://localhost/api/runs/${runId}/cancel`, { method: "POST" }), {
      params: Promise.resolve({ id: runId }),
    });
  }

  it("finalizes immediately when no worker holds the lease", async () => {
    const runId = await runIn("QUEUED");

    const res = await cancel(runId);
    expect(res.status).toBe(200);

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("CANCELED");

    const canceledEvent = await prisma.runEvent.findFirst({
      where: { runId, type: RUN_EVENT_TYPES.runCanceled },
    });
    expect(canceledEvent).not.toBeNull();

    const anchor = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.canceled" },
    });
    expect(anchor).not.toBeNull();
    expect((anchor?.metadata as { runChainHead?: string } | null)?.runChainHead).toBe(
      canceledEvent!.hash,
    );

    // Nothing is actively executing, so the cleanup job can run right away.
    expect(enqueueRunWorkflow).toHaveBeenCalledTimes(1);
    expect(enqueueRunWorkflow.mock.calls[0]![1]).toBeUndefined();
  });

  it("defers finalizing when a worker currently holds the lease", async () => {
    const runId = await runIn("RUNNING", { expiresInMs: 60_000 });

    const res = await cancel(runId);
    expect(res.status).toBe(200);

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    // Status flips immediately so the worker's own loop-top check sees it...
    expect(run.status).toBe("CANCELED");

    // ...but this route must not have appended or anchored anything: the
    // worker may still be mid-step, and finalizing here would race it exactly
    // the way the bug this test pins used to.
    const canceledEvent = await prisma.runEvent.findFirst({
      where: { runId, type: RUN_EVENT_TYPES.runCanceled },
    });
    expect(canceledEvent).toBeNull();

    const anchor = await prisma.auditEvent.findFirst({
      where: { orgId, entityId: runId, action: "run.canceled" },
    });
    expect(anchor).toBeNull();

    // The cleanup job is still enqueued, but delayed past the lease TTL rather
    // than racing the still-possibly-active worker.
    expect(enqueueRunWorkflow).toHaveBeenCalledTimes(1);
    expect(enqueueRunWorkflow.mock.calls[0]![1]).toEqual({ delayMs: 60_000 });
  });

  it("finalizes immediately once the held lease has already expired", async () => {
    // An expired lease means whoever held it is gone (crashed, or its
    // heartbeat stopped) — nothing is actively executing, so this is the same
    // as the no-lease case.
    const runId = await runIn("RUNNING", { expiresInMs: -1_000 });

    await cancel(runId);

    const canceledEvent = await prisma.runEvent.findFirst({
      where: { runId, type: RUN_EVENT_TYPES.runCanceled },
    });
    expect(canceledEvent).not.toBeNull();
    expect(enqueueRunWorkflow.mock.calls[0]![1]).toBeUndefined();
  });

  it("is idempotent: canceling an already-canceled run refuses rather than double-finalizing", async () => {
    const runId = await runIn("QUEUED");
    await cancel(runId);

    const res = await cancel(runId);
    expect(res.status).toBe(409);

    const events = await prisma.runEvent.findMany({
      where: { runId, type: RUN_EVENT_TYPES.runCanceled },
    });
    expect(events).toHaveLength(1);
  });
});
