import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueCompensateRun } from "@/lib/queue";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { planCompensation } from "@ghost/core/compensate";
import { journalFromEvents, journalFromLegacyRunSteps } from "@ghost/core/journal";

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

  return { run, plan: planCompensation(steps, journal) };
}

export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const loaded = await loadPlan(id, session.user.orgId);
  if (!loaded) return NextResponse.json({ error: "not found" }, { status: 404 });

  const { run, plan } = loaded;
  return NextResponse.json({
    runStatus: run.status,
    canUndo: REVERSIBLE_STATES.has(run.status),
    complete: plan.complete,
    entries: plan.entries.map((e) => ({
      stepIndex: e.stepIndex,
      stepLabel: e.stepLabel,
      description: e.compensation.description,
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

  const { run, plan } = loaded;
  if (!REVERSIBLE_STATES.has(run.status)) {
    return NextResponse.json(
      { error: `a ${run.status} run cannot be reversed` },
      { status: 409 },
    );
  }
  if (plan.entries.length === 0) {
    return NextResponse.json(
      {
        error:
          plan.irreversible.length > 0
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
      reversible: plan.entries.length,
      irreversible: plan.irreversible.length,
    },
  });

  await enqueueCompensateRun({ runId: id, orgId });
  return NextResponse.json({ ok: true, reversing: plan.entries.length });
}
