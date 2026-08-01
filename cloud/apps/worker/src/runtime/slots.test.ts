import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type Server } from "node:http";
import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import type { RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { runWorkflowJob } from "../jobs/runWorkflow.js";
import { useDiscoveredChromium } from "../browser/chromium.js";
import { closeQueue } from "../queue.js";

/**
 * DB-backed test for the per-workflow concurrency cap.
 *
 * The assertions are about what the cap actually promises: a run over the cap
 * does not touch anything, it says why it is waiting, and it starts on its own
 * once a slot frees. Needs Redis as well as Postgres, because a throttled run
 * schedules its own re-check.
 */

const hasDb = Boolean(process.env.DATABASE_URL);
const hasRedis = Boolean(process.env.REDIS_URL);
const here = dirname(fileURLToPath(import.meta.url));
const fixtureHtml = readFileSync(join(here, "../browser/__fixtures__/form.html"), "utf8");

useDiscoveredChromium();

function job(runId: string, orgId: string, id: string): Job<RunWorkflowJob> {
  return { data: { runId, orgId }, id } as Job<RunWorkflowJob>;
}

describe.skipIf(!hasDb || !hasRedis)("concurrency cap (Postgres + Redis)", () => {
  let server: Server;
  let base: string;
  let artifactDir: string;
  let orgId: string;
  let userId: string;

  beforeAll(async () => {
    artifactDir = mkdtempSync(join(tmpdir(), "ghost-slots-"));
    process.env.GHOST_ARTIFACT_DIR = artifactDir;

    server = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(fixtureHtml);
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()));
    const addr = server.address();
    if (!addr || typeof addr === "string") throw new Error("no listen address");
    base = `http://127.0.0.1:${addr.port}`;

    const slug = `slots-${Date.now()}`;
    const org = await prisma.organization.create({ data: { name: "Slots Org", slug } });
    orgId = org.id;
    const user = await prisma.user.create({
      data: { email: `${slug}@example.com`, memberships: { create: { orgId, role: "OWNER" } } },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) =>
      server.close((err) => (err ? reject(err) : resolve())),
    );
    await closeQueue();
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
    rmSync(artifactDir, { recursive: true, force: true });
    await prisma.$disconnect();
  });

  /** A workflow with a cap, and a helper to queue runs of it. */
  async function seedWorkflow(steps: WorkflowSteps, cap: number | null) {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: "capped",
        createdById: userId,
        maxActiveRuns: cap,
        versions: { create: { version: 1, steps: steps as unknown as Prisma.InputJsonValue } },
      },
      include: { versions: true },
    });
    const versionId = wf.versions[0]!.id;
    return async () => {
      const run = await prisma.run.create({
        data: { orgId, workflowVersionId: versionId, triggeredById: userId, status: "QUEUED" },
      });
      return run.id;
    };
  }

  /** Two steps, the second of which halts for approval. */
  const gatedSteps = (): WorkflowSteps => [
    { id: "nav", type: "navigate", url: `${base}/`, label: "Open order form" },
    {
      id: "submit",
      type: "click",
      selector: { role: "button", name: "Submit order" },
      label: "Submit order",
    },
  ];

  const plainSteps = (): WorkflowSteps => [
    { id: "nav", type: "navigate", url: `${base}/`, label: "Open order form" },
  ];

  it("holds a run over the cap in QUEUED without executing anything", async () => {
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();

    await runWorkflowJob(job(first, orgId, "a1"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: first } })).status).toBe(
      "AWAITING_APPROVAL",
    );

    await runWorkflowJob(job(second, orgId, "a2"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: second } });
    expect(run.status).toBe("QUEUED");
    expect(run.startedAt).toBeNull();
    // Nothing ran: no RunStep rows, so the browser never opened the form.
    expect(await prisma.runStep.count({ where: { runId: second } })).toBe(0);
  });

  it("says why it is waiting, and names what is holding the slot", async () => {
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();
    await runWorkflowJob(job(first, orgId, "b1"));
    await runWorkflowJob(job(second, orgId, "b2"));

    const event = await prisma.runEvent.findFirstOrThrow({
      where: { runId: second, type: RUN_EVENT_TYPES.runThrottled },
    });
    const payload = event.payload as { cap: number; activeRunIds: string[]; reason: string };
    expect(payload.cap).toBe(1);
    expect(payload.activeRunIds).toEqual([first]);
    expect(payload.reason).toMatch(/allows 1 active run/);
  });

  it("records the wait once, not once per re-check", async () => {
    // A run held for a day would otherwise bury its real events under hundreds
    // of identical throttle entries — and each one is a hash-chain append.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();
    await runWorkflowJob(job(first, orgId, "c1"));

    await runWorkflowJob(job(second, orgId, "c2"));
    await runWorkflowJob(job(second, orgId, "c3"));
    await runWorkflowJob(job(second, orgId, "c4"));

    expect(
      await prisma.runEvent.count({
        where: { runId: second, type: RUN_EVENT_TYPES.runThrottled },
      }),
    ).toBe(1);
  });

  it("lets the waiting run through once the slot frees", async () => {
    const queue = await seedWorkflow(plainSteps(), 1);
    const first = await queue();
    const second = await queue();

    await runWorkflowJob(job(first, orgId, "d1"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: first } })).status).toBe("SUCCEEDED");

    await runWorkflowJob(job(second, orgId, "d2"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: second } })).status).toBe(
      "SUCCEEDED",
    );
  });

  it("keeps the slot while a run sits at its approval gate", async () => {
    // The trade-off written down in concurrency.ts: an approval holds its slot,
    // so a later run cannot overtake one parked mid-flow.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();
    await runWorkflowJob(job(first, orgId, "e1"));
    await runWorkflowJob(job(second, orgId, "e2"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: second } })).status).toBe("QUEUED");

    // Reject the gate, ending the first run. Its slot frees.
    const approval = await prisma.approval.findFirstOrThrow({
      where: { runId: first, status: "PENDING" },
    });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });
    await runWorkflowJob(job(first, orgId, "e3"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: first } })).status).toBe("FAILED");

    await runWorkflowJob(job(second, orgId, "e4"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: second } })).status).toBe(
      "AWAITING_APPROVAL",
    );
  });

  it("does not let an incident hold a slot forever", async () => {
    // One broken run must not wedge its workflow. Parked as an incident, it
    // releases the slot; it re-enters admission when a human retries it.
    const queue = await seedWorkflow(plainSteps(), 1);
    const stuck = await queue();
    const next = await queue();

    await runWorkflowJob(job(stuck, orgId, "f1"));
    await prisma.run.update({ where: { id: stuck }, data: { status: "INCIDENT" } });

    await runWorkflowJob(job(next, orgId, "f2"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: next } })).status).toBe("SUCCEEDED");
  });

  it("runs everything at once when no cap is set", async () => {
    const queue = await seedWorkflow(gatedSteps(), null);
    const first = await queue();
    const second = await queue();

    await runWorkflowJob(job(first, orgId, "g1"));
    await runWorkflowJob(job(second, orgId, "g2"));

    for (const id of [first, second]) {
      expect((await prisma.run.findUniqueOrThrow({ where: { id } })).status).toBe(
        "AWAITING_APPROVAL",
      );
    }
    expect(
      await prisma.runEvent.count({ where: { type: RUN_EVENT_TYPES.runThrottled, runId: second } }),
    ).toBe(0);
  });

  it("resumes a run that already holds its slot, even at a cap of one", async () => {
    // The self-deadlock this rules out: the run's own AWAITING_APPROVAL row is
    // the slot it would otherwise be queuing behind.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const only = await queue();
    await runWorkflowJob(job(only, orgId, "h1"));

    const approval = await prisma.approval.findFirstOrThrow({
      where: { runId: only, status: "PENDING" },
    });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });

    await runWorkflowJob(job(only, orgId, "h2"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: only } })).status).toBe("SUCCEEDED");
  });
});
