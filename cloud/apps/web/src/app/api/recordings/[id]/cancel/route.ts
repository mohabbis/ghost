import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { cancelCompile } from "@/lib/recording-compiler";

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
    select: { id: true, compileStatus: true, harnessSessionId: true },
  });
  if (!recording) return Response.json({ error: "not found" }, { status: 404 });
  if (recording.compileStatus !== "RUNNING" || !recording.harnessSessionId) {
    return Response.json({ error: "no running compile to cancel" }, { status: 409 });
  }

  // Cancel is fire-and-forget here: HarnessRouter's own session status moves
  // to `cancelled`, and the next stream/detail poll (`finalizeIfTerminal`)
  // observes that and persists it — one code path for every terminal state
  // instead of a second one just for this button.
  await cancelCompile(recording.harnessSessionId);
  await appendAuditEvent(orgId, userId, {
    action: "recording.compile_cancel_requested",
    entityType: "Recording",
    entityId: recording.id,
  });

  return Response.json({ ok: true }, { status: 202 });
}
