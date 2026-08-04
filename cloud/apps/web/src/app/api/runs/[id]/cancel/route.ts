import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { appendAuditEvent, appendRunEvent, runChainHead } from "@ghost/core/audit-log";
import { enqueueRunWorkflow } from "@/lib/queue";
import { RUN_EVENT_TYPES } from "@ghost/core/run-events";

/** Mirrors the worker's own lease TTL (`LEASE_MS` in `runWorkflow.ts`). */
const LEASE_MS = 60_000;

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
 *
 * The same applies to a run being reversed. A compensation is a sequence of
 * mutations against a live system, so it needs the same stop — checked between
 * reversals, with whatever has already been undone left undone and stated.
 *
 * Whoever finalizes (appends `run.canceled` and anchors the journal head into
 * the org chain) has to be certain no step is still being written concurrently
 * — otherwise a step commits after the anchor and the run's true final head no
 * longer matches what was sealed, which `/api/audit/verify` then reports as
 * tampering on a run nothing actually touched. This route is only certain of
 * that when no worker currently holds the run's lease: an active lease means a
 * worker owns this run right now and may be mid-step, so finalizing here would
 * race it exactly the way described above. In that case this route only flips
 * the status; the worker's own loop-top CANCELED check (`runWorkflow.ts`)
 * finalizes once its current step has actually committed, which is safe
 * because it's the same process doing both writes in order.
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
  const result = await prisma.$transaction(async (tx) => {
    const { count } = await tx.run.updateMany({
      where: {
        id,
        orgId,
        // COMPENSATING included: a reversal is a sequence of real mutations,
        // and an operator who sees a multi-step undo working the wrong records
        // needs the same stop the forward direction has. The compensation
        // worker polls this between reversals.
        status: { in: ["QUEUED", "RUNNING", "AWAITING_APPROVAL", "INCIDENT", "COMPENSATING"] },
      },
      data: {
        status: "CANCELED",
        endedAt: new Date(),
        error: `canceled by ${session.user.email ?? "a user"}`,
      },
    });
    if (count !== 1) return null;

    // A pending approval on a canceled run must not stay consumable.
    await tx.approval.updateMany({
      where: { runId: id, status: "PENDING" },
      data: { status: "REJECTED", resolvedById: userId, resolvedAt: new Date() },
    });

    // Read the lease *after* the status flip, in the same transaction, so the
    // decision below reflects the same moment the run actually became
    // CANCELED rather than a snapshot taken before it.
    const leaseRow = await tx.run.findUniqueOrThrow({
      where: { id },
      select: { leaseOwner: true, leaseExpiresAt: true },
    });
    const leaseActive =
      leaseRow.leaseOwner !== null &&
      leaseRow.leaseExpiresAt !== null &&
      leaseRow.leaseExpiresAt > new Date();

    if (leaseActive) {
      // A worker owns this run right now and may be mid-step. Do not append
      // or anchor here — see the function doc comment above.
      return { finalize: false as const };
    }

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
    return { finalize: true as const };
  });

  if (result === null) {
    return NextResponse.json({ error: "run is not cancelable" }, { status: 409 });
  }

  if (result.finalize) {
    // Anchor the journal head into the org chain, exactly as a worker-side
    // terminal transition does. Without this a run canceled before the engine
    // ever saw it again ended with an unanchored journal.
    await appendAuditEvent(orgId, userId, {
      action: "run.canceled",
      entityType: "Run",
      entityId: id,
      metadata: { runChainHead: await runChainHead(id) },
    });
  }

  // A canceled run may never re-enter the engine (it could still be QUEUED),
  // so nothing would otherwise delete the browser credentials captured at its
  // last gate. This job hits the worker's terminal guard, which purges them
  // and — when this route deferred finalizing above — also appends
  // `run.canceled` and anchors, once the lease has actually expired.
  //
  // Delaying the enqueue by the full lease TTL when a worker still owned the
  // run at cancel time gives it every chance to finish its in-flight step and
  // finalize itself first; the terminal guard's own lease check (see
  // `runWorkflow.ts`) is what makes it safe even if that race is somehow still
  // live when this job runs.
  await enqueueRunWorkflow(
    {
      runId: id,
      orgId,
      resumeToken: `cleanup-${Date.now()}`,
    },
    result.finalize ? undefined : { delayMs: LEASE_MS },
  ).catch(() => undefined);

  return NextResponse.json({ ok: true });
}
