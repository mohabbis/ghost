import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type Server } from "node:http";
import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import type { CompensateRunJob, RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { appendRunEvent } from "@ghost/core/audit-log";
import { runWorkflowJob } from "./runWorkflow.js";
import { compensateRunJob } from "./compensateRun.js";
import { useDiscoveredChromium } from "../browser/chromium.js";

/**
 * DB-backed test for compensation — reversing a run's completed side effects.
 *
 * The assertions that matter are about honesty rather than mechanics: that a
 * reversal is gated like the action it undoes, that an effect with no defined
 * reversal is reported rather than quietly skipped, and that a run which was
 * only partly reversed does not read as cleanly undone.
 */

const hasDb = Boolean(process.env.DATABASE_URL);
const here = dirname(fileURLToPath(import.meta.url));
const fixtureHtml = readFileSync(join(here, "../browser/__fixtures__/form.html"), "utf8");

useDiscoveredChromium();

function runJob(runId: string, orgId: string, jobId = "t"): Job<RunWorkflowJob> {
  return { data: { runId, orgId }, id: jobId } as Job<RunWorkflowJob>;
}
function compJob(runId: string, orgId: string): Job<CompensateRunJob> {
  return { data: { runId, orgId }, id: `c-${runId}` } as Job<CompensateRunJob>;
}

describe.skipIf(!hasDb)("compensateRunJob (Postgres)", () => {
  let server: Server;
  let base: string;
  let artifactDir: string;
  let orgId: string;
  let userId: string;

  beforeAll(async () => {
    artifactDir = mkdtempSync(join(tmpdir(), "ghost-comp-"));
    process.env.GHOST_ARTIFACT_DIR = artifactDir;

    server = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(fixtureHtml);
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()));
    const addr = server.address();
    if (!addr || typeof addr === "string") throw new Error("no listen address");
    base = `http://127.0.0.1:${addr.port}`;

    const slug = `comp-${Date.now()}`;
    const org = await prisma.organization.create({ data: { name: "Comp Org", slug } });
    orgId = org.id;
    const user = await prisma.user.create({
      data: {
        email: `${slug}@example.com`,
        memberships: { create: { orgId, role: "OWNER" } },
      },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) =>
      server.close((err) => (err ? reject(err) : resolve())),
    );
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
    rmSync(artifactDir, { recursive: true, force: true });
    await prisma.$disconnect();
  });

  async function seedRun(steps: WorkflowSteps) {
    const wf = await prisma.workflow.create({
      data: {
        orgId,
        name: "comp demo",
        createdById: userId,
        versions: { create: { version: 1, steps: steps as unknown as Prisma.InputJsonValue } },
      },
      include: { versions: true },
    });
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: wf.versions[0]!.id,
        triggeredById: userId,
        status: "QUEUED",
      },
    });
    return run.id;
  }

  /** Drive a run to completion through its single approval gate. */
  async function runToCompletion(runId: string) {
    await runWorkflowJob(runJob(runId, orgId, `f-${runId}`));
    const approval = await prisma.approval.findFirst({
      where: { runId, status: "PENDING", phase: "FORWARD" },
    });
    if (approval) {
      await prisma.approval.update({
        where: { id: approval.id },
        data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
      });
      await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });
      await runWorkflowJob(runJob(runId, orgId, `f2-${runId}`));
    }
  }

  const reversibleSteps = (): WorkflowSteps => [
    { id: "nav", type: "navigate", url: `${base}/`, label: "Open order form" },
    {
      id: "submit",
      type: "click",
      selector: { role: "button", name: "Submit order" },
      label: "Submit order",
      compensate: {
        description: "Open the order and click Cancel order",
        actions: [
          { type: "navigate", url: `${base}/` },
          { type: "click", selector: { role: "button", name: "Cancel order" } },
        ],
      },
    },
  ];

  it("reverses a completed run once the reversal is approved", async () => {
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    expect((await prisma.run.findUniqueOrThrow({ where: { id: runId } })).status).toBe("SUCCEEDED");

    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    // Undoing a submitted order is a mutating action, so it gates first.
    await compensateRunJob(compJob(runId, orgId));
    let run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { approvals: true },
    });
    expect(run.status).toBe("AWAITING_APPROVAL");
    const gate = run.approvals.find((a) => a.phase === "COMPENSATION");
    expect(gate?.status).toBe("PENDING");
    expect(gate?.reason).toMatch(/cancel order/i);

    await prisma.approval.update({
      where: { id: gate!.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    await compensateRunJob(compJob(runId, orgId));

    run = await prisma.run.findUniqueOrThrow({ where: { id: runId }, include: { approvals: true } });
    expect(run.status).toBe("COMPENSATED");
    // Fully reversed, so no caveat on the run row.
    expect(run.error).toBeNull();

    const compensated = await prisma.runEvent.count({
      where: { runId, type: RUN_EVENT_TYPES.stepCompensated },
    });
    expect(compensated).toBe(1);
  });

  it("does not touch anything before the reversal is approved", async () => {
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    await compensateRunJob(compJob(runId, orgId));

    // Gate opened, nothing reversed.
    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.stepCompensated } }),
    ).toBe(0);
  });

  it("stops without reversing when the reversal is rejected", async () => {
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    const gate = await prisma.approval.findFirstOrThrow({
      where: { runId, phase: "COMPENSATION" },
    });
    await prisma.approval.update({
      where: { id: gate.id },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    await compensateRunJob(compJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.stepCompensated } }),
    ).toBe(0);
  });

  it("reports an effect it cannot reverse rather than claiming a clean undo", async () => {
    // The honesty assertion. A run whose submit has no defined reversal must not
    // end up looking cleanly undone — the operator needs to know the order is
    // still out there.
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${base}/`, label: "Open order form" },
      {
        id: "submit",
        type: "click",
        selector: { role: "button", name: "Submit order" },
        label: "Submit order",
        // no `compensate`
      },
    ]);
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    await compensateRunJob(compJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("COMPENSATED");
    // ...but it says so on the run row rather than reading as fully undone.
    expect(run.error).toMatch(/no defined reversal/i);

    const irreversible = await prisma.runEvent.findMany({
      where: { runId, type: RUN_EVENT_TYPES.stepIrreversible },
    });
    expect(irreversible).toHaveLength(1);
    expect(irreversible[0]!.stepIndex).toBe(1);
  });

  it("is re-entrant: a redelivered job does not reverse twice", async () => {
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    const gate = await prisma.approval.findFirstOrThrow({
      where: { runId, phase: "COMPENSATION" },
    });
    await prisma.approval.update({
      where: { id: gate.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    // Re-drive it. Reversing a reversal would be its own kind of double-action.
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.stepCompensated } }),
    ).toBe(1);
  });

  it("keeps the forward step's screenshot when it stores the reversal's", async () => {
    // The reversal image used to be written over the forward step's key,
    // destroying the record of what originally happened — the very thing the
    // reversal exists to account for.
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    const forward = await prisma.runStep.findFirstOrThrow({ where: { runId, index: 1 } });
    const forwardKey = forward.screenshotKey!;
    const forwardBytes = readFileSync(join(artifactDir, forwardKey));

    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));
    const gate = await prisma.approval.findFirstOrThrow({ where: { runId, phase: "COMPENSATION" } });
    await prisma.approval.update({
      where: { id: gate.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    // Forward evidence untouched...
    expect(readFileSync(join(artifactDir, forwardKey))).toEqual(forwardBytes);
    // ...and the reversal stored its own, under its own key.
    const compensated = await prisma.runEvent.findFirstOrThrow({
      where: { runId, type: RUN_EVENT_TYPES.stepCompensated },
    });
    const undoKey = (compensated.payload as { screenshotKey: string }).screenshotKey;
    expect(undoKey).not.toBe(forwardKey);
    expect(undoKey).toMatch(/undo-1\.png$/);
  });

  it("records the start once, even when the first reversal gates", async () => {
    // The resumed job used "no reversal has completed yet" as its proxy for
    // "not started", so a plan that gates first announced the start twice and
    // repeated every irreversible entry — inflating a supposedly factual record.
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    const gate = await prisma.approval.findFirstOrThrow({ where: { runId, phase: "COMPENSATION" } });
    await prisma.approval.update({
      where: { id: gate.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    expect(
      await prisma.runEvent.count({
        where: { runId, type: RUN_EVENT_TYPES.compensationStarted },
      }),
    ).toBe(1);
  });

  it("stops rather than reversing twice when a reversal started and never finished", async () => {
    // Simulates a worker killed after the cancellation reached the target but
    // before the terminal event committed. Re-running would cancel the order a
    // second time, so the run stops for a human instead.
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));
    const gate = await prisma.approval.findFirstOrThrow({ where: { runId, phase: "COMPENSATION" } });
    await prisma.approval.update({
      where: { id: gate.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });

    // A start with no outcome, exactly as a crash mid-reversal would leave it.
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepCompensationStarted,
      stepIndex: 1,
      payload: { stepId: "submit" },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
    await compensateRunJob(compJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/never reported an outcome/);
    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.stepCompensated } }),
    ).toBe(0);
  });

  it("refuses to reverse a run whose journal does not verify", async () => {
    // The plan is derived from the journal, so a single altered row could
    // invent a reversal for an action that never happened.
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);
    const event = await prisma.runEvent.findFirstOrThrow({
      where: { runId, type: RUN_EVENT_TYPES.stepSucceeded },
    });
    await prisma.runEvent.update({
      where: { id: event.id },
      data: { payload: { tampered: true } },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });

    await compensateRunJob(compJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/JOURNAL_TAMPERED/);
    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.compensationStarted } }),
    ).toBe(0);
  });

  it("attributes a reversal to its approver, not to whoever started the run", async () => {
    // The run is triggered by one person and the undo approved by another. An
    // audit trail that names the wrong person for a mutation is worse than none.
    const other = await prisma.user.create({
      data: {
        email: `approver-${Date.now()}@example.com`,
        memberships: { create: { orgId, role: "ADMIN" } },
      },
    });
    try {
      const runId = await seedRun(reversibleSteps());
      await runToCompletion(runId);
      await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
      await compensateRunJob(compJob(runId, orgId));

      const gate = await prisma.approval.findFirstOrThrow({
        where: { runId, phase: "COMPENSATION" },
      });
      await prisma.approval.update({
        where: { id: gate.id },
        data: { status: "APPROVED", resolvedById: other.id, resolvedAt: new Date() },
      });
      await prisma.run.update({ where: { id: runId }, data: { status: "COMPENSATING" } });
      await compensateRunJob(compJob(runId, orgId));

      const audit = await prisma.auditEvent.findFirstOrThrow({
        where: { orgId, action: "step.compensated", entityId: `${runId}:1` },
      });
      expect(audit.actorId).toBe(other.id);
      expect(audit.actorId).not.toBe(userId);
    } finally {
      await prisma.user.delete({ where: { id: other.id } }).catch(() => undefined);
    }
  });

  it("ignores a run that is not being compensated", async () => {
    const runId = await seedRun(reversibleSteps());
    await runToCompletion(runId);

    await compensateRunJob(compJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("SUCCEEDED");
    expect(
      await prisma.runEvent.count({ where: { runId, type: RUN_EVENT_TYPES.compensationStarted } }),
    ).toBe(0);
  });
});
