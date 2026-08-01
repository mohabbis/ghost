import type { Job } from "bullmq";
import { prisma } from "@ghost/core/db";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { appendAuditEvent, appendRunEvent, runChainHead } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import type { CompensateRunJob } from "@ghost/core/queue";
import { journalFromEvents, journalFromLegacyRunSteps } from "@ghost/core/journal";
import { planCompensation, actionAsStep, type CompensationEntry } from "@ghost/core/compensate";
import { BrowserSession, applyStep, verifyStep } from "../browser/driver.js";
import { artifactStore, screenshotKey } from "../storage/artifacts.js";

/**
 * Reverse a run's completed side effects — the BPMN saga, over the run journal.
 *
 * Camunda triggers compensation by invoking completed activities' handlers in
 * reverse. Ghost can do the same because the journal already records precisely
 * what completed, so reversal is a backwards walk over evidence rather than a
 * guess.
 *
 * Three properties this job is built to preserve:
 *
 * **Reversal is itself gated.** Undoing a submitted order is a mutating action.
 * Each reversal is classified, and a sensitive one halts for approval exactly
 * like a forward step — using a COMPENSATION-phase Approval so it cannot
 * collide with the forward gate on the same index.
 *
 * **Nothing is silently skipped.** A completed side effect with no defined
 * reversal is journalled as `step.irreversible` and surfaced. An operator who
 * thinks a run was fully undone when the confirmation email is still out is
 * worse off than one who was told the truth.
 *
 * **The reversal is itself audited.** Every attempt appends to the same
 * hash-chained journal, so the record shows what was undone, by whom, and what
 * could not be.
 */
export async function compensateRunJob(job: Job<CompensateRunJob>): Promise<void> {
  const { runId } = job.data;

  const run = await prisma.run.findUnique({
    where: { id: runId },
    include: { workflowVersion: true, approvals: true, steps: true },
  });
  if (!run) throw new Error(`run ${runId} not found`);
  if (run.status !== "COMPENSATING") return;

  const steps = parseWorkflowSteps(run.workflowVersion.steps);

  const events = await prisma.runEvent.findMany({ where: { runId }, orderBy: { seq: "asc" } });
  const journal =
    events.length > 0
      ? journalFromEvents(events)
      : journalFromLegacyRunSteps(run.steps.map((s) => ({ index: s.index, status: s.status })));

  const plan = planCompensation(steps, journal);

  // Reversals already done in a previous attempt — this job is re-entrant for
  // the same reason forward execution is.
  const done = new Set(
    events
      .filter((e) => e.type === RUN_EVENT_TYPES.stepCompensated)
      .map((e) => e.stepIndex)
      .filter((i): i is number => i !== null),
  );

  const approved = new Set(
    run.approvals
      .filter((a) => a.phase === "COMPENSATION" && a.status === "APPROVED")
      .map((a) => a.stepIndex),
  );
  const rejected = new Set(
    run.approvals
      .filter((a) => a.phase === "COMPENSATION" && a.status === "REJECTED")
      .map((a) => a.stepIndex),
  );

  if (done.size === 0) {
    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.compensationStarted,
      payload: {
        reversible: plan.entries.length,
        irreversible: plan.irreversible.length,
      },
    });
    // Record what cannot be undone up front, so the journal carries it even if
    // the reversal later fails partway.
    for (const entry of plan.irreversible) {
      await appendRunEvent(runId, {
        type: RUN_EVENT_TYPES.stepIrreversible,
        stepIndex: entry.stepIndex,
        payload: { stepId: entry.stepId, reason: entry.reason },
      });
    }
  }

  let session: BrowserSession | undefined;

  try {
    for (const entry of plan.entries) {
      if (done.has(entry.stepIndex)) continue;

      if (rejected.has(entry.stepIndex)) {
        await finish(runId, run.orgId, run.triggeredById, "rejected", plan.irreversible.length);
        return;
      }

      // Gate before touching anything.
      if (entry.requiresApproval && !approved.has(entry.stepIndex)) {
        await prisma.approval.upsert({
          where: {
            runId_stepIndex_phase: {
              runId,
              stepIndex: entry.stepIndex,
              phase: "COMPENSATION",
            },
          },
          create: {
            runId,
            stepIndex: entry.stepIndex,
            phase: "COMPENSATION",
            reason: entry.approvalReason ?? entry.compensation.description,
          },
          update: {},
        });
        await prisma.run.update({
          where: { id: runId },
          data: { status: "AWAITING_APPROVAL", cursor: entry.stepIndex },
        });
        await appendRunEvent(runId, {
          type: RUN_EVENT_TYPES.gateOpened,
          stepIndex: entry.stepIndex,
          payload: { phase: "COMPENSATION", reason: entry.approvalReason },
        });
        await appendAuditEvent(run.orgId, run.triggeredById, {
          action: "run.awaiting_approval",
          entityType: "Approval",
          entityId: runId,
          metadata: {
            stepIndex: entry.stepIndex,
            phase: "COMPENSATION",
            reason: entry.approvalReason,
          },
        });
        return;
      }

      if (!session) session = await BrowserSession.launch();
      const ok = await reverseOne(runId, run.orgId, run.triggeredById, entry, session);
      if (!ok) return;
      done.add(entry.stepIndex);
    }

    await finish(runId, run.orgId, run.triggeredById, "complete", plan.irreversible.length);
  } finally {
    if (session) await session.close().catch(() => undefined);
  }
}

