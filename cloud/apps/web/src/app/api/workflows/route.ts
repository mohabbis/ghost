import { Prisma } from "@ghost/core/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { createWorkflowInput, formatIssues } from "@/lib/workflow-input";

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
  const { name, description, steps } = parsed.data;

  // Creation and its audit entry commit together. A workflow that exists with
  // no record of who authored it is a governance gap, not a cosmetic one.
  const workflow = await prisma.$transaction(async (tx) => {
    const created = await tx.workflow.create({
      data: {
        orgId,
        name,
        description: description || null,
        createdById: userId,
        versions: {
          create: {
            version: 1,
            note: "created",
            steps: steps as unknown as Prisma.InputJsonValue,
          },
        },
      },
    });

    await appendAuditEvent(
      orgId,
      userId,
      {
        action: "workflow.version_published",
        entityType: "Workflow",
        entityId: created.id,
        metadata: { version: 1, stepCount: steps.length },
      },
      tx,
    );

    return created;
  });

  return Response.json({ workflowId: workflow.id, version: 1 }, { status: 201 });
}
