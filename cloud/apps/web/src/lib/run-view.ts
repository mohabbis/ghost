import { prisma } from "@/lib/db";
import { canApproveRun } from "@ghost/core/roles";
import { throttleReason } from "@ghost/core/concurrency";
import { loadActor } from "@/lib/members";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import { classifyException, type ExceptionKind } from "@ghost/core/classifier/exception";

/**
 * Build the run detail view: status, ordered steps, pending approvals, and
 * the handful of derived fields (throttle reason, restore evidence, gate
 * actions) the timeline needs to render without a second round trip.
 *
 * Shared between `GET /api/runs/[id]` (a single fetch) and
 * `GET /api/runs/[id]/stream` (an SSE poll loop) so the two never drift —
 * they used to be one inline handler, and duplicating this by hand across a
 * poll loop would be exactly the kind of copy that silently diverges the
 * first time either one gets a bug fix.
 */
export async function buildRunView(orgId: string, viewerId: string, runId: string) {
  const run = await prisma.run.findFirst({
    where: { id: runId, orgId },
    include: {
      steps: { orderBy: { index: "asc" } },
      approvals: { orderBy: { stepIndex: "asc" } },
      workflowVersion: {
        include: {
          workflow: {
            select: { name: true, maxActiveRuns: true, requireSeparateApprover: true },
          },
        },
      },
    },
  });
  if (!run) return null;

  // Evidence captured when the engine rebuilt the page after a halt. It is
  // written for the human reviewing an incident, so it has to be reachable
  // from the product — writing it and never surfacing it would defeat the
  // purpose of capturing it.
  const restoreEvent = await prisma.runEvent.findFirst({
    where: {
      runId,
      type: { in: ["session.restored", "session.restore_failed"] },
    },
    orderBy: { seq: "desc" },
    select: { type: true },
  });
  const restoreScreenshotUrl = restoreEvent
    ? `/api/artifacts/runs/${runId}/restore-${run.cursor}.png`
    : null;

  // The exact actions a compensation gate is asking to authorize, as recorded
  // on `gate.opened`. Read from the journal rather than recomputed, so the
  // prompt shows what the worker committed to running rather than what a fresh
  // plan would produce now.
  const gateEvents = await prisma.runEvent.findMany({
    where: { runId, type: "gate.opened" },
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

  // Role is re-read from the DB (not the JWT) so a demotion takes effect on
  // the next poll rather than lasting until the session expires.
  const actor = await loadActor(orgId, viewerId);
  const roleAllowsApprove = actor !== null && canApproveRun(actor.role);

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
      where: { runId, type: "run.throttled" },
      orderBy: { seq: "desc" },
      select: { seq: true },
    });
    // Only while the throttle is the run's latest word — an older one from
    // earlier in the run's life must not make a moving run look blocked.
    const newer = throttled
      ? await prisma.runEvent.count({ where: { runId, seq: { gt: throttled.seq } } })
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
        reason: cap === null ? "Waiting for a free slot." : throttleReason(cap, activeRunIds),
        activeRunIds,
      };
    }
  }

  // Exception disposition, for an INCIDENT run only. Computed here so the
  // timeline can lead with what kind of problem this is and whether retrying
  // risks repeating an effect, instead of a raw driver error and two
  // equal-weight buttons.
  //
  // The stored `incidentKind` is what the engine decided when it stopped and is
  // what an auditor should see; the live computation supplies the guidance text
  // and — always — the cautious answer on duplicate risk, so a stale stored
  // label can never downgrade a warning.
  let exception: {
    kind: ExceptionKind;
    owner: string;
    headline: string;
    guidance: string;
    retryUseful: boolean;
    retryMayDuplicate: boolean;
  } | null = null;

  if (run.status === "INCIDENT") {
    let stoppedStep;
    try {
      stoppedStep = parseWorkflowSteps(run.workflowVersion.steps)[run.cursor];
    } catch {
      stoppedStep = undefined;
    }
    const recorded = run.steps.find((s) => s.index === run.cursor);
    const d = classifyException({
      reason: run.error ?? "",
      step: stoppedStep,
      recordedOutcome:
        recorded?.status === "UNKNOWN" ? "UNKNOWN" : recorded?.status === "FAILED" ? "FAILED" : null,
    });
    const kind = (run.incidentKind as ExceptionKind | null) ?? d.kind;
    exception = {
      kind,
      owner: d.owner,
      headline: d.headline,
      guidance: d.guidance,
      retryUseful: d.retryUseful,
      retryMayDuplicate:
        d.retryMayDuplicate || kind === "OUTCOME_UNKNOWN" || recorded?.status === "UNKNOWN",
    };
  }

  return {
    throttle,
    exception,
    id: run.id,
    status: run.status,
    error: run.error,
    workflowName: run.workflowVersion.workflow.name,
    // Whether *this viewer* may approve. Computed here rather than shipping the
    // policy and the triggerer's id to the client and asking it to decide: the
    // route is the authority either way, and this keeps who-started-what out of
    // a polling payload. OWNER/ADMIN only, plus SoD when the workflow opts in.
    canApprove:
      roleAllowsApprove &&
      (!run.workflowVersion.workflow.requireSeparateApprover ||
        run.triggeredById === null ||
        run.triggeredById !== viewerId),
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
      // Extract values stay in the run journal (needed for {{ }} refs) but are
      // not shipped to the browser — cleartext PII/extracted secrets must not
      // sit in every poll of the timeline (P1-4 mitigation).
      output: null,
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
  };
}

export type RunView = NonNullable<Awaited<ReturnType<typeof buildRunView>>>;

/** Statuses the timeline stops polling/streaming at. Mirrors run-timeline.tsx. */
export const RUN_TERMINAL_STATUSES = new Set(["SUCCEEDED", "FAILED", "CANCELED", "COMPENSATED"]);
