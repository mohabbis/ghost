import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import {
  classifyException,
  type ExceptionKind,
  type ExceptionOwner,
} from "@ghost/core/classifier/exception";

/**
 * The exception queue: every run in this org waiting on a human, oldest first.
 *
 * This is the surface that makes incidents a *product* rather than a state.
 * Before it, an incident was reachable only by knowing a run's id and opening
 * that run — fine when you triggered the run yourself thirty seconds ago,
 * useless as the way an ops team finds the twelve things that broke overnight.
 *
 * Oldest-first is deliberate and not a UI preference: an exception is work
 * someone is waiting on, and the run that has been parked longest is the one
 * most likely to have gone stale — a portal session that will need
 * re-authenticating, a batch whose downstream deadline is closest. Newest-first
 * would bury exactly the rows that need attention.
 *
 * ## Reads only
 *
 * Nothing here mutates or resumes anything. Resolution stays in
 * `POST /api/runs/[id]/incident`, which re-checks the run's state under its own
 * authorization. A queue that could also act would need every one of those
 * checks duplicated here.
 */

/** Cap on rows returned, so one org's bad night cannot fetch unboundedly. */
const MAX_ROWS = 200;

export interface ExceptionRow {
  runId: string;
  workflowName: string;
  workflowVersion: number;
  stepIndex: number;
  stepLabel: string | null;
  kind: ExceptionKind;
  owner: ExceptionOwner;
  headline: string;
  guidance: string;
  retryUseful: boolean;
  retryMayDuplicate: boolean;
  error: string | null;
  /**
   * When the run stopped, taken from the stopped-on step.
   *
   * NOT `Run.endedAt`: an INCIDENT is not terminal, so that column is still null
   * — reading it here produced a field that was always null. Falls back to the
   * run's creation time when the step row carries no end (a run that halted
   * before the step was ever written).
   */
  stoppedAt: string | null;
  assignee: { id: string; name: string | null; email: string | null } | null;
  triggeredBy: { name: string | null; email: string | null } | null;
}

export async function GET(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;

  const url = new URL(req.url);
  // `mine=1` narrows to the caller's own assignments — the "my work" view.
  const mine = url.searchParams.get("mine") === "1";
  const kindFilter = url.searchParams.get("kind");
  const ownerFilter = url.searchParams.get("owner");

  const runs = await prisma.run.findMany({
    where: {
      orgId,
      status: "INCIDENT",
      ...(mine && session.user.id
        ? { incidentAssigneeId: session.user.id }
        : {}),
      ...(kindFilter ? { incidentKind: kindFilter } : {}),
    },
    // Oldest first — see the module comment.
    orderBy: { createdAt: "asc" },
    take: MAX_ROWS,
    include: {
      workflowVersion: { include: { workflow: { select: { name: true } } } },
      incidentAssignee: { select: { id: true, name: true, email: true } },
      triggeredBy: { select: { name: true, email: true } },
    },
  });

  // The stopped-on step for every run in the page, in one query rather than one
  // per row. Prisma cannot filter a relation by a column of the parent row
  // (`index = run.cursor`), so the pairing is done here instead of in SQL — but
  // it is still a single round trip, which is the part that matters.
  const stepRows = await prisma.runStep.findMany({
    where: {
      runId: { in: runs.map((r) => r.id) },
      index: { in: [...new Set(runs.map((r) => r.cursor))] },
    },
    select: {
      runId: true,
      index: true,
      status: true,
      label: true,
      endedAt: true,
    },
  });
  const stoppedStep = new Map(
    stepRows.map((s) => [`${s.runId}:${s.index}`, s]),
  );

  const rows: ExceptionRow[] = [];

  for (const run of runs) {
    // The stored `incidentKind` is the classification made when the run
    // stopped, and it is what an auditor should see. Re-classify only when it is
    // absent — a run that predates this column, or one whose incident was
    // raised by an older worker. Never overwrite the stored value on read.
    let kind = run.incidentKind as ExceptionKind | null;

    // The step is needed for the disposition text either way, and for duplicate
    // risk when re-classifying. A malformed stored version must not take the
    // whole queue down with it, so parse defensively.
    let step;
    try {
      step = parseWorkflowSteps(run.workflowVersion.steps)[run.cursor];
    } catch {
      step = undefined;
    }

    const recorded = stoppedStep.get(`${run.id}:${run.cursor}`) ?? null;

    const disposition = classifyException({
      reason: run.error ?? "",
      step,
      recordedOutcome:
        recorded?.status === "UNKNOWN"
          ? "UNKNOWN"
          : recorded?.status === "FAILED"
            ? "FAILED"
            : null,
    });

    // Trust the stored kind for the label, but take the *safety* flag from the
    // live computation. If the two ever disagree about duplicate risk, the
    // cautious answer wins: a stale stored kind must not be able to downgrade a
    // warning that the current classifier would raise.
    kind = kind ?? disposition.kind;
    const retryMayDuplicate =
      disposition.retryMayDuplicate ||
      kind === "OUTCOME_UNKNOWN" ||
      recorded?.status === "UNKNOWN";

    const shaped: ExceptionRow = {
      runId: run.id,
      workflowName: run.workflowVersion.workflow.name,
      workflowVersion: run.workflowVersion.version,
      stepIndex: run.cursor,
      stepLabel: recorded?.label ?? step?.label ?? step?.type ?? null,
      kind,
      owner: disposition.owner,
      headline: disposition.headline,
      guidance: disposition.guidance,
      retryUseful: disposition.retryUseful,
      retryMayDuplicate,
      error: run.error,
      stoppedAt: (recorded?.endedAt ?? run.createdAt).toISOString(),
      assignee: run.incidentAssignee
        ? {
            id: run.incidentAssignee.id,
            name: run.incidentAssignee.name,
            email: run.incidentAssignee.email,
          }
        : null,
      triggeredBy: run.triggeredBy
        ? { name: run.triggeredBy.name, email: run.triggeredBy.email }
        : null,
    };

    if (ownerFilter && shaped.owner !== ownerFilter) continue;
    rows.push(shaped);
  }

  // Counts are computed over the returned rows, so they describe exactly what
  // the caller is looking at rather than a different, unfiltered population.
  const byOwner: Record<string, number> = {};
  const byKind: Record<string, number> = {};
  for (const r of rows) {
    byOwner[r.owner] = (byOwner[r.owner] ?? 0) + 1;
    byKind[r.kind] = (byKind[r.kind] ?? 0) + 1;
  }

  return NextResponse.json({
    total: rows.length,
    truncated: runs.length === MAX_ROWS,
    byOwner,
    byKind,
    exceptions: rows,
  });
}
