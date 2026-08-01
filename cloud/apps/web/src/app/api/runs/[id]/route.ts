import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { throttleReason } from "@ghost/core/concurrency";

/** Run detail for polling: run status + ordered steps + pending approvals. */
export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const run = await prisma.run.findFirst({
    where: { id, orgId: session.user.orgId },
    include: {
      steps: { orderBy: { index: "asc" } },
      approvals: { orderBy: { stepIndex: "asc" } },
      workflowVersion: { include: { workflow: { select: { name: true, maxActiveRuns: true } } } },
    },
  });
  if (!run) return NextResponse.json({ error: "not found" }, { status: 404 });

  // Evidence captured when the engine rebuilt the page after a halt. It is
  // written for the human reviewing an incident, so it has to be reachable
  // from the product — writing it and never surfacing it would defeat the
  // purpose of capturing it.
  const restoreEvent = await prisma.runEvent.findFirst({
    where: {
      runId: id,
      type: { in: ["session.restored", "session.restore_failed"] },
    },
    orderBy: { seq: "desc" },
    select: { type: true },
  });
  const restoreScreenshotUrl = restoreEvent
    ? `/api/artifacts/runs/${id}/restore-${run.cursor}.png`
    : null;

  // The exact actions a compensation gate is asking to authorize, as recorded
  // on `gate.opened`. Read from the journal rather than recomputed, so the
  // prompt shows what the worker committed to running rather than what a fresh
  // plan would produce now.
  const gateEvents = await prisma.runEvent.findMany({
    where: { runId: id, type: "gate.opened" },
    orderBy: { seq: "asc" },
    select: { stepIndex: true, payload: true },
  });
  const gateActions = new Map<string, string[]>();
  for (const e of gateEvents) {
    const payload = (e.payload ?? {}) as { phase?: string; actions?: string[] };
    if (payload.actions?.length) {
      gateActions.set(`${payload.phase ?? "FORWARD"}:${e.stepIndex}`, payload.actions);
    }
  }

  // "Queued" alone does not say whether Ghost is busy, broken, or deliberately
  // holding this run back behind its workflow's concurrency cap.
  //
  // The journal event records *that* the run was throttled; the holders are
  // read live. A run held for hours outlives the run that was blocking it when
  // the event was written, and linking an operator to a run that finished long
  // ago is worse than not linking at all.
  let throttle: { reason: string; activeRunIds: string[] } | null = null;
  if (run.status === "QUEUED" || (run.status === "COMPENSATING" && run.slotHeldAt === null)) {
    const throttled = await prisma.runEvent.findFirst({
      where: { runId: id, type: "run.throttled" },
      orderBy: { seq: "desc" },
      select: { seq: true },
    });
    // Only while the throttle is the run's latest word — an older one from
    // earlier in the run's life must not make a moving run look blocked.
    const newer = throttled
      ? await prisma.runEvent.count({ where: { runId: id, seq: { gt: throttled.seq } } })
      : 0;

    if (throttled && newer === 0) {
      const cap = run.workflowVersion.workflow.maxActiveRuns;
      const live = await prisma.run.findMany({
        where: {
          slotHeldAt: { not: null },
          workflowVersion: { workflowId: run.workflowVersion.workflowId },
        },
        orderBy: { slotHeldAt: "asc" },
        select: { id: true },
      });
      const activeRunIds = live.map((r) => r.id);
      throttle = {
        reason:
          cap === null
            ? "Waiting for a free slot."
            : throttleReason(cap, activeRunIds),
        activeRunIds,
      };
    }
  }

  return NextResponse.json({
    throttle,
    id: run.id,
    status: run.status,
    error: run.error,
    workflowName: run.workflowVersion.workflow.name,
    startedAt: run.startedAt,
    endedAt: run.endedAt,
    // The step the run is stopped on, so an incident can offer retry/skip.
    cursor: run.cursor,
    restoreScreenshotUrl,
    restoreOutcome: restoreEvent?.type ?? null,
    steps: run.steps.map((s) => ({
      index: s.index,
      type: s.type,
      label: s.label,
      status: s.status,
      screenshotUrl: s.screenshotKey ? `/api/artifacts/${s.screenshotKey}` : null,
      verification: s.verification,
      error: s.error,
      attempt: s.attempt,
      output: s.output,
    })),
    approvals: run.approvals.map((a) => ({
      stepIndex: a.stepIndex,
      status: a.status,
      reason: a.reason,
      expiresAt: a.expiresAt,
      // Which direction is waiting. Approving a reversal is a different
      // decision from approving the step it reverses, and the prompt has to
      // say which one is on the table.
      phase: a.phase,
      actions: gateActions.get(`${a.phase}:${a.stepIndex}`) ?? [],
    })),
  });
}
