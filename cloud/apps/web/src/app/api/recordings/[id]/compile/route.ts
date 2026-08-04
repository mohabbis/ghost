import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { HarnessRouterError } from "@/lib/harness-router";
import { prisma } from "@/lib/db";
import { startCompile } from "@/lib/recording-compiler";

/** Starts (or restarts) the HarnessRouter compile task for one recording.
 * Refused while a compile is already `RUNNING` — restarting on top of a run
 * that is still going would start a second runtime task for the same
 * recording, which the setup guide is explicit about never doing. */
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

  let responseId: string;
  let sessionId: string;
  try {
    ({ responseId, sessionId } = await startCompile(recording));
  } catch (err) {
    // Surfaced as-is: a 402 ("out of credits") or 401 ("bad key") is
    // actionable by whoever owns the HarnessRouter Workspace, not a bug —
    // an uncaught throw here would otherwise reach the client as a bare,
    // unhelpful 500 with the message lost to Next's production error hiding.
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
      { action: "recording.compile_started", entityType: "Recording", entityId: recording.id },
      tx,
    );
  });

  return Response.json({ ok: true }, { status: 202 });
}
