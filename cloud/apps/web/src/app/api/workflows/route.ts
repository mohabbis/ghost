import { Prisma } from "@ghost/core/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { createWorkflowInput, formatIssues } from "@/lib/workflow-input";

/** Thrown inside the creation transaction to abort it without committing —
 * caught outside and turned into a 409, never allowed to look like the kind
 * of error that should retry or 500. */
class RecordingNotClaimableError extends Error {}

/**
 * Create a workflow and its first version from authored steps.
 *
 * Until this existed the only way to get a workflow was `POST
 * /api/workflows/demo`, which seeds one hardcoded definition — so Ghost could
 * execute exactly one workflow, the one it ships with.
 */
export async function POST(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;

  const parsed = createWorkflowInput.safeParse(await req.json().catch(() => null));
  if (!parsed.success) {
    return Response.json({ error: formatIssues(parsed.error) }, { status: 400 });
  }
  const { name, description, steps, recordingId } = parsed.data;

  // Creation and its audit entry commit together. A workflow that exists with
  // no record of who authored it is a governance gap, not a cosmetic one.
  const workflow = await prisma.$transaction(async (tx) => {
    // Org-scope + status-gate the recording *inside* the transaction: a
    // recording id alone must never link another tenant's proposal into this
    // workflow, and only a still-`READY` proposal (not already published,
    // not mid-recompile) may be claimed.
    if (recordingId) {
      const recording = await tx.recording.findFirst({
        where: { id: recordingId, orgId, compileStatus: "READY", workflowId: null },
        select: { id: true },
      });
      if (!recording) {
        throw new RecordingNotClaimableError();
      }
    }

    const created = await tx.workflow.create({
      data: {
        orgId,
        name,
        description: description || null,
        createdById: userId,
        versions: {
          create: {
            version: 1,
            note: recordingId ? "created from a compiled recording" : "created",
            steps: steps as unknown as Prisma.InputJsonValue,
          },
        },
      },
    });

    if (recordingId) {
      await tx.recording.update({
        where: { id: recordingId },
        data: { status: "COMPILED", workflowId: created.id },
      });
    }

    await appendAuditEvent(
      orgId,
      userId,
      {
        action: "workflow.version_published",
        entityType: "Workflow",
        entityId: created.id,
        metadata: { version: 1, stepCount: steps.length, recordingId: recordingId ?? null },
      },
      tx,
    );

    return created;
  }).catch((err) => {
    if (err instanceof RecordingNotClaimableError) return null;
    throw err;
  });

  if (!workflow) {
    return Response.json(
      { error: "recording not found, already published, or not ready" },
      { status: 409 },
    );
  }

  return Response.json({ workflowId: workflow.id, version: 1 }, { status: 201 });
}
