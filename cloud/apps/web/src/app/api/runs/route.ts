import { NextResponse } from "next/server";
import { canStartRun } from "@ghost/core/roles";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { loadActor } from "@/lib/members";
import { startRun } from "@/lib/start-run";

/** Trigger a run of a workflow's latest version. */
export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id;
  // Re-read role from the DB; JWT says nothing about a demotion since sign-in.
  // Today every Role may start a run; the check is here so a future VIEWER
  // cannot slip through when canStartRun tightens.
  const actor = await loadActor(orgId, userId);
  if (!actor || !canStartRun(actor.role)) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }

  const body = (await req.json().catch(() => ({}))) as { workflowId?: string };
  if (!body.workflowId) {
    return NextResponse.json({ error: "workflowId required" }, { status: 400 });
  }

  // Org-scope the lookup: only versions of workflows this org owns.
  const version = await prisma.workflowVersion.findFirst({
    where: { workflowId: body.workflowId, workflow: { orgId } },
    orderBy: { version: "desc" },
  });
  if (!version) {
    return NextResponse.json({ error: "workflow not found" }, { status: 404 });
  }

  const started = await startRun({
    orgId,
    workflowVersionId: version.id,
    triggeredById: userId,
  });
  if (!started.ok) {
    // 503, not 500: the request was fine and retrying is the right move once
    // the queue is reachable again.
    return NextResponse.json({ error: started.error, runId: started.runId }, { status: 503 });
  }

  return NextResponse.json({ runId: started.runId });
}
