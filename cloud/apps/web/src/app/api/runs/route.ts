import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { startRun } from "@/lib/start-run";

/** Trigger a run of a workflow's latest version. */
export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;

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
    triggeredById: session.user.id,
  });
  if (!started.ok) {
    // 503, not 500: the request was fine and retrying is the right move once
    // the queue is reachable again.
    return NextResponse.json({ error: started.error, runId: started.runId }, { status: 503 });
  }

  return NextResponse.json({ runId: started.runId });
}
