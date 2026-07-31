import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";

/** Run detail for polling: run status + ordered steps + pending approvals. */
export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const run = await prisma.run.findFirst({
    where: { id, orgId: session.user.orgId },
    include: {
      steps: { orderBy: { index: "asc" } },
      approvals: { orderBy: { stepIndex: "asc" } },
      workflowVersion: { include: { workflow: { select: { name: true } } } },
    },
  });
  if (!run) return NextResponse.json({ error: "not found" }, { status: 404 });

  return NextResponse.json({
    id: run.id,
    status: run.status,
    error: run.error,
    workflowName: run.workflowVersion.workflow.name,
    startedAt: run.startedAt,
    endedAt: run.endedAt,
    // The step the run is stopped on, so an incident can offer retry/skip.
    cursor: run.cursor,
    steps: run.steps.map((s) => ({
      index: s.index,
      type: s.type,
      label: s.label,
      status: s.status,
      screenshotUrl: s.screenshotKey ? `/api/artifacts/${s.screenshotKey}` : null,
      verification: s.verification,
      error: s.error,
      attempt: s.attempt,
      output: s.output,
    })),
    approvals: run.approvals.map((a) => ({
      stepIndex: a.stepIndex,
      status: a.status,
      reason: a.reason,
      expiresAt: a.expiresAt,
    })),
  });
}
