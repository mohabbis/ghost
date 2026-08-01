import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { parseMaxActiveRuns } from "@ghost/core/concurrency";

/**
 * Update a workflow's settings. Currently only its concurrency cap.
 *
 * The cap is a governance control — it decides how much load Ghost is allowed
 * to put on a customer's system — so changing it is audited like any other
 * configuration change, with the old and new values recorded.
 */
export async function PATCH(req: Request, context: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const { id } = await context.params;

  const body = (await req.json().catch(() => ({}))) as { maxActiveRuns?: unknown };
  if (!("maxActiveRuns" in body)) {
    return Response.json({ error: "maxActiveRuns required" }, { status: 400 });
  }

  let maxActiveRuns: number | null;
  try {
    maxActiveRuns = parseMaxActiveRuns(body.maxActiveRuns);
  } catch (err) {
    return Response.json({ error: (err as Error).message }, { status: 400 });
  }

  // Org-scope the read before the write: a workflow id alone must never be
  // enough to reconfigure another tenant's workflow.
  const workflow = await prisma.workflow.findFirst({
    where: { id, orgId },
    select: { id: true, maxActiveRuns: true },
  });
  if (!workflow) return Response.json({ error: "workflow not found" }, { status: 404 });

  if (workflow.maxActiveRuns !== maxActiveRuns) {
    await prisma.workflow.update({ where: { id }, data: { maxActiveRuns } });
    await appendAuditEvent(orgId, session.user.id ?? null, {
      action: "workflow.concurrency_changed",
      entityType: "Workflow",
      entityId: id,
      metadata: { from: workflow.maxActiveRuns, to: maxActiveRuns },
    });
  }

  return Response.json({ id, maxActiveRuns });
}
