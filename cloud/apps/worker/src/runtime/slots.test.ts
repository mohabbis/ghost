import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type Server } from "node:http";
import { Queue, type Job } from "bullmq";
import IORedis from "ioredis";
import { prisma, Prisma } from "@ghost/core/db";
import { QUEUE_NAMES, type RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { runWorkflowJob } from "../jobs/runWorkflow.js";
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

function job(runId: string, orgId: string, id: string): Job<RunWorkflowJob> {
  return { data: { runId, orgId }, id } as Job<RunWorkflowJob>;
}

/** Separate connection for the assertions, so closing it cannot disturb the worker's. */
function probeConnection(): IORedis {
  return new IORedis(process.env.REDIS_URL!, { maxRetriesPerRequest: null });
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

  it("does not hand the slot to itself when it is refused one", async () => {
    // The spin this rules out: a throttled run is still QUEUED, and is usually
    // the *oldest* QUEUED run of its workflow — so releasing a slot on its
    // behalf nominates itself and re-enqueues it with no delay. That job is
    // throttled again, releases again, and the loop runs as fast as Postgres
    // and Redis will answer for as long as the cap stays full.
    //
    // Asserted on the queue rather than on the database, because the damage is
    // an immediate self-directed job, not a wrong row.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();
    await runWorkflowJob(job(first, orgId, "z1"));

    const q = new Queue(QUEUE_NAMES.runWorkflow, { connection: probeConnection() });
    try {
      await q.drain(true);
      await runWorkflowJob(job(second, orgId, "z2"));

      const jobs = await q.getJobs(["waiting", "delayed", "prioritized", "active"]);
      const mine = jobs.filter((j) => j.data.runId === second);
      // Exactly one job: the slow safety-net re-check. Not a second, immediate
      // one nominating itself.
      expect(mine).toHaveLength(1);
      expect(mine[0]!.opts.delay ?? 0).toBeGreaterThan(60_000);
    } finally {
      await q.drain(true).catch(() => undefined);
      await q.close();
    }
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

  it("keeps the slot while a cancelled run drains", async () => {
    // The cancel route marks the run CANCELED immediately, but the worker
    // deliberately lets the current browser action finish. Reading ownership
    // off the status would free the slot during that drain and admit a second
    // run alongside one still mutating the customer's system.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    await queue();
    await runWorkflowJob(job(first, orgId, "k1"));

    await prisma.run.update({ where: { id: first }, data: { status: "CANCELED" } });

    const held = await prisma.run.findUniqueOrThrow({ where: { id: first } });
    expect(held.slotHeldAt).not.toBeNull(); // still owns it, status notwithstanding
  });

  it("keeps the slot across the QUEUED gap after an approval", async () => {
    // The approval route sets an approved run back to QUEUED before its resume
    // job is enqueued. If the slot were derived from status it would look free
    // in that gap, another run would take it, and the approved mid-flow run
    // would be throttled behind it.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const first = await queue();
    const second = await queue();
    await runWorkflowJob(job(first, orgId, "m1"));

    const approval = await prisma.approval.findFirstOrThrow({
      where: { runId: first, status: "PENDING" },
    });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    // Exactly what the approval route does.
    await prisma.run.update({ where: { id: first }, data: { status: "QUEUED" } });

    // The other run must not be able to slip in.
    await runWorkflowJob(job(second, orgId, "m2"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: second } })).status).toBe("QUEUED");
    expect(
      await prisma.runEvent.count({ where: { runId: second, type: RUN_EVENT_TYPES.runThrottled } }),
    ).toBe(1);
  });

  it("ignores a stale re-check once the run has moved on", async () => {
    // A throttled run leaves a five-minute re-check in Redis. If it is handed a
    // slot in the meantime and reaches an approval gate, that stale job would
    // otherwise set it back to RUNNING and append another `run.resumed` and
    // another `gate.opened` — an approval prompt nobody asked for.
    const queue = await seedWorkflow(gatedSteps(), 1);
    const only = await queue();
    await runWorkflowJob(job(only, orgId, "n1"));
    expect((await prisma.run.findUniqueOrThrow({ where: { id: only } })).status).toBe(
      "AWAITING_APPROVAL",
    );

    const before = await prisma.runEvent.count({
      where: { runId: only, type: RUN_EVENT_TYPES.gateOpened },
    });

    const stale = { data: { runId: only, orgId, throttleRecheck: true }, id: "n2" } as Job<
      RunWorkflowJob
    >;
    await runWorkflowJob(stale);

    expect(
      await prisma.runEvent.count({ where: { runId: only, type: RUN_EVENT_TYPES.gateOpened } }),
    ).toBe(before);
    expect((await prisma.run.findUniqueOrThrow({ where: { id: only } })).status).toBe(
      "AWAITING_APPROVAL",
    );
  });

  it("fills every free slot when several holders finish at once", async () => {
    // Waking exactly one run per release leaves capacity idle: two releases
    // both pick the same oldest waiting run, so one slot sits unused until a
    // five-minute re-check notices.
    const queue = await seedWorkflow(plainSteps(), 2);
    const a = await queue();
    const b = await queue();
    const c = await queue();
    const d = await queue();

    await runWorkflowJob(job(a, orgId, "p1"));
    await runWorkflowJob(job(b, orgId, "p2"));
    // Both finished, so both slots are free and both waiting runs should have
    // been woken — not the same one twice.
    for (const id of [c, d]) {
      await runWorkflowJob(job(id, orgId, `p-${id.slice(-4)}`));
      expect((await prisma.run.findUniqueOrThrow({ where: { id } })).status).toBe("SUCCEEDED");
    }
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