async function reverseOne(
  runId: string,
  orgId: string,
  actorId: string | null,
  entry: CompensationEntry,
  session: BrowserSession,
): Promise<boolean> {
  // Compensation actions carry no per-action policy of their own, so they use
  // the same default budget a step would.
  const timeoutMs = Number(process.env.GHOST_STEP_TIMEOUT_MS) || 30_000;

  try {
    for (const action of entry.compensation.actions) {
      await applyStep(session.page, actionAsStep(action, entry.stepId), { timeoutMs });
    }

    const verification = entry.compensation.verify
      ? await verifyStep(
          session.page,
          { id: entry.stepId, type: "verify", assertion: entry.compensation.verify },
        )
      : null;

    if (verification && !verification.passed) {
      throw new Error(`reversal verification failed: ${verification.detail}`);
    }

    const shot = await session.page.screenshot().catch(() => null);
    const key = shot
      ? await artifactStore()
          .put(screenshotKey(runId, entry.stepIndex), shot, "image/png")
          .catch(() => null)
      : null;

    await prisma.$transaction(async (tx) => {
      await tx.runStep.updateMany({
        where: { runId, index: entry.stepIndex },
        data: { status: "SKIPPED", endedAt: new Date() },
      });
      await appendRunEvent(
        runId,
        {
          type: RUN_EVENT_TYPES.stepCompensated,
          stepIndex: entry.stepIndex,
          payload: {
            stepId: entry.stepId,
            description: entry.compensation.description,
            screenshotKey: key,
          },
        },
        tx,
      );
    });

    await appendAuditEvent(orgId, actorId, {
      action: "step.compensated",
      entityType: "RunStep",
      entityId: `${runId}:${entry.stepIndex}`,
      metadata: { description: entry.compensation.description },
    });
    return true;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);

    await appendRunEvent(runId, {
      type: RUN_EVENT_TYPES.stepCompensationFailed,
      stepIndex: entry.stepIndex,
      payload: { stepId: entry.stepId, error: message },
    });
    // A failed reversal leaves the system in a half-undone state, which is
    // exactly when a human needs to look. Stop here rather than pressing on.
    await prisma.run.update({
      where: { id: runId },
      data: {
        status: "INCIDENT",
        cursor: entry.stepIndex,
        error: `reversal of step ${entry.stepIndex} failed: ${message}`,
      },
    });
    await appendAuditEvent(orgId, actorId, {
      action: "run.compensation_failed",
      entityType: "Run",
      entityId: runId,
      metadata: { stepIndex: entry.stepIndex, error: message },
    });
    return false;
  }
}

async function finish(
  runId: string,
  orgId: string,
  actorId: string | null,
  outcome: "complete" | "rejected",
  irreversibleCount: number,
): Promise<void> {
  await prisma.run.update({
    where: { id: runId },
    data: {
      status: outcome === "complete" ? "COMPENSATED" : "INCIDENT",
      endedAt: new Date(),
      // Stated on the run itself, so nobody reads "COMPENSATED" as "fully
      // undone" when part of it could not be.
      error:
        outcome === "rejected"
          ? "reversal rejected by an approver"
          : irreversibleCount > 0
            ? `reversed, but ${irreversibleCount} completed step(s) had no defined reversal`
            : null,
    },
  });
  await appendRunEvent(runId, {
    type: RUN_EVENT_TYPES.compensationFinished,
    payload: { outcome, irreversible: irreversibleCount },
  });
  await appendAuditEvent(orgId, actorId, {
    action: outcome === "complete" ? "run.compensated" : "run.compensation_rejected",
    entityType: "Run",
    entityId: runId,
    metadata: { irreversible: irreversibleCount, runChainHead: await runChainHead(runId) },
  });
}
