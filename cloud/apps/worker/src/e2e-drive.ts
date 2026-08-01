/**
 * Manual end-to-end driver (not a test): seeds a run against the web app's
 * `/fixtures/order` page, enqueues it through BullMQ, and reports what the
 * worker did. Used to verify the queue path and worker process, which the
 * DB-backed vitest suite bypasses by calling the job function directly.
 *
 * Run with: pnpm --filter @ghost/worker exec tsx src/e2e-drive.ts
 */
import { Queue } from "bullmq";
import IORedis from "ioredis";
import { prisma, Prisma } from "@ghost/core/db";
import { QUEUE_NAMES, runWorkflowJobId } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";

const APP = process.env.APP_URL ?? "http://localhost:3000";
const connection = new IORedis(process.env.REDIS_URL!, { maxRetriesPerRequest: null });
const queue = new Queue(QUEUE_NAMES.runWorkflow, { connection });

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function waitFor(runId: string, pred: (s: string) => boolean, label: string, ms = 45_000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    if (pred(run.status)) return run;
    await sleep(500);
  }
  throw new Error(`timed out waiting for ${label}`);
}

async function main() {
  const slug = `e2e-${Date.now()}`;
  const org = await prisma.organization.create({ data: { name: "E2E", slug } });
  const user = await prisma.user.create({
    data: { email: `${slug}@example.com`, memberships: { create: { orgId: org.id, role: "OWNER" } } },
  });

  const steps: WorkflowSteps = [
    { id: "nav", type: "navigate", url: `${APP}/fixtures/order`, label: "Open order form" },
    {
      id: "name",
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
  ];

  const wf = await prisma.workflow.create({
    data: {
      orgId: org.id,
      name: "E2E demo",
      versions: { create: { version: 1, steps: steps as unknown as Prisma.InputJsonValue } },
    },
    include: { versions: true },
  });
  const run = await prisma.run.create({
    data: { orgId: org.id, workflowVersionId: wf.versions[0]!.id, triggeredById: user.id },
  });

  console.log(`run ${run.id} enqueued`);
  await queue.add(QUEUE_NAMES.runWorkflow, { runId: run.id, orgId: org.id }, {
    jobId: runWorkflowJobId(run.id),
  });

  await waitFor(run.id, (s) => s === "AWAITING_APPROVAL", "gate");
  const gated = await prisma.run.findUniqueOrThrow({
    where: { id: run.id },
    include: { approvals: true },
  });
  console.log(`  halted at gate, cursor=${gated.cursor}, sessionKey=${gated.sessionKey ? "captured" : "none"}`);

  // Duplicate enqueue with the same id must collapse to one job.
  await queue.add(QUEUE_NAMES.runWorkflow, { runId: run.id, orgId: org.id }, {
    jobId: runWorkflowJobId(run.id),
  });

  await prisma.approval.update({
    where: { id: gated.approvals[0]!.id },
    data: { status: "APPROVED", resolvedById: user.id, resolvedAt: new Date() },
  });
  await prisma.run.update({ where: { id: run.id }, data: { status: "QUEUED" } });
  await queue.add(QUEUE_NAMES.runWorkflow, { runId: run.id, orgId: org.id, fromStepIndex: 2 }, {
    jobId: runWorkflowJobId(run.id, 2),
  });

  const done = await waitFor(run.id, (s) => ["SUCCEEDED", "FAILED", "INCIDENT"].includes(s), "finish");
  const submits = await prisma.runEvent.count({
    where: { runId: run.id, stepIndex: 2, type: "step.succeeded" },
  });
  const events = await prisma.runEvent.count({ where: { runId: run.id } });

  console.log(`  final status: ${done.status}${done.error ? ` (${done.error})` : ""}`);
  console.log(`  journal entries: ${events}`);
  console.log(`  times the submit click succeeded: ${submits}`);

  const journal = await prisma.runEvent.findMany({
    where: { runId: run.id },
    orderBy: { seq: "asc" },
    select: { seq: true, type: true, stepIndex: true },
  });
  console.log("  journal:");
  for (const e of journal) {
    console.log(`    ${e.seq}. ${e.type}${e.stepIndex === null ? "" : ` (step ${e.stepIndex})`}`);
  }

  const restored = journal.some((e) => e.type === "session.restored");
  console.log(`  restored browser state on resume: ${restored}`);
  console.log(submits === 1 && done.status === "SUCCEEDED" && restored ? "PASS" : "FAIL");

  await prisma.organization.delete({ where: { id: org.id } }).catch(() => undefined);
  await prisma.user.delete({ where: { id: user.id } }).catch(() => undefined);
  await queue.close();
  await connection.quit();
  await prisma.$disconnect();
}

void main();
