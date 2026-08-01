import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { appendAuditEvent, appendRunEvent, runChainHead } from "@ghost/core/audit-log";
import { enqueueRunWorkflow } from "@/lib/queue";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";

/**
 * Stop a run.
 *
 * The worker has always polled for `CANCELED` before each step, but nothing in
 * the codebase ever set it — the check was dead code and there was no way to
 * stop a run.
 *
 * Note what this does and does not promise. The worker checks between steps, so
 * a step already in flight runs to completion. For a step whose effect must not
 * happen twice, that is the honest behaviour: a cancel that claimed to stop a
 * payment mid-flight would be the same class of lie as a silent double-send.
 * The UI says "stop after the current step" for that reason.
 */
export async function POST(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const userId = session.user.id ?? null;
  const { id } = await params;

  // Status change, approval invalidation and the journal entry commit together.
  // As three separate writes this had two problems: a queued run canceled here
  // never reaches the worker's cancel branch, so its journal was left
  // unanchored; and for a *running* job the worker could observe CANCELED,
  // append its own event and anchor, after which this route appended another —
  // leaving the sealed head no longer matching the journal, which reads as
  // tampering. One transaction, and an append that is a no-op if the worker got
  // there first, removes both.
  const canceled = await prisma.$transaction(async (tx) => {
    const { count } = await tx.run.updateMany({
      where: {
        id,
        orgId,
        status: { in: ["QUEUED", "RUNNING", "AWAITING_APPROVAL", "INCIDENT"] },
      },
      data: {
        status: "CANCELED",
        endedAt: new Date(),
        error: `canceled by ${session.user.email ?? "a user"}`,
      },
    });
    if (count !== 1) return false;

    // A pending approval on a canceled run must not stay consumable.
    await tx.approval.updateMany({
      where: { runId: id, status: "PENDING" },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });

    const already = await tx.runEvent.findFirst({
      where: { runId: id, type: RUN_EVENT_TYPES.runCanceled },
      select: { id: true },
    });
    if (!already) {
      await appendRunEvent(
        id,
        { type: RUN_EVENT_TYPES.runCanceled, payload: { canceledById: userId } },
        tx,
      );
    }
    return true;
  });

  if (!canceled) {
    return NextResponse.json({ error: "run is not cancelable" }, { status: 409 });
  }

  // Anchor the journal head into the org chain, exactly as a worker-side
  // terminal transition does. Without this a run canceled before the engine
  // ever saw it again ended with an unanchored journal.
  await appendAuditEvent(orgId, userId, {
    action: "run.canceled",
    entityType: "Run",
    entityId: id,
    metadata: { runChainHead: await runChainHead(id) },
  });

  // A canceled run may never re-enter the engine (it could still be QUEUED),
  // so nothing would otherwise delete the browser credentials captured at its
  // last gate. This job hits the worker's terminal guard, which purges them.
  await enqueueRunWorkflow({
    runId: id,
    orgId,
    resumeToken: `cleanup-${Date.now()}`,
  }).catch(() => undefined);

  return NextResponse.json({ ok: true });
}
