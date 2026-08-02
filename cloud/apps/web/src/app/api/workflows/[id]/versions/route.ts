import { Prisma } from "@ghost/core/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { formatIssues, publishVersionInput } from "@/lib/workflow-input";
import type { WorkflowSteps } from "@ghost/core/schema/step";

/**
 * Publish the next version of a workflow.
 *
 * Editing appends a version rather than mutating one. `Run` pins a
 * `workflowVersionId`, so a run already in flight keeps executing the
 * definition it started with while an edit lands — the alternative is changing
 * the steps out from under a run that is halfway through a customer's system,
 * possibly while it waits at an approval gate a human is still reading.
 */
export async function POST(req: Request, context: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id } = await context.params;

  const parsed = publishVersionInput.safeParse(await req.json().catch(() => null));
  if (!parsed.success) {
    return Response.json({ error: formatIssues(parsed.error) }, { status: 400 });
  }
  const { steps, note } = parsed.data;

  let result: PublishResult;
  try {
    result = await publishVersion({ id, orgId, userId, steps, note });
  } catch (err) {
    // Two publishes racing both read the same latest version and both try to
    // create the next one. `@@unique([workflowId, version])` catches the loser
    // and the transaction rolls back, so nothing is half-written — but the
    // author deserves "retry", not a database error.
    if (err instanceof Prisma.PrismaClientKnownRequestError && err.code === "P2002") {
      return Response.json(
        { error: "another version was published; reload and retry" },
        { status: 409 },
      );
    }
    throw err;
  }

  if (!result.ok) return Response.json({ error: "workflow not found" }, { status: 404 });

  return Response.json({ workflowId: id, version: result.version }, { status: 201 });
}

type PublishResult = { ok: false } | { ok: true; version: number };

function publishVersion(input: {
  id: string;
  orgId: string;
  userId: string | null;
  steps: WorkflowSteps;
  note?: string;
}): Promise<PublishResult> {
  const { id, orgId, userId, steps, note } = input;

  return prisma.$transaction(async (tx) => {
    // Org-scope the read before the write: a workflow id alone must never be
    // enough to publish steps into another tenant's workflow.
    const workflow = await tx.workflow.findFirst({
      where: { id, orgId },
      select: { id: true },
    });
    if (!workflow) return { ok: false };

    const latest = await tx.workflowVersion.findFirst({
      where: { workflowId: id },
      orderBy: { version: "desc" },
      select: { version: true },
    });
    const version = (latest?.version ?? 0) + 1;

    await tx.workflowVersion.create({
      data: {
        workflowId: id,
        version,
        note: note || null,
        steps: steps as unknown as Prisma.InputJsonValue,
      },
    });

    // Touch the workflow so the list's `updatedAt` ordering reflects the edit.
    await tx.workflow.update({ where: { id }, data: { updatedAt: new Date() } });

    await appendAuditEvent(
      orgId,
      userId,
      {
        action: "workflow.version_published",
        entityType: "Workflow",
        entityId: id,
        metadata: { version, stepCount: steps.length },
      },
      tx,
    );

    return { ok: true, version };
  });
}
