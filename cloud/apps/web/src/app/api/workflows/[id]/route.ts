import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { parseMaxActiveRuns } from "@ghost/core/concurrency";
import { enqueueCompensateRun, enqueueRunWorkflow } from "@/lib/queue";

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

  const body = (await req.json().catch(() => ({}))) as {
    maxActiveRuns?: unknown;
    requireSeparateApprover?: unknown;
  };
  const setsCap = "maxActiveRuns" in body;
  const setsApprover = "requireSeparateApprover" in body;
  if (!setsCap && !setsApprover) {
    return Response.json(
      { error: "maxActiveRuns or requireSeparateApprover required" },
      { status: 400 },
    );
  }

  let maxActiveRuns: number | null = null;
  if (setsCap) {
    try {
      maxActiveRuns = parseMaxActiveRuns(body.maxActiveRuns);
    } catch (err) {
      return Response.json({ error: (err as Error).message }, { status: 400 });
    }
  }

  if (setsApprover && typeof body.requireSeparateApprover !== "boolean") {
    return Response.json({ error: "requireSeparateApprover must be a boolean" }, { status: 400 });
  }
  const requireSeparateApprover = body.requireSeparateApprover as boolean | undefined;

  // Read, write and audit in one transaction. As separate statements, a failed
  // audit append left the setting changed with no record of who changed it, and
  // two concurrent PATCHes could both read the same `from` value and write two
  // audit entries claiming the same starting point — a governance control whose
  // log does not match what happened.
  const result = await prisma.$transaction(async (tx) => {
    // Org-scope the read before the write: a workflow id alone must never be
    // enough to reconfigure another tenant's workflow.
    const workflow = await tx.workflow.findFirst({
      where: { id, orgId },
      select: { id: true, maxActiveRuns: true, requireSeparateApprover: true },
    });
    if (!workflow) return { ok: false as const };

    const from = workflow.maxActiveRuns;
    if (setsCap && from !== maxActiveRuns) {
      await tx.workflow.update({ where: { id }, data: { maxActiveRuns } });
      await appendAuditEvent(
        orgId,
        session.user.id ?? null,
        {
          action: "workflow.concurrency_changed",
          entityType: "Workflow",
          entityId: id,
          metadata: { from, to: maxActiveRuns },
        },
        tx,
      );
    }

    // Audited separately from the cap, and under its own action. Both are
    // governance settings but they answer different questions, and an auditor
    // reading "who removed the requirement for a second approver" should not
    // have to infer it from a concurrency event.
    if (setsApprover && workflow.requireSeparateApprover !== requireSeparateApprover) {
      await tx.workflow.update({ where: { id }, data: { requireSeparateApprover } });
      await appendAuditEvent(
        orgId,
        session.user.id ?? null,
        {
          action: "workflow.approval_policy_changed",
          entityType: "Workflow",
          entityId: id,
          metadata: {
            from: workflow.requireSeparateApprover,
            to: requireSeparateApprover,
          },
        },
        tx,
      );
    }

    return { ok: true as const, from };
  });

  if (!result.ok) return Response.json({ error: "workflow not found" }, { status: 404 });

  // Raising or clearing a cap creates capacity, and capacity nobody is told
  // about is not capacity: the runs already waiting would sit until each of
  // their five-minute re-checks fired. Best-effort — the setting is saved
  // either way, and the re-checks remain the backstop.
  //
  // Gated on `setsCap`: without it, a request that only flips the approval
  // policy leaves `maxActiveRuns` at its `null` default here, which reads as
  // "cap cleared" and would wake every waiting run for a cap nobody touched.
  const relaxed =
    setsCap &&
    result.from !== maxActiveRuns &&
    (maxActiveRuns === null || (result.from !== null && maxActiveRuns > result.from));
  if (relaxed) {
    await wakeWaitingRuns(id, maxActiveRuns).catch(() => undefined);
  }

  // Report the cap that is actually stored, not the local default.
  return Response.json({
    id,
    maxActiveRuns: setsCap ? maxActiveRuns : result.from,
    ...(setsApprover ? { requireSeparateApprover } : {}),
  });
}

/**
 * Offer the new capacity to the runs already waiting on it.
 *
 * Deliberately not a re-implementation of admission: it just knocks on the
 * door. Each woken run goes through `claimSlot` like any other, so waking more
 * than there is room for costs a job and nothing else — while waking too few
 * leaves a workflow stalled behind a cap that was already raised.
 *
 * Two shapes of waiting: a forward run sits in QUEUED, a refused reversal sits
 * in COMPENSATING with no slot, and they resume through different queues.
 */
async function wakeWaitingRuns(workflowId: string, cap: number | null): Promise<void> {
  const waiting = await prisma.run.findMany({
    where: {
      slotHeldAt: null,
      status: { in: ["QUEUED", "COMPENSATING"] },
      workflowVersion: { workflowId },
    },
    orderBy: { createdAt: "asc" },
    select: { id: true, orgId: true, status: true },
    take: cap ?? 25,
  });

  for (const run of waiting) {
    const token = `cap-raised-${Date.now()}-${run.id.slice(-6)}`;
    if (run.status === "COMPENSATING") {
      await enqueueCompensateRun({ runId: run.id, orgId: run.orgId, resumeToken: token });
    } else {
      await enqueueRunWorkflow({ runId: run.id, orgId: run.orgId, resumeToken: token });
    }
  }
}
