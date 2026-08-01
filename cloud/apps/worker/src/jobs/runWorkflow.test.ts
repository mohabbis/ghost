import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type Server } from "node:http";
import { readFileSync } from "node:fs";
import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import type { RunWorkflowJob } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { runWorkflowJob } from "./runWorkflow.js";
import { useDiscoveredChromium } from "../browser/chromium.js";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import {
  appendAuditEvent,
  appendRunEvent,
  auditPayloadFromRow,
  runEventPayloadFromRow,
} from "@ghost/core/audit-log";
import { verifyAuditChain } from "@ghost/core/audit";

/**
 * DB-backed integration test for the approval halt/resume loop.
 *
 * Requires DATABASE_URL (docker-compose Postgres or any migrated ghost DB).
 * Serves the hermetic form fixture over HTTP so navigate URLs work like prod.
 * Skips cleanly when DATABASE_URL is unset so unit-only CI stays green.
 */

const hasDb = Boolean(process.env.DATABASE_URL);
const here = dirname(fileURLToPath(import.meta.url));
const fixtureHtml = readFileSync(
  join(here, "../browser/__fixtures__/form.html"),
  "utf8",
);

// Same browser discovery the driver tests use. Without it this suite fails in
// any image where the provisioned Chromium revision differs from the one
// Playwright pins — which is most CI runners and dev containers.
useDiscoveredChromium();

function fakeJob(runId: string, orgId: string, jobId = "test-job"): Job<RunWorkflowJob> {
  return { data: { runId, orgId }, id: jobId } as Job<RunWorkflowJob>;
}

