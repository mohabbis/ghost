import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueCompensateRun } from "@/lib/queue";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { describeActions, planCompensation, remainingEntries } from "@ghost/core/compensate";
import { journalFromEvents, journalFromLegacyRunSteps } from "@ghost/core/journal";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";

/**
 * Preview (GET) and trigger (POST) the reversal of a run.
 *
 * `GET` mutates nothing. It exists because Ghost's pipeline puts review before
 * execution, and undo is no exception — an operator should see exactly which
 * steps will be walked back, which of those will need approval, and, above all,
 * which completed side effects **cannot** be reversed, before deciding.
 *
 * That last part is the point. A "run undone" badge covering a run whose
 * confirmation email is still in a customer's inbox would be a lie of exactly
 * the kind this product exists to avoid.
 */

const REVERSIBLE_STATES = new Set(["SUCCEEDED", "FAILED", "CANCELED", "INCIDENT"]);

async function loadPlan(runId: string, orgId: string) {
  const run = await prisma.run.findFirst({
    where: { id: runId, orgId },
    include: { workflowVersion: true, steps: true },
  });
  if (!run) return null;

  const steps = parseWorkflowSteps(run.workflowVersion.steps);
  const events = await prisma.runEvent.findMany({
    where: { runId },
    orderBy: { seq: "asc" },
    select: { seq: true, type: true, stepIndex: true, payload: true },
  });
  const journal =
    events.length > 0
      ? journalFromEvents(events)
      : journalFromLegacyRunSteps(run.steps.map((s) => ({ index: s.index, status: s.status })));

  const plan = planCompensation(steps, journal);

  // Subtract what a previous, partly-successful reversal already undid.
  // `INCIDENT` is a reversible state, so a run that reversed two steps and then
  // failed on a third can be previewed again — and the preview must show what
  // is *left*, not re-offer work the worker's own `done` set will skip. The
  // count reported in the audit event comes from the same set.
  const alreadyCompensated = new Set(
    events
      .filter((e) => e.type === RUN_EVENT_TYPES.stepCompensated)
      .map((e) => e.stepIndex)
      .filter((i): i is number => i !== null),
  );

  return { run, plan, remaining: remainingEntries(plan, alreadyCompensated), alreadyCompensated };
}

export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const loaded = await loadPlan(id, session.user.orgId);
  if (!loaded) return NextResponse.json({ error: "not found" }, { status: 404 });

  const { run, plan, remaining, alreadyCompensated } = loaded;
  return NextResponse.json({
    runStatus: run.status,
    // A step still in flight makes the plan untrustworthy, so undo is not on
    // offer until the run settles — see `CompensationPlan.inFlight`.
    canUndo: REVERSIBLE_STATES.has(run.status) && plan.inFlight.length === 0,
    blockedBy: plan.inFlight.length > 0 ? plan.inFlight : null,
    complete: plan.complete,
    alreadyReversed: [...alreadyCompensated].sort((a, b) => a - b),
    entries: remaining.map((e) => ({
      stepIndex: e.stepIndex,
      stepLabel: e.stepLabel,
      description: e.compensation.description,
      // The author's description is a claim; these are what will actually run.
      // An approver cannot review "Open the order and click Cancel" — it says
      // nothing about which URL is opened or which control is clicked.
      actions: describeActions(e.compensation),
      requiresApproval: e.requiresApproval,
      approvalReason: e.approvalReason ?? null,
    })),
    irreversible: plan.irreversible.map((e) => ({
      stepIndex: e.stepIndex,
      stepLabel: e.stepLabel,
      reason: e.reason,
    })),
  });
}

export async function POST(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id } = await params;

  const loaded = await loadPlan(id, orgId);
  if (!loaded) return NextResponse.json({ error: "not found" }, { status: 404 });

  const { run, plan, remaining } = loaded;
  if (!REVERSIBLE_STATES.has(run.status)) {
    return NextResponse.json(
      { error: `a ${run.status} run cannot be reversed` },
      { status: 409 },
    );
  }
  if (plan.inFlight.length > 0) {
    // Cancellation lets the current step finish. Undoing around a step that is
    // still landing would reverse the steps before it and then let the draining
    // worker apply the newer side effect — breaking reverse order and reporting
    // a complete undo with the last action still in place.
    return NextResponse.json(
      {
        error: `step ${plan.inFlight[0]} is still in flight — wait for the run to settle before reversing it`,
      },
      { status: 409 },
    );
  }
  if (remaining.length === 0) {
    return NextResponse.json(
      {
        error:
          plan.entries.length > 0
            ? "every reversible step in this run has already been reversed"
            : plan.irreversible.length > 0
              ? "nothing in this run can be reversed"
              : "this run made no changes to reverse",
      },
      { status: 409 },
    );
  }

  // Conditional update: a double-clicked Undo enqueues once.
  const { count } = await prisma.run.updateMany({
    where: { id, orgId, status: { in: [...REVERSIBLE_STATES] as never } },
    data: { status: "COMPENSATING", error: null, endedAt: null },
  });
  if (count !== 1) {
    return NextResponse.json({ error: "run is already being reversed" }, { status: 409 });
  }

  await appendAuditEvent(orgId, userId, {
    action: "run.compensation_requested",
    entityType: "Run",
    entityId: id,
    metadata: {
      requestedById: userId,
      reversible: remaining.length,
      irreversible: plan.irreversible.length,
    },
  });

  // `requestedById` follows the job so the worker attributes each reversal to
  // the person who asked for it, not to whoever started the original run.
  await enqueueCompensateRun({
    runId: id,
    orgId,
    requestedById: userId,
    resumeToken: `req-${Date.now()}`,
  });
  return NextResponse.json({ ok: true, reversing: remaining.length });
}
