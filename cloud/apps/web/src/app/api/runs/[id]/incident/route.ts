import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";
import { appendAuditEvent, appendRunEvent } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { classifyStep } from "@ghost/core/classifier";

/**
 * Resolve an incident: retry the failed step, or skip it.
 *
 * Borrowed from Camunda. A failed step stops the run and waits for a person
 * rather than killing it — an ops person watching a 40-invoice batch wants to
 * fix the one broken step, not restart the batch.
 *
 * **Skip is gated.** Skipping a step the classifier calls sensitive requires a
 * fresh approval, because otherwise "skip" would be a way to walk a run past
 * the approval gate — the one thing the engine must never allow.
 */
/**
 * Has a reversal been started on this run and not yet concluded?
 *
 * Read from the journal rather than a status flag: `COMPENSATING` is cleared
 * the moment the reversal stops, so by the time the run is an INCIDENT the
 * status no longer says which direction it was going.
 */
async function compensationInProgress(runId: string): Promise<boolean> {
  const last = await prisma.runEvent.findFirst({
    where: {
      runId,
      type: { in: [RUN_EVENT_TYPES.compensationStarted, RUN_EVENT_TYPES.compensationFinished] },
    },
    orderBy: { seq: "desc" },
    select: { type: true },
  });
  return last?.type === RUN_EVENT_TYPES.compensationStarted;
}

export async function POST(req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id } = await params;

  const body = (await req.json().catch(() => ({}))) as { action?: string };
  if (body.action !== "retry" && body.action !== "skip") {
    return NextResponse.json({ error: "action must be retry|skip" }, { status: 400 });
  }

  const run = await prisma.run.findFirst({
    where: { id, orgId, status: "INCIDENT" },
    include: { workflowVersion: true },
  });
  if (!run) return NextResponse.json({ error: "no incident on this run" }, { status: 404 });

  // A compensation that failed also stops as INCIDENT, and these controls are
  // forward-recovery controls. Letting them run on a reversal would append
  // forward-step recovery events to a run whose steps are all already complete:
  // the forward worker would find nothing left to do and mark an originally
  // successful run SUCCEEDED, quietly erasing the fact that its undo failed
  // partway. Reversal has its own recovery path (retry the undo, or accept the
  // partial state) and must not borrow this one.
  const compensation = await compensationInProgress(id);
  if (compensation) {
    return NextResponse.json(
      {
        error:
          "this incident is a failed reversal, not a failed step — retry or skip here would " +
          "resume the original run and hide the incomplete undo. Request the undo again once " +
          "the cause is fixed.",
      },
      { status: 409 },
    );
  }

  const steps = parseWorkflowSteps(run.workflowVersion.steps);
  const index = run.cursor;
  const step = steps[index];
  if (!step) return NextResponse.json({ error: "incident step not found" }, { status: 409 });

  // Monotonic discriminator for the resume job id. Both an approval resume and
  // this incident retry resume from the same step index, and BullMQ would drop
  // the second as a duplicate — leaving the run QUEUED with nothing to pick it
  // up. The journal sequence is already monotonic per run, so it distinguishes
  // them without inventing a second counter.
  let resumeSeq = 0;

  if (body.action === "skip") {
    if (classifyStep(step).sensitive) {
      return NextResponse.json(
        {
          error:
            "this step requires approval, so it cannot be skipped past the gate — " +
            "cancel the run instead, or fix the workflow and start a new run",
        },
        { status: 403 },
      );
    }

    await prisma.$transaction(async (tx) => {
      await tx.runStep.upsert({
        where: { runId_index: { runId: id, index } },
        create: {
          runId: id,
          index,
          type: step.type,
          status: "SKIPPED",
          label: step.label ?? step.type,
          endedAt: new Date(),
        },
        update: { status: "SKIPPED", endedAt: new Date() },
      });
      await tx.run.update({
        where: { id },
        data: { status: "QUEUED", error: null, cursor: index + 1 },
      });
      const { seq } = await appendRunEvent(
        id,
        {
          type: RUN_EVENT_TYPES.stepSkipped,
          stepIndex: index,
          payload: { skippedById: userId, stepId: step.id },
        },
        tx,
      );
      resumeSeq = seq;
    });
  } else {
    // Retry: clear the recorded failure so the journal fold stops reporting it.
    // The step's own `step.started`/`step.failed` history stays in the chain —
    // this appends, it never rewrites.
    await prisma.$transaction(async (tx) => {
      await tx.runStep.updateMany({
        where: { runId: id, index },
        data: { status: "PENDING", error: null },
      });
      await tx.run.update({ where: { id }, data: { status: "QUEUED", error: null } });
      const { seq } = await appendRunEvent(
        id,
        {
          type: RUN_EVENT_TYPES.stepRetryRequested,
          stepIndex: index,
          payload: { phase: "incident", retriedById: userId },
        },
        tx,
      );
      resumeSeq = seq;
    });
  }

  await appendAuditEvent(orgId, userId, {
    action: body.action === "skip" ? "run.incident_skipped" : "run.incident_retried",
    entityType: "Run",
    entityId: id,
    metadata: { stepIndex: index },
  });

  await enqueueRunWorkflow({
    runId: id,
    orgId,
    fromStepIndex: index,
    resumeToken: `incident-${resumeSeq}`,
  });
  return NextResponse.json({ ok: true });
}
