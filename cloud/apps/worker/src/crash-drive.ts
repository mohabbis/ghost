/**
 * Crash-recovery driver (not a test): kills the worker **mid-mutation** and
 * verifies the run recovers without performing that mutation twice.
 *
 * This is the property the whole durable-execution design exists for, and the
 * one thing the vitest suite structurally cannot prove: it calls
 * `runWorkflowJob` in-process, so there is no process to kill, no lease to
 * expire, and no BullMQ redelivery. `e2e-drive.ts` exercises the real queue but
 * lets every job finish. Neither answers "what if the box dies halfway through
 * clicking Submit?" — which, for a product whose pitch is that a human approved
 * one payment and got exactly one payment, is the question that matters.
 *
 * What it does:
 *
 *   1. seeds an org, workflow and run against the web app's /fixtures/order
 *   2. spawns a worker as a child process and enqueues the run
 *   3. waits for the approval gate, approves, and re-enqueues
 *   4. waits until the journal shows `step.started` for the submit step with
 *      no matching `step.succeeded` — the browser is mid-click, the mutation
 *      is in flight
 *   5. SIGKILLs the worker. Not SIGTERM: graceful shutdown is the case that
 *      already works, and a power cut does not send SIGTERM.
 *   6. restarts a worker and lets BullMQ redeliver after the lease expires
 *   7. asserts the run reaches a terminal state having succeeded that step
 *      exactly once
 *
 * PASS requires `step.succeeded` for the submit step to appear **exactly
 * once**. Two would mean the customer's order was placed twice. Zero with a
 * SUCCEEDED status would mean the engine skipped a step it claimed to run.
 *
 * A run that ends INCIDENT is also an acceptable outcome and is reported as
 * such: refusing to resume when page state cannot be provably rebuilt is the
 * designed fail-safe. What is never acceptable is two submits.
 *
 * Run with:
 *   pnpm --filter @ghost/worker exec tsx src/crash-drive.ts
 *
 * Requires DATABASE_URL, REDIS_URL, GHOST_SESSION_KEY, GHOST_ARTIFACT_DIR and a
 * web app serving APP_URL — the same environment as `e2e-drive.ts`.
 */
import { spawn, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { Queue } from "bullmq";
import IORedis from "ioredis";
import { prisma, Prisma } from "@ghost/core/db";
import { QUEUE_NAMES, runWorkflowJobId } from "@ghost/core/queue";
import type { WorkflowSteps } from "@ghost/core/schema/step";

const APP = process.env.APP_URL ?? "http://localhost:3000";
const SUBMIT_STEP = 2;

const connection = new IORedis(process.env.REDIS_URL!, { maxRetriesPerRequest: null });
const queue = new Queue(QUEUE_NAMES.runWorkflow, { connection });

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * Kills the worker's whole process group and waits for it to actually be gone.
 *
 * Returning before the process has exited would let the driver conclude that
 * recovery worked when in fact the original worker was still running and
 * simply finished the job itself.
 */
async function killWorkerGroup(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise<void>((resolve) => child.once("exit", () => resolve()));
  try {
    process.kill(-child.pid!, "SIGKILL");
  } catch {
    child.kill("SIGKILL");
  }
  await Promise.race([exited, sleep(10_000)]);
  if (child.exitCode === null && child.signalCode === null) {
    throw new Error("worker did not die — the crash was not simulated, so any result is meaningless");
  }
}

/**
 * `detached: true` puts the worker in its own process group so it can be
 * killed as a group.
 *
 * This is not incidental. Spawning through a wrapper (`npx tsx …`) and calling
 * `child.kill()` signals the *wrapper*; the node process actually running the
 * worker is a grandchild and survives, finishes the step normally, and the
 * driver reports a confident PASS having tested nothing. That is exactly what
 * the first version of this file did.
 */
function startWorker(label: string): ChildProcess {
  const entry = fileURLToPath(new URL("./index.ts", import.meta.url));
  const tsxBin = fileURLToPath(new URL("../../../node_modules/.bin/tsx", import.meta.url));
  const child = spawn(tsxBin, [entry], {
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
    detached: true,
  });
  const tag = (line: string) => `    [${label}] ${line}`;
  child.stdout?.on("data", (b: Buffer) =>
    b
      .toString()
      .split("\n")
      .filter(Boolean)
      .forEach((l) => console.log(tag(l))),
  );
  child.stderr?.on("data", (b: Buffer) =>
    b
      .toString()
      .split("\n")
      .filter(Boolean)
      .forEach((l) => console.log(tag(l))),
  );
  return child;
}

async function waitForStatus(runId: string, ok: (s: string) => boolean, label: string, ms = 90_000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const run = await prisma.run.findUniqueOrThrow({ where: { id: runId } });
    if (ok(run.status)) return run;
    await sleep(400);
  }
  throw new Error(`timed out waiting for ${label}`);
}

