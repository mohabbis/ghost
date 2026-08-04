import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { ingestTrace, MAX_TRACE_BYTES } from "@/lib/recording-ingest";

/**
 * Upload a workflow-recording trace and create the `Recording` it will be
 * reviewed from.
 *
 * A trace produced by a Ghost recorder (`@ghost/core/recording/trace`) is
 * compiled into typed steps deterministically, in this request — no model and
 * no configured compiler, which is what makes recording work in a production
 * deployment that has neither. Any other format (HAR, Playwright zip) is stored
 * and left for whatever compiler is configured, if one is.
 *
 * The extension posts to `/api/agent/recordings` instead, which accepts a
 * bearer credential; both share `ingestTrace`.
 */
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
    return Response.json({ error: 'a "file" upload is required' }, { status: 400 });
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

  const result = await ingestTrace({
    orgId,
    userId,
    filename: file.name || "recording-trace",
    contentType: file.type,
    buffer: Buffer.from(await file.arrayBuffer()),
  });

  return Response.json(result, { status: 201 });
}

export async function GET() {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const recordings = await prisma.recording.findMany({
    where: { orgId: session.user.orgId },
    orderBy: { createdAt: "desc" },
    take: 50,
    select: {
      id: true,
      createdAt: true,
      status: true,
      compileStatus: true,
      rawTraceFilename: true,
      workflowId: true,
    },
  });
  return Response.json({ recordings });
}
