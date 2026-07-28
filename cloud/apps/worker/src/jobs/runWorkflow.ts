import type { Job } from "bullmq";
import { prisma, Prisma } from "@ghost/core/db";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { hashAuditEvent, type AuditPayload } from "@ghost/core/audit";
import type { RunWorkflowJob } from "@ghost/core/queue";
import { planNextAction, isNoopStep } from "../runtime/state-machine.js";
import { BrowserSession, restoreBrowserPrefix, runStep } from "../browser/driver.js";
import { artifactStore, screenshotKey } from "../storage/artifacts.js";

/**
 * Execute (or resume) a workflow run.
 *
 * Imperative shell around the pure state machine (`planNextAction`): it loads
 * the run, walks steps from `run.cursor`, and for each one either executes it
 * (screenshot + verify + RunStep + audit) or halts at an approval gate. Every
 * transition appends to the org's hash-chained audit log. Cancellation is
 * checked before each step, so the run is interruptible.
 */
export async function runWorkflowJob(job: Job<RunWorkflowJob>): Promise<void> {
  const { runId } = job.data;

  const run = await prisma.run.findUnique({
    where: { id: runId },
    include: { workflowVersion: true, approvals: true },
  });
  if (!run) throw new Error(`run ${runId} not found`);
  if (run.status === "CANCELED" || run.status === "SUCCEEDED" || run.status === "FAILED") return;

  const steps = parseWorkflowSteps(run.workflowVersion.steps);
  const approved = new Set(
    run.approvals.filter((a) => a.status === "APPROVED").map((a) => a.stepIndex),
  );

  await prisma.run.update({
    where: { id: runId },
    data: run.startedAt ? { status: "RUNNING" } : { status: "RUNNING", startedAt: new Date() },
  });
  if (!run.startedAt) {
    await appendAudit(run.orgId, run.triggeredById, {
      action: "run.started",
      entityType: "Run",
      entityId: runId,
    });
  }

  let session: BrowserSession | undefined;
  let cursor = run.cursor;

  try {
    for (;;) {
      const fresh = await prisma.run.findUnique({
        where: { id: runId },
        select: { status: true },
      });
      if (fresh?.status === "CANCELED") {
        await appendAudit(run.orgId, run.triggeredById, {
          action: "run.canceled",
          entityType: "Run",
          entityId: runId,
        });
        return;
      }

      const action = planNextAction(steps, cursor, approved);

      if (action.kind === "done") {
        await prisma.run.update({
          where: { id: runId },
          data: { status: "SUCCEEDED", endedAt: new Date(), cursor },
        });
        await appendAudit(run.orgId, run.triggeredById, {
          action: "run.succeeded",
          entityType: "Run",
          entityId: runId,
        });
        return;
      }

      if (action.kind === "gate") {
        const existing = await prisma.approval.findFirst({
          where: { runId, stepIndex: action.index, status: "PENDING" },
        });
        if (!existing) {
          await prisma.approval.create({
            data: { runId, stepIndex: action.index, reason: action.reason },
          });
        }
        await prisma.run.update({
          where: { id: runId },
          data: { status: "AWAITING_APPROVAL", cursor: action.index },
        });
        await appendAudit(run.orgId, run.triggeredById, {
          action: "run.awaiting_approval",
          entityType: "Approval",
          entityId: runId,
          metadata: { stepIndex: action.index, reason: action.reason },
        });
        return;
      }

      // action.kind === "execute"
      const { index, step } = action;
      await prisma.runStep.upsert({
        where: { runId_index: { runId, index } },
        create: {
          runId,
          index,
          type: step.type,
          status: "RUNNING",
          label: step.label ?? step.type,
          startedAt: new Date(),
        },
        update: { status: "RUNNING", startedAt: new Date() },
      });

      try {
        let screenshotRef: string | null = null;
        let verification: { passed: boolean; detail: string } | null = null;

        if (!isNoopStep(step)) {
          if (!session) {
            session = await BrowserSession.launch();
            // Resume-after-approval: rebuild page state lost when the prior job
            // closed Chromium at the gate. See restoreBrowserPrefix.
            if (cursor > 0) {
              await restoreBrowserPrefix(session.page, steps.slice(0, cursor));
            }
          }
          const result = await runStep(session.page, step);
          screenshotRef = await artifactStore().put(
            screenshotKey(runId, index),
            result.screenshot,
            "image/png",
          );
          verification = result.verification;
        }

        const passed = verification ? verification.passed : true;
        await prisma.runStep.update({
          where: { runId_index: { runId, index } },
          data: {
            status: passed ? "SUCCEEDED" : "FAILED",
            screenshotKey: screenshotRef,
            verification: verification ? (verification as Prisma.InputJsonValue) : Prisma.JsonNull,
            endedAt: new Date(),
          },
        });
        await appendAudit(run.orgId, run.triggeredById, {
          action: passed ? "step.succeeded" : "step.failed",
          entityType: "RunStep",
          entityId: `${runId}:${index}`,
          metadata: { type: step.type },
        });

        if (!passed) {
          await failRun(runId, index, `verification failed at step ${index}`);
          return;
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        await prisma.runStep.update({
          where: { runId_index: { runId, index } },
          data: { status: "FAILED", error: message, endedAt: new Date() },
        });
        await appendAudit(run.orgId, run.triggeredById, {
          action: "step.failed",
          entityType: "RunStep",
          entityId: `${runId}:${index}`,
          metadata: { error: message },
        });
        await failRun(runId, index, message);
        return;
      }

      cursor = index + 1;
    }
  } finally {
    if (session) await session.close();
  }
}

async function failRun(runId: string, index: number, error: string): Promise<void> {
  await prisma.run.update({
    where: { id: runId },
    data: { status: "FAILED", endedAt: new Date(), error, cursor: index },
  });
}

/** Append one event to the org's hash chain: hash folds in the prior hash. */
async function appendAudit(
  orgId: string,
  actorId: string | null,
  payload: AuditPayload,
): Promise<void> {
  const last = await prisma.auditEvent.findFirst({
    where: { orgId },
    orderBy: { createdAt: "desc" },
    select: { hash: true },
  });
  const prevHash = last?.hash ?? null;
  const hash = hashAuditEvent(prevHash, payload);
  await prisma.auditEvent.create({
    data: {
      orgId,
      actorId,
      action: payload.action,
      entityType: payload.entityType,
      entityId: payload.entityId ?? null,
      metadata:
        payload.metadata === undefined
          ? Prisma.JsonNull
          : (payload.metadata as Prisma.InputJsonValue),
      prevHash,
      hash,
    },
  });
}
