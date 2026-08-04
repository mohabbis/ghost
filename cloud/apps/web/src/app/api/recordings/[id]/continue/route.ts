import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { compilerErrorResponse } from "@/lib/compiler";
import { prisma } from "@/lib/db";
import { continueCompile } from "@/lib/recording-compiler";

/** Resumes a compile that ended without producing steps, reusing the saved
 * job handle rather than starting a new task from scratch. */
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
    select: {
      id: true,
      compileStatus: true,
      compileJobId: true,
      compileSessionId: true,
    },
  });
  if (!recording) return Response.json({ error: "not found" }, { status: 404 });
  if (recording.compileStatus !== "FAILED" || !recording.compileJobId || !recording.compileSessionId) {
    return Response.json({ error: "nothing to continue for this recording" }, { status: 409 });
  }

  let job;
  try {
    job = await continueCompile({
      jobId: recording.compileJobId,
      sessionId: recording.compileSessionId,
    });
  } catch (err) {
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
      { action: "recording.compile_continued", entityType: "Recording", entityId: recording.id },
      tx,
    );
  });

  return Response.json({ ok: true }, { status: 202 });
}
