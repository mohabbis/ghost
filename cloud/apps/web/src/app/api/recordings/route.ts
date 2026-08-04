import { appendAuditEvent } from "@ghost/core/audit-log";
import { artifactStore } from "@ghost/core/storage/artifacts";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";

/** Recording traces are small event/HAR/trace-zip logs, not raw video — cap
 * well under Vercel's 100MB function body limit to keep uploads fast and
 * keep an accidental video upload from tying up the compile step. */
const MAX_TRACE_BYTES = 25 * 1024 * 1024;

function sanitizeFilename(name: string): string {
  const base = name.split(/[/\\]/).pop() || "recording-trace";
  return base.replace(/[^a-zA-Z0-9._-]/g, "_").slice(-200) || "recording-trace";
}

/** Upload a raw workflow-recording trace and create the `Recording` it will
 * be compiled from. The capture mechanism itself (server-side remote browser
 * vs. extension — see `docs/CURSOR_HANDOFF.md` Phase 2) is still an open
 * decision; this accepts a trace produced by whatever means already exists
 * (a JSON event log, a HAR export, a Playwright trace .zip). */
export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;

  const form = await req.formData().catch(() => null);
  const file = form?.get("file");
  if (!file || !(file instanceof File)) {
    return Response.json({ error: "a \"file\" upload is required" }, { status: 400 });
  }
  if (file.size === 0) {
    return Response.json({ error: "the uploaded file is empty" }, { status: 400 });
  }
  if (file.size > MAX_TRACE_BYTES) {
    return Response.json(
      { error: `trace file exceeds the ${MAX_TRACE_BYTES / (1024 * 1024)}MB limit` },
      { status: 413 },
    );
  }

  const filename = sanitizeFilename(file.name || "recording-trace");
  const buffer = Buffer.from(await file.arrayBuffer());

  const recording = await prisma.recording.create({
    data: { orgId, status: "STOPPED" },
  });

  try {
    const key = `recordings/${recording.id}/trace-${filename}`;
    await artifactStore().put(key, buffer, file.type || "application/octet-stream");
    await prisma.$transaction(async (tx) => {
      await tx.recording.update({
        where: { id: recording.id },
        data: { rawTraceKey: key, rawTraceFilename: filename },
      });
      await appendAuditEvent(
        orgId,
        userId,
        {
          action: "recording.uploaded",
          entityType: "Recording",
          entityId: recording.id,
          metadata: { filename, bytes: buffer.byteLength },
        },
        tx,
      );
    });
  } catch (err) {
    // Storage failed after the row was created — don't leave an unusable
    // Recording with no trace behind for the org to trip over.
    await prisma.recording.delete({ where: { id: recording.id } }).catch(() => undefined);
    throw err;
  }

  return Response.json({ id: recording.id }, { status: 201 });
}

export async function GET() {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }

  const recordings = await prisma.recording.findMany({
    where: { orgId: session.user.orgId },
    orderBy: { createdAt: "desc" },
    select: {
      id: true,
      status: true,
      compileStatus: true,
      rawTraceFilename: true,
      workflowId: true,
      createdAt: true,
    },
  });

  return Response.json({ recordings });
}