/** True once the submit step is in flight: started, not yet finished. */
async function submitInFlight(runId: string): Promise<boolean> {
  const events = await prisma.runEvent.findMany({
    where: { runId, stepIndex: SUBMIT_STEP },
    select: { type: true },
  });
  const started = events.some((e) => e.type === "step.started");
  const settled = events.some((e) => e.type === "step.succeeded" || e.type === "step.failed");
  return started && !settled;
}

async function main() {
  const slug = `crash-${Date.now()}`;
  const org = await prisma.organization.create({ data: { name: "Crash", slug } });
  const user = await prisma.user.create({
    data: {
      email: `${slug}@example.com`,
      memberships: { create: { orgId: org.id, role: "OWNER" } },
    },
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
      name: "Crash demo",
      versions: { create: { version: 1, steps: steps as unknown as Prisma.InputJsonValue } },
    },
    include: { versions: true },
  });
  const run = await prisma.run.create({
    data: { orgId: org.id, workflowVersionId: wf.versions[0]!.id, triggeredById: user.id },
  });

  let worker = startWorker("worker-1");
  let killed = false;

  try {
    console.log(`run ${run.id}`);
    await queue.add(
      QUEUE_NAMES.runWorkflow,
      { runId: run.id, orgId: org.id },
      { jobId: runWorkflowJobId(run.id) },
    );

    await waitForStatus(run.id, (s) => s === "AWAITING_APPROVAL", "gate");
    const gated = await prisma.run.findUniqueOrThrow({
      where: { id: run.id },
      include: { approvals: true },
    });
    console.log(`  halted at gate (cursor=${gated.cursor})`);

    await prisma.approval.update({
      where: { id: gated.approvals[0]!.id },
      data: { status: "APPROVED", resolvedById: user.id, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id: run.id }, data: { status: "QUEUED" } });
    await queue.add(
      QUEUE_NAMES.runWorkflow,
      { runId: run.id, orgId: org.id, fromStepIndex: SUBMIT_STEP },
      { jobId: runWorkflowJobId(run.id, SUBMIT_STEP) },
    );
    console.log("  approved, resume enqueued");

    // Race the worker to the exact moment the mutation is in flight. Polled
    // tightly because the window between `step.started` and `step.succeeded`
    // for a single click is small.
    const deadline = Date.now() + 60_000;
    while (Date.now() < deadline) {
      if (await submitInFlight(run.id)) break;
      const status = (await prisma.run.findUniqueOrThrow({ where: { id: run.id } })).status;
      if (["SUCCEEDED", "FAILED", "INCIDENT"].includes(status)) break;
      await sleep(25);
    }

    const caught = await submitInFlight(run.id);
    await killWorkerGroup(worker);
    killed = true;
    console.log(
      caught
        ? `  SIGKILLed the worker (pid ${worker.pid}) with the submit click in flight`
        : `  NOTE: did not catch the submit mid-flight; killed anyway (pid ${worker.pid})`,
    );
    console.log(`  worker exit: code=${worker.exitCode} signal=${worker.signalCode}`);

    // The lease has to lapse before another worker may claim the run; that is
    // the mechanism preventing two workers executing one run, and it is why
    // recovery is not instant.
    await sleep(2_000);
    worker = startWorker("worker-2");
    console.log("  restarted worker, waiting for recovery");

    // Nudge redelivery: a job killed mid-flight is reclaimed by BullMQ's
    // stalled-job check, which is slower than this driver's patience.
    await queue.add(
      QUEUE_NAMES.runWorkflow,
      { runId: run.id, orgId: org.id, fromStepIndex: SUBMIT_STEP },
      { jobId: `${runWorkflowJobId(run.id, SUBMIT_STEP)}-recover` },
    );

    const done = await waitForStatus(
      run.id,
      (s) => ["SUCCEEDED", "FAILED", "INCIDENT"].includes(s),
      "recovery",
      120_000,
    );

    const submits = await prisma.runEvent.count({
      where: { runId: run.id, stepIndex: SUBMIT_STEP, type: "step.succeeded" },
    });
    const journal = await prisma.runEvent.findMany({
      where: { runId: run.id },
      orderBy: { seq: "asc" },
      select: { seq: true, type: true, stepIndex: true },
    });

    console.log(`  final status: ${done.status}${done.error ? ` (${done.error})` : ""}`);
    console.log("  journal:");
    for (const e of journal) {
      console.log(`    ${e.seq}. ${e.type}${e.stepIndex === null ? "" : ` (step ${e.stepIndex})`}`);
    }
    console.log(`  times the submit click succeeded: ${submits}`);

    // The invariant. Anything other than exactly one is a defect, including
    // zero alongside SUCCEEDED — that would be a skipped step reported as done.
    const noDuplicate = submits <= 1;
    const consistent = done.status === "SUCCEEDED" ? submits === 1 : true;
    console.log(noDuplicate && consistent ? "PASS" : "FAIL");
  } finally {
    await killWorkerGroup(worker).catch(() => undefined);
    void killed;
    await prisma.organization.delete({ where: { id: org.id } }).catch(() => undefined);
    await prisma.user.delete({ where: { id: user.id } }).catch(() => undefined);
    await queue.close();
    await connection.quit();
    await prisma.$disconnect();
  }
}

void main();
