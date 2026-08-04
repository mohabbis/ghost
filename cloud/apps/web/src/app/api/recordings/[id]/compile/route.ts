import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { compilerErrorResponse } from "@/lib/compiler";
import { prisma } from "@/lib/db";
import { startCompile } from "@/lib/recording-compiler";

/** Starts (or restarts) the compile task for one recording.
 * Refused while a compile is already `RUNNING` — restarting on top of a run
 * that is still going would start a second task for the same recording, which
 * the setup guide is explicit about never doing. */
export async function POST(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id } = await params;

  const recording = await prisma.recording.findFirst({
    where: { id, orgId },
    select: { id: true, orgId: true, rawTraceKey: true, rawTraceFilename: true, compileStatus: true },
  });
  if (!recording) return Response.json({ error: "not found" }, { status: 404 });
  if (recording.compileStatus === "RUNNING") {
    return Response.json({ error: "a compile is already running for this recording" }, { status: 409 });
  }
  if (!recording.rawTraceKey) {
    return Response.json({ error: "recording has no uploaded trace" }, { status: 400 });
  }

  let job;
  try {
    job = await startCompile(recording);
  } catch (err) {
    // 503 when no compiler is configured — the expected production state, not
    // a bug. Upstream 4xx passes through, since "out of credits" or "bad key"
    // is actionable by whoever runs the compiler; everything else is 502. An
    // uncaught throw would instead reach the client as a bare 500 with the
    // message lost to Next's production error hiding.
    const response = compilerErrorResponse(err);
    if (response) return response;
    throw err;
  }

  await prisma.$transaction(async (tx) => {
    await tx.recording.update({
      where: { id: recording.id },
      data: {
        compileStatus: "RUNNING",
        compileJobId: job.jobId,
        compileSessionId: job.sessionId,
        compileError: null,
      },
    });
    await appendAuditEvent(
      orgId,
      userId,
      { action: "recording.compile_started", entityType: "Recording", entityId: recording.id },
      tx,
    );
  });

  return Response.json({ ok: true }, { status: 202 });
}