describe.skipIf(!hasDb)("runWorkflowJob (Postgres)", () => {
  let server: Server;
  let fixtureBase: string;
  let artifactDir: string;
  let orgId: string;
  let userId: string;

  beforeAll(async () => {
    artifactDir = mkdtempSync(join(tmpdir(), "ghost-artifacts-"));
    process.env.GHOST_ARTIFACT_DIR = artifactDir;

    server = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(fixtureHtml);
    });
    await new Promise<void>((resolve) => {
      server.listen(0, "127.0.0.1", () => resolve());
    });
    const addr = server.address();
    if (!addr || typeof addr === "string") throw new Error("no listen address");
    fixtureBase = `http://127.0.0.1:${addr.port}`;

    const slug = `it-${Date.now()}`;
    const org = await prisma.organization.create({
      data: { name: "Integration Test Org", slug },
    });
    orgId = org.id;
    const user = await prisma.user.create({
      data: {
        email: `${slug}@example.com`,
        name: "IT",
        memberships: { create: { orgId, role: "OWNER" } },
      },
    });
    userId = user.id;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) => {
      server.close((err) => (err ? reject(err) : resolve()));
    });
    // Cascade cleans org-owned rows.
    await prisma.organization.delete({ where: { id: orgId } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: userId } }).catch(() => undefined);
    rmSync(artifactDir, { recursive: true, force: true });
    await prisma.$disconnect();
  });

  async function seedRun(steps: WorkflowSteps) {
    const workflow = await prisma.workflow.create({
      data: {
        orgId,
        name: "IT demo",
        createdById: userId,
        versions: {
          create: {
            version: 1,
            steps: steps as unknown as Prisma.InputJsonValue,
          },
        },
      },
      include: { versions: true },
    });
    const version = workflow.versions[0]!;
    const run = await prisma.run.create({
      data: {
        orgId,
        workflowVersionId: version.id,
        triggeredById: userId,
        status: "QUEUED",
      },
    });
    return run.id;
  }

  /** How many times the journal records step `index` completing successfully. */
  async function countSucceeded(runId: string, index: number): Promise<number> {
    return prisma.runEvent.count({
      where: { runId, stepIndex: index, type: RUN_EVENT_TYPES.stepSucceeded },
    });
  }

  const demoSteps = (): WorkflowSteps => [
    { id: "nav", type: "navigate", url: `${fixtureBase}/`, label: "Open order form" },
    {
      id: "fill",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "Ada Lovelace",
      sensitive: false,
      label: "Enter customer name",
    },
    {
      id: "submit",
      type: "click",
      selector: { role: "button", name: "Submit order" },
      label: "Submit order",
    },
    {
      id: "verify",
      type: "verify",
      assertion: { kind: "textPresent", expected: "Order submitted" },
      label: "Confirm submission",
    },
  ];

  it("halts at the sensitive click, then succeeds after approval", async () => {
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));

    let run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: true, approvals: true },
    });
    expect(run.status).toBe("AWAITING_APPROVAL");
    expect(run.cursor).toBe(2);
    expect(run.approvals).toHaveLength(1);
    expect(run.approvals[0]!.status).toBe("PENDING");
    expect(run.steps.filter((s) => s.status === "SUCCEEDED")).toHaveLength(2);

    await prisma.approval.update({
      where: { id: run.approvals[0]!.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });

    await runWorkflowJob(fakeJob(runId, orgId));

    run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: { orderBy: { index: "asc" } }, approvals: true },
    });
    expect(run.status).toBe("SUCCEEDED");
    expect(run.steps).toHaveLength(4);
    expect(run.steps.every((s) => s.status === "SUCCEEDED")).toBe(true);
    expect(run.steps[3]!.verification).toMatchObject({ passed: true });
  });

  it("fails the run when the pending approval is rejected", async () => {
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));

    const pending = await prisma.approval.findFirstOrThrow({
      where: { runId, status: "PENDING" },
    });
    await prisma.approval.update({
      where: { id: pending.id },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });
    // Mirror the web approve route's reject path (no re-enqueue).
    await prisma.run.update({
      where: { id: runId },
      data: { status: "FAILED", endedAt: new Date(), error: `rejected at step ${pending.stepIndex}` },
    });

    // A stray re-enqueue must be a no-op once FAILED.
    await runWorkflowJob(fakeJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("FAILED");
    expect(run.error).toMatch(/rejected/);
  });

  // -------------------------------------------------------------------------
  // Durable execution
  // -------------------------------------------------------------------------

  it("does not re-execute completed steps when a stale cursor is redelivered", async () => {
    // The regression this whole slice exists for. Previously `cursor` was a
    // local variable persisted only at gates, so a crash mid-run meant BullMQ
    // redelivered the job, the run restarted from a stale cursor, and — because
    // approvals are re-read from the database — the already-approved "Submit
    // order" click re-planned as `execute` rather than `gate`. The order was
    // submitted twice with no human in the loop.
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId, status: "PENDING" } });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });
    await runWorkflowJob(fakeJob(runId, orgId));

    const finished = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(finished.status).toBe("SUCCEEDED");

    const submitsBefore = await countSucceeded(runId, 2);
    expect(submitsBefore).toBe(1);

    // Simulate the crash: rewind the cursor and status, then redeliver.
    await prisma.run.update({
      where: { id: runId },
      data: { status: "QUEUED", cursor: 0, endedAt: null },
    });
    await runWorkflowJob(fakeJob(runId, orgId, "redelivered"));

    // The sensitive click recorded exactly one success, both times.
    expect(await countSucceeded(runId, 2)).toBe(1);
    const after = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(after.status).toBe("SUCCEEDED");
  });

  it("stops at an interrupted at-most-once step instead of guessing", async () => {
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId, status: "PENDING" } });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });

    // Reproduce the crash window through the real appender, so the journal
    // chain stays valid and the engine follows the indeterminate path rather
    // than the tampered-journal one: the payment click recorded `step.started`
    // and never recorded an outcome.
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepStarted,
      stepIndex: 2,
      payload: { type: "click", attempt: 1 },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });

    await runWorkflowJob(fakeJob(runId, orgId, "after-crash"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    // The run stops and asks a human; it never re-clicks Submit.
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/OUTCOME_UNKNOWN/);
    expect(await countSucceeded(runId, 2)).toBe(0);

    const step = await prisma.runStep.findUnique({
      where: { runId_index: { runId, index: 2 } },
    });
    expect(step?.status).toBe("UNKNOWN");
  });

  it("does not re-run a sensitive step after a crash during its verification retry", async () => {
    // End-to-end form of the regression the journal fold guards. The approved
    // click executes, its verification fails, the engine appends `step.retried`
    // to re-run the assertion, and the worker dies mid-retry. Resumption must
    // treat the step as still in flight — not as never having run — or the
    // order is submitted a second time.
    const runId = await seedRun(demoSteps());

    await runWorkflowJob(fakeJob(runId, orgId));
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId, status: "PENDING" } });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });

    // The click ran; its assertion is being retried when the worker dies.
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepStarted,
      stepIndex: 2,
      payload: { type: "click", attempt: 1 },
    });
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepRetried,
      stepIndex: 2,
      payload: { phase: "verify", attempt: 2 },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });

    await runWorkflowJob(fakeJob(runId, orgId, "after-verify-crash"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/OUTCOME_UNKNOWN/);
    // The decisive assertion: the click was never repeated.
    expect(await countSucceeded(runId, 2)).toBe(0);
  });

  it("refuses to resume on a tampered journal", async () => {
    const runId = await seedRun(demoSteps());
    await runWorkflowJob(fakeJob(runId, orgId));

    // Rewrite a recorded payload without recomputing the chain.
    const first = await prisma.runEvent.findFirstOrThrow({
      where: { runId },
      orderBy: { seq: "asc" },
    });
    await prisma.runEvent.update({
      where: { id: first.id },
      data: { payload: { tampered: true } },
    });
    await prisma.run.update({ where: { id: runId }, data: { status: "QUEUED" } });

    await runWorkflowJob(fakeJob(runId, orgId, "tampered"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("FAILED");
    expect(run.error).toMatch(/JOURNAL_TAMPERED/);
  });

  it("holds the lease so two concurrent jobs cannot both advance a run", async () => {
    const runId = await seedRun(demoSteps());

    await Promise.all([
      runWorkflowJob(fakeJob(runId, orgId, "a")),
      runWorkflowJob(fakeJob(runId, orgId, "b")),
    ]);

    // Exactly one `step.started` per index — no interleaved double execution.
    const started = await prisma.runEvent.findMany({
      where: { runId, type: RUN_EVENT_TYPES.stepStarted },
    });
    const perIndex = new Map<number, number>();
    for (const e of started) perIndex.set(e.stepIndex!, (perIndex.get(e.stepIndex!) ?? 0) + 1);
    for (const [, count] of perIndex) expect(count).toBe(1);
  });

  it("writes a verifiable per-run journal", async () => {
    const runId = await seedRun(demoSteps());
    await runWorkflowJob(fakeJob(runId, orgId));

    const events = await prisma.runEvent.findMany({ where: { runId }, orderBy: { seq: "asc" } });
    expect(events.length).toBeGreaterThan(0);
    expect(events.map((e) => e.seq)).toEqual(events.map((_, i) => i + 1));

    const chain = verifyAuditChain(
      events.map((e) => ({
        prevHash: e.prevHash,
        hash: e.hash,
        payload: runEventPayloadFromRow(e),
      })),
    );
    expect(chain).toEqual({ intact: true, firstBreakIndex: null });
  });

  it("keeps the org audit chain intact under concurrent appends", async () => {
    // The old inline appender read the head then inserted, untransacted and
    // ordered by createdAt, so two writers folded onto the same prevHash and
    // forked the chain. This asserts the advisory lock + seq fix.
    const org = await prisma.organization.create({
      data: { name: "Chain Org", slug: `chain-${Date.now()}` },
    });
    try {
      await Promise.all(
        Array.from({ length: 20 }, (_, i) =>
          appendAuditEvent(org.id, null, {
            action: "test.concurrent",
            entityType: "Run",
            entityId: `e${i}`,
          }),
        ),
      );

      const rows = await prisma.auditEvent.findMany({
        where: { orgId: org.id },
        orderBy: { seq: "asc" },
      });
      expect(rows.map((r) => r.seq)).toEqual(Array.from({ length: 20 }, (_, i) => i + 1));

      const chain = verifyAuditChain(
        rows.map((r) => ({
          prevHash: r.prevHash,
          hash: r.hash,
          payload: auditPayloadFromRow(r),
        })),
      );
      expect(chain).toEqual({ intact: true, firstBreakIndex: null });
    } finally {
      await prisma.organization.delete({ where: { id: org.id } }).catch(() => undefined);
    }
  });

  it("falls back to RunStep rows for runs that predate the journal", async () => {
    // Without this fallback, deploying durable execution would make every
    // in-flight run look like "nothing has executed" and re-run it from zero —
    // triggering the exact double-submit the journal prevents.
    const runId = await seedRun(demoSteps());
    await runWorkflowJob(fakeJob(runId, orgId));
    const approval = await prisma.approval.findFirstOrThrow({ where: { runId, status: "PENDING" } });
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: userId, resolvedAt: new Date() },
    });

    // Strip the journal, leaving only the legacy RunStep rows — exactly what a
    // run that was mid-flight when durable execution shipped looks like.
    await prisma.runEvent.deleteMany({ where: { runId } });
    await prisma.run.update({
      where: { id: runId },
      data: { status: "QUEUED", cursor: 0, sessionKey: null, sessionUrl: null },
    });
    const before = await prisma.runStep.findMany({
      where: { runId, index: { in: [0, 1] } },
      orderBy: { index: "asc" },
    });

    await runWorkflowJob(fakeJob(runId, orgId, "legacy"));

    // The legacy RunStep rows tell the engine steps 0-1 already ran, so it does
    // not touch them — their timestamps are untouched and no new journal entry
    // claims they succeeded again.
    const after = await prisma.runStep.findMany({
      where: { runId, index: { in: [0, 1] } },
      orderBy: { index: "asc" },
    });
    expect(after.map((s) => s.startedAt?.toISOString())).toEqual(
      before.map((s) => s.startedAt?.toISOString()),
    );
    expect(await countSucceeded(runId, 0)).toBe(0);
    expect(await countSucceeded(runId, 1)).toBe(0);

    // And because there is no recorded page state for those steps, the engine
    // refuses to rebuild the browser by guessing. It stops for a human rather
    // than risking a repeat of the already-approved submit.
    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.error).toMatch(/RESTORE_UNSAFE/);
    expect(await countSucceeded(runId, 2)).toBe(0);
  });

  // -------------------------------------------------------------------------
  // Cancel, incidents, and the variable store
  // -------------------------------------------------------------------------

  it("stops between steps when the run is canceled", async () => {
    const runId = await seedRun(demoSteps());
    await prisma.run.update({ where: { id: runId }, data: { status: "CANCELED" } });

    await runWorkflowJob(fakeJob(runId, orgId));

    const run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: true },
    });
    expect(run.status).toBe("CANCELED");
    expect(run.steps).toHaveLength(0);
  });

  it("raises an incident on a failed step instead of killing the run", async () => {
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${fixtureBase}/` },
      // A selector that will never resolve, with a short timeout so the test
      // does not wait on Playwright's 30s default.
      {
        id: "ghost",
        type: "fill",
        selector: { css: "#does-not-exist" },
        value: "x",
        sensitive: false,
        timeoutMs: 1_000,
      },
    ]);

    await runWorkflowJob(fakeJob(runId, orgId, "incident"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("INCIDENT");
    expect(run.cursor).toBe(1);
  });

  it("retries a benign step according to its policy before giving up", async () => {
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${fixtureBase}/` },
      {
        id: "ghost",
        type: "fill",
        selector: { css: "#does-not-exist" },
        value: "x",
        sensitive: false,
        timeoutMs: 500,
        retry: { maxAttempts: 3, backoffMs: 10, factor: 1 },
      },
    ]);

    await runWorkflowJob(fakeJob(runId, orgId, "retries"));

    const attempts = await prisma.runEvent.count({
      where: { runId, stepIndex: 1, type: RUN_EVENT_TYPES.stepStarted },
    });
    expect(attempts).toBe(3);
    const step = await prisma.runStep.findUniqueOrThrow({
      where: { runId_index: { runId, index: 1 } },
    });
    expect(step.attempt).toBe(3);
  });

  it("never retries a sensitive step, whatever the definition asks for", async () => {
    // The clamp must hold at execution time: a stored policy cannot talk the
    // engine into clicking "Submit order" twice.
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${fixtureBase}/` },
      {
        id: "pay",
        type: "click",
        selector: { role: "button", name: "Submit order — missing" },
        timeoutMs: 500,
        retry: { maxAttempts: 5, backoffMs: 10, factor: 1 },
      },
    ]);

    await prisma.approval.create({
      data: { runId, stepIndex: 1, reason: "test", status: "APPROVED", resolvedById: userId },
    });
    await runWorkflowJob(fakeJob(runId, orgId, "no-retry"));

    const attempts = await prisma.runEvent.count({
      where: { runId, stepIndex: 1, type: RUN_EVENT_TYPES.stepStarted },
    });
    expect(attempts).toBe(1);
  });

  it("feeds an extracted value into a later step", async () => {
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${fixtureBase}/` },
      { id: "who", type: "extract", selector: { css: "h1" }, name: "heading" },
      {
        id: "echo",
        type: "fill",
        selector: { role: "textbox", name: "Full name" },
        value: "{{ steps.who.heading }}",
        sensitive: false,
      },
      {
        id: "check",
        type: "verify",
        assertion: { kind: "textPresent", expected: "Order form" },
      },
    ]);

    await runWorkflowJob(fakeJob(runId, orgId, "expr"));

    const run = await prisma.run.findUniqueOrThrow({
      where: { id: runId },
      include: { steps: { orderBy: { index: "asc" } } },
    });
    expect(run.status).toBe("SUCCEEDED");
    expect(run.steps[1]!.output).toEqual({ heading: "Order form" });
  });

  it("fails a run whose expression cannot be resolved, rather than filling blank", async () => {
    const runId = await seedRun([
      { id: "nav", type: "navigate", url: `${fixtureBase}/` },
      {
        id: "echo",
        type: "fill",
        selector: { role: "textbox", name: "Full name" },
        value: "{{ steps.nope.missing }}",
        sensitive: false,
      },
    ]);

    await runWorkflowJob(fakeJob(runId, orgId, "bad-expr"));

    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    expect(run.status).toBe("FAILED");
    expect(run.error).toMatch(/nope/);
  });
});
