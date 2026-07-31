import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";
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

  // One conditional update decides the race: a double-submitted approval
  // updates zero rows the second time and becomes a no-op, so only one resume
  // is ever enqueued.
  const resolved = await prisma.$transaction(async (tx) => {
    const { count } = await tx.approval.updateMany({
      where: { runId: id, stepIndex: index, status: "PENDING" },
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
        data: { status: "QUEUED" },
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
        payload: { decision: body.decision, resolvedById: userId },
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
    entityId: `${id}:${index}`,
    metadata: { stepIndex: index, runId: id },
  });

  if (approve) {
    await enqueueRunWorkflow({ runId: id, orgId, fromStepIndex: index });
    return NextResponse.json({ ok: true, resumed: true });
  }
  return NextResponse.json({ ok: true, resumed: false });
}
