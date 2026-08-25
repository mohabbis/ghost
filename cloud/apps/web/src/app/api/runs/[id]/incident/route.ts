import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";
import { appendAuditEvent, appendRunEvent } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { classifyStep } from "@ghost/core/classifier";
import { classifyException, duplicateRiskFor } from "@ghost/core/classifier/exception";

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

  const body = (await req.json().catch(() => ({}))) as {
    action?: string;
    assigneeId?: string | null;
    acknowledgeDuplicateRisk?: boolean;
    // Identity of the incident the caller was looking at when they confirmed.
    // See the acknowledgement check below.
    expectStepIndex?: number;
    expectIncidentRaisedAt?: string;
  };
  if (body.action !== "retry" && body.action !== "skip" && body.action !== "assign") {
    return NextResponse.json({ error: "action must be retry|skip|assign" }, { status: 400 });
  }

  const run = await prisma.run.findFirst({
    where: { id, orgId, status: "INCIDENT" },
    include: { workflowVersion: true },
  });
  if (!run) return NextResponse.json({ error: "no incident on this run" }, { status: 404 });

  // ---- Assign -----------------------------------------------------------
  // Handled before the compensation guard below, because assignment is the one
  // control that is always appropriate: a failed *reversal* is exactly the kind
  // of incident that needs a named owner, even though retry and skip are refused
  // on it.
  //
  // Assignment changes no run state and resumes nothing, so it is open to any
  // member. Deciding who looks at a problem is not authorizing the action that
  // caused it.
  if (body.action === "assign") {
    let assigneeId: string | null = null;
    if (body.assigneeId != null) {
      if (typeof body.assigneeId !== "string") {
        return NextResponse.json({ error: "assigneeId must be a string or null" }, { status: 400 });
      }
      // Tenant isolation: an exception may only be assigned to a member of the
      // org that owns the run. Without this check any user id would be accepted,
      // leaking the existence of accounts across tenants and putting another
      // org's user on this org's queue.
      const member = await prisma.membership.findFirst({
        where: { orgId, userId: body.assigneeId },
        select: { userId: true },
      });
      if (!member) {
        return NextResponse.json({ error: "assignee is not a member of this org" }, { status: 404 });
      }
      assigneeId = member.userId;
    }

    // One transaction: a routing mutation that applied but was never recorded
    // would leave the queue showing an owner with no audit trail explaining how
    // they got it, while the caller was told the request failed. Either both
    // land or neither does. `appendAuditEvent` takes the tx for exactly this.
    // Both the membership check and the run state are re-verified *inside* the
    // transaction below. The lookup above is only an early, friendly 404: member
    // removal runs its own unassignment cleanup in a transaction of its own, and
    // between that read and this write the membership can vanish — the foreign
    // key references `User`, not `Membership`, so the write would still succeed
    // and re-attach a non-member to this org's queue.
    //
    // Conditional on the run still being an INCIDENT, inside the transaction.
    // The `findFirst` above is a read: a retry, skip, cancel or undo can resolve
    // the incident between it and this write, and an id-only update would then
    // assign an owner to a run that is no longer an exception — reporting
    // success, and leaving a stale assignee to surface on the run's *next*
    // incident.
    let assigned = 0;
    await prisma.$transaction(async (tx) => {
      if (assigneeId !== null) {
        const stillMember = await tx.membership.findFirst({
          where: { orgId, userId: assigneeId },
          select: { userId: true },
        });
        if (!stillMember) return;
      }
      const res = await tx.run.updateMany({
        where: { id, orgId, status: "INCIDENT" },
        data: { incidentAssigneeId: assigneeId },
      });
      assigned = res.count;
      if (assigned !== 1) return;
      await appendAuditEvent(
        orgId,
        userId,
        {
          action: assigneeId ? "run.incident_assigned" : "run.incident_unassigned",
          entityType: "Run",
          entityId: id,
          metadata: { stepIndex: run.cursor, assigneeId },
        },
        tx,
      );
    });
    if (assigned !== 1) {
      return NextResponse.json(
        {
          error:
            "could not assign: the run is no longer an open exception, or that " +
            "person is no longer a member of this organization",
        },
        { status: 409 },
      );
    }
    return NextResponse.json({ ok: true, assigneeId });
  }

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
        // Clear the routing fields: this run is no longer an open exception, so
        // it must leave the queue and must not stay assigned to someone who has
        // finished with it. A later failure raises a fresh, re-classified
        // incident.
        data: {
          status: "QUEUED",
          error: null,
          cursor: index + 1,
          incidentKind: null,
          incidentAssigneeId: null,
          incidentRaisedAt: null,
        },
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
      // Same atomicity as the retry branch: the state change, the journal
      // event and the org audit record commit together or not at all.
      await appendAuditEvent(
        orgId,
        userId,
        {
          action: "run.incident_skipped",
          entityType: "Run",
          entityId: id,
          metadata: { stepIndex: index, kind: run.incidentKind },
        },
        tx,
      );
    });
  } else {
    // Retrying a step whose effect may already have happened is a decision the
    // engine deliberately leaves to a human (see journal.ts on clearing
    // `inFlight`) — but it must be a decision, not a mis-click. The disposition
    // is recomputed here rather than read from `Run.incidentKind`, so a stale
    // stored label cannot wave a risky retry through.
    //
    // This adds no prohibition: the retry still happens, on the same terms as
    // before, for any caller that says it understands the risk. What changes is
    // that it cannot happen *accidentally*, and the record shows the warning was
    // shown.
    // Deliberately NOT `.catch(() => null)`. The recorded outcome is the signal
    // that decides whether this retry needs an acknowledgement, so swallowing a
    // transient database error would turn "I could not find out" into "there is
    // nothing to worry about" — the least restrictive answer available, on the
    // one query where being wrong repeats a payment. A lookup failure means the
    // gate cannot be evaluated, so the retry is refused rather than allowed.
    let recorded: { status: string } | null;
    try {
      recorded = await prisma.runStep.findUnique({
        where: { runId_index: { runId: id, index } },
        select: { status: true },
      });
    } catch {
      return NextResponse.json(
        {
          error:
            "could not determine whether this step's effect is known, so the retry was refused. " +
            "This is a transient storage error — try again.",
        },
        { status: 503 },
      );
    }

    const disposition = classifyException({
      reason: run.error ?? "",
      step,
      recordedOutcome:
        recorded?.status === "UNKNOWN" ? "UNKNOWN" : recorded?.status === "FAILED" ? "FAILED" : null,
    });

    // Union of the live verdict and the stored label, still gated on the step
    // actually reaching outside the browser. See duplicateRiskFor.
    const mayDuplicate = duplicateRiskFor({
      disposition,
      storedKind: run.incidentKind,
      recordedOutcome: recorded?.status === "UNKNOWN" ? "UNKNOWN" : null,
      step,
    });

    // An acknowledgement has to be *about something*. A bare boolean can
    // outlive the incident it was shown for: if another operator resolves this
    // incident while the confirmation is open and the run then parks on a new
    // risky step, the still-open dialog would acknowledge a step its clicker
    // never looked at. So the caller states which incident it was shown — step
    // index and raised-at — and a mismatch is refused. Same principle as
    // approving the *resolved* action rather than the template: the human must
    // be confirming the thing that actually runs.
    if (mayDuplicate && body.acknowledgeDuplicateRisk === true) {
      const sameStep =
        body.expectStepIndex === undefined || body.expectStepIndex === index;
      const sameIncident =
        body.expectIncidentRaisedAt === undefined ||
        (run.incidentRaisedAt !== null &&
          new Date(body.expectIncidentRaisedAt).getTime() === run.incidentRaisedAt.getTime());
      if (!sameStep || !sameIncident) {
        return NextResponse.json(
          {
            error:
              "this run has stopped on a different step since that confirmation was shown. " +
              "Re-read the current exception before retrying.",
            requiresAcknowledgement: true,
            staleAcknowledgement: true,
          },
          { status: 409 },
        );
      }
    }

    if (mayDuplicate && body.acknowledgeDuplicateRisk !== true) {
      return NextResponse.json(
        {
          error:
            "this step may already have taken effect, so retrying it could repeat that effect. " +
            "Confirm in the target system first, then retry with acknowledgeDuplicateRisk: true.",
          kind: disposition.kind,
          guidance: disposition.guidance,
          requiresAcknowledgement: true,
        },
        { status: 409 },
      );
    }
    // Retry: clear the recorded failure so the journal fold stops reporting it.
    // The step's own `step.started`/`step.failed` history stays in the chain —
    // this appends, it never rewrites.
    await prisma.$transaction(async (tx) => {
      await tx.runStep.updateMany({
        where: { runId: id, index },
        data: { status: "PENDING", error: null },
      });
      await tx.run.update({
        where: { id },
        // See the skip branch: leaving INCIDENT clears the routing fields.
        data: {
          status: "QUEUED",
          error: null,
          incidentKind: null,
          incidentAssigneeId: null,
          incidentRaisedAt: null,
        },
      });
      const { seq } = await appendRunEvent(
        id,
        {
          type: RUN_EVENT_TYPES.stepRetryRequested,
          stepIndex: index,
          payload: {
            phase: "incident",
            retriedById: userId,
            kind: disposition.kind,
            // Present only when the retry carried duplicate risk, so its
            // presence in the chain is itself the evidence of an informed
            // decision rather than a flag that is always there.
            ...(mayDuplicate ? { acknowledgedDuplicateRisk: true } : {}),
          },
        },
        tx,
      );
      resumeSeq = seq;
      // Inside the transaction, with the state change and the journal event.
      // Appended after them so it is ordered last in the chain, but committed
      // atomically: if this were left outside and failed, the run would already
      // have left INCIDENT carrying an acknowledgement recorded only in the run
      // journal, and the stalled-run reclaimer could later drive the retry with
      // no org-level audit record of who accepted the duplicate risk.
      await appendAuditEvent(
        orgId,
        userId,
        {
          action: "run.incident_retried",
          entityType: "Run",
          entityId: id,
          metadata: {
            stepIndex: index,
            kind: run.incidentKind,
            ...(mayDuplicate ? { acknowledgedDuplicateRisk: true } : {}),
          },
        },
        tx,
      );
    });
  }

  await enqueueRunWorkflow({
    runId: id,
    orgId,
    fromStepIndex: index,
    resumeToken: `incident-${resumeSeq}`,
  });
  return NextResponse.json({ ok: true });
}
