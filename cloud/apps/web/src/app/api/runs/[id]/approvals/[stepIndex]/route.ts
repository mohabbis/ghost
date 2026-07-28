import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { enqueueRunWorkflow } from "@/lib/queue";

/**
 * Resolve a pending approval. Approving re-enqueues the run, which resumes at
 * the gated step (its `run.cursor` still points there) — now with the approval
 * recorded, so the state machine executes it. Rejecting fails the run.
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
  const { id, stepIndex } = await params;
  const index = Number(stepIndex);

  const body = (await req.json().catch(() => ({}))) as { decision?: string };
  if (body.decision !== "approve" && body.decision !== "reject") {
    return NextResponse.json({ error: "decision must be approve|reject" }, { status: 400 });
  }

  const run = await prisma.run.findFirst({ where: { id, orgId } });
  if (!run) return NextResponse.json({ error: "not found" }, { status: 404 });

  const approval = await prisma.approval.findFirst({
    where: { runId: id, stepIndex: index, status: "PENDING" },
  });
  if (!approval) {
    return NextResponse.json({ error: "no pending approval for step" }, { status: 409 });
  }

  if (body.decision === "approve") {
    await prisma.approval.update({
      where: { id: approval.id },
      data: { status: "APPROVED", resolvedById: session.user.id, resolvedAt: new Date() },
    });
    await prisma.run.update({ where: { id }, data: { status: "QUEUED" } });
    await enqueueRunWorkflow({ runId: id, orgId, fromStepIndex: index });
    return NextResponse.json({ ok: true, resumed: true });
  }

  await prisma.approval.update({
    where: { id: approval.id },
    data: { status: "REJECTED", resolvedById: session.user.id, resolvedAt: new Date() },
  });
  await prisma.run.update({
    where: { id },
    data: { status: "FAILED", endedAt: new Date(), error: `rejected at step ${index}` },
  });
  return NextResponse.json({ ok: true, resumed: false });
}
