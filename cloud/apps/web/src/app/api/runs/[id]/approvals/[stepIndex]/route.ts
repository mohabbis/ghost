import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueCompensateRun, enqueueRunWorkflow } from "@/lib/queue";
import { appendAuditEvent, appendRunEvent } from "@ghost/core/audit-log";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";

/**
 * Resolve a pending approval. Approving re-enqueues the run, which resumes at
 * the gated step — now with the approval recorded, so the state machine
 * executes it. Rejecting fails the run.
 *
 * The decision is written into both chains. Until now the single most
 * trust-critical fact in the product — *who authorized this payment* — lived
 * only in `Approval.resolvedById`, outside the tamper-evident log. An audit
 * trail that records what the machine did but not who told it to is not an
 * audit trail.
 */
export async function POST(
  req: Request,
  { params }: { params: Promise<{ id: string; stepIndex: string }> },
) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id, stepIndex } = await params;
  const index = Number(stepIndex);
  if (!Number.isInteger(index) || index < 0) {
    return NextResponse.json({ error: "bad step index" }, { status: 400 });
  }

  const body = (await req.json().catch(() => ({}))) as { decision?: string; note?: string };
  if (body.decision !== "approve" && body.decision !== "reject") {
    return NextResponse.json({ error: "decision must be approve|reject" }, { status: 400 });
  }

  const run = await prisma.run.findFirst({ where: { id, orgId } });
  if (!run) return NextResponse.json({ error: "not found" }, { status: 404 });

  const approve = body.decision === "approve";
  const now = new Date();

  // Which direction of the run is waiting. A gate opened while reversing the
  // run resumes the compensation job, not the forward one — approving the undo
  // of step N must never restart step N.
  const pending = await prisma.approval.findFirst({
    where: { runId: id, stepIndex: index, status: "PENDING" },
    select: { phase: true },
  });
  if (!pending) {
    return NextResponse.json({ error: "no pending approval for step" }, { status: 409 });
  }
  const compensating = pending.phase === "COMPENSATION";

  // One conditional update decides the race: a double-submitted approval
  // updates zero rows the second time and becomes a no-op, so only one resume
  // is ever enqueued.
  const resolved = await prisma.$transaction(async (tx) => {
    const { count } = await tx.approval.updateMany({
      where: { runId: id, stepIndex: index, phase: pending.phase, status: "PENDING" },
      data: {
        status: approve ? "APPROVED" : "REJECTED",
        resolvedById: userId,
        resolvedAt: now,
        note: body.note ?? null,
      },
    });
    if (count !== 1) return false;

    if (approve) {
      await tx.run.updateMany({
        where: { id, status: "AWAITING_APPROVAL" },
        data: { status: compensating ? "COMPENSATING" : "QUEUED" },
      });
    } else if (compensating) {
      // A refused reversal is not a failed run — the run already happened. It
      // stops as an incident so a human can decide what to do instead.
      await tx.run.update({
        where: { id },
        data: { status: "INCIDENT", error: `reversal rejected at step ${index}` },
      });
    } else {
      await tx.run.update({
        where: { id },
        data: { status: "FAILED", endedAt: now, error: `rejected at step ${index}` },
      });
    }

    await appendRunEvent(
      id,
      {
        type: RUN_EVENT_TYPES.gateResolved,
        stepIndex: index,
        // The phase is part of the record, not just of the lookup. Without it,
        // approving step 3 and approving the *reversal* of step 3 leave
        // identical rows, and an auditor reading the chain cannot tell which
        // mutation was authorized.
        payload: { decision: body.decision, resolvedById: userId, phase: pending.phase },
      },
      tx,
    );
    return true;
  });

  if (!resolved) {
    return NextResponse.json({ error: "no pending approval for step" }, { status: 409 });
  }

  await appendAuditEvent(orgId, userId, {
    action: approve ? "approval.approved" : "approval.rejected",
    entityType: "Approval",
    // Phase is part of the identity: `<run>:<step>` alone collides between the
    // forward approval for a step and the approval of that step's reversal.
    entityId: `${id}:${index}:${pending.phase}`,
    metadata: { stepIndex: index, runId: id, phase: pending.phase },
  });

  if (approve) {
    if (compensating) {
      // Through the compensation queue, not the run queue. `runWorkflowJob`
      // only leases QUEUED / RUNNING / AWAITING_APPROVAL runs, so a job sent
      // there for a COMPENSATING run returns immediately — and since the
      // original compensation job already ended at the gate, nothing would ever
      // pick the reversal back up and the run would sit in COMPENSATING
      // forever. The resume token matters for the same reason: the initial
      // compensation job is retained under the plain id and would swallow this.
      await enqueueCompensateRun({
        runId: id,
        orgId,
        requestedById: userId,
        resumeToken: `gate-${index}-${Date.now()}`,
      });
    } else {
      await enqueueRunWorkflow({ runId: id, orgId, fromStepIndex: index });
    }
    return NextResponse.json({ ok: true, resumed: true });
  }

  // Rejected: the run is terminal and will not resume, so the browser
  // credentials captured at the gate must go. The worker's terminal guard does
  // the deletion — the web app has no artifact store of its own.
  await enqueueRunWorkflow({
    runId: id,
    orgId,
    resumeToken: `cleanup-${Date.now()}`,
  }).catch(() => undefined);
  return NextResponse.json({ ok: true, resumed: false });
}
