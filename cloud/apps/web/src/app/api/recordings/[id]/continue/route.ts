import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { HarnessRouterError } from "@/lib/harness-router";
import { prisma } from "@/lib/db";
import { continueCompile, ensureRecordingCompilerHarness } from "@/lib/recording-compiler";

/** Resumes a compile session that ended without producing `steps.json`
 * (e.g. `incomplete`), reusing the saved session rather than starting a new
 * runtime task from scratch. */
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
      harnessResponseId: true,
      harnessSessionId: true,
    },
  });
  if (!recording) return Response.json({ error: "not found" }, { status: 404 });
  if (recording.compileStatus !== "FAILED" || !recording.harnessResponseId || !recording.harnessSessionId) {
    return Response.json({ error: "nothing to continue for this recording" }, { status: 409 });
  }

  let responseId: string;
  let sessionId: string;
  try {
    const harnessId = await ensureRecordingCompilerHarness();
    ({ responseId, sessionId } = await continueCompile(
      harnessId,
      recording.harnessSessionId,
      recording.harnessResponseId,
    ));
  } catch (err) {
    if (err instanceof HarnessRouterError) {
      const status = err.status >= 400 && err.status < 500 ? err.status : 502;
      return Response.json({ error: err.message }, { status });
    }
    throw err;
  }

  await prisma.$transaction(async (tx) => {
    await tx.recording.update({
      where: { id: recording.id },
      data: {
        compileStatus: "RUNNING",
        harnessResponseId: responseId,
        harnessSessionId: sessionId,
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
