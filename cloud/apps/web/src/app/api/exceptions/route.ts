import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import {
  classifyException,
  duplicateRiskFor,
  kindsForOwner,
  EXCEPTION_KINDS,
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
  const ownerFilter = url.searchParams.get("owner") as ExceptionOwner | null;

  // Both filters are pushed into SQL, and both must admit `incidentKind: null`.
  //
  // Two bugs live here otherwise. Filtering `incidentKind = 'X'` in SQL silently
  // drops every incident whose kind was never stored — rows from before the
  // column existed — so those never reach the fallback classification below and
  // `?kind=UNKNOWN` omits exactly the incidents it should return. And applying an
  // *owner* filter after `take` means the cap selects the 200 longest-parked
  // incidents first: if those are all operator-owned, `?owner=author` returns
  // nothing while author-owned exceptions sit just past the cap, reporting a
  // `total` of 0 that reads as "none exist".
  //
  // `owner` is a pure function of `kind` (see kindsForOwner), so it becomes an IN
  // list rather than needing a stored owner column. Null-kind rows are admitted
  // here and filtered after classification.
  const wantedKinds: ExceptionKind[] | null = ownerFilter
    ? kindsForOwner(ownerFilter).filter((k) => !kindFilter || k === kindFilter)
    : kindFilter && (EXCEPTION_KINDS as readonly string[]).includes(kindFilter)
      ? [kindFilter as ExceptionKind]
      : null;

  const runs = await prisma.run.findMany({
    where: {
      orgId,
      status: "INCIDENT",
      ...(mine && session.user.id
        ? { incidentAssigneeId: session.user.id }
        : {}),
      ...(wantedKinds
        ? { OR: [{ incidentKind: { in: wantedKinds } }, { incidentKind: null }] }
        : {}),
    },
    // Longest-parked first, on when the incident was raised rather than when the
    // run was created — see the module comment.
    orderBy: [{ incidentRaisedAt: "asc" }, { createdAt: "asc" }],
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

    // Trust the stored kind for the label — it is what the engine decided when
    // the run stopped. Duplicate risk comes from the shared helper, which unions
    // the live verdict with the stored label and then still requires the step to
    // reach outside the browser, so an indeterminate *read* is not flagged as a
    // possibly-repeated effect.
    kind = kind ?? disposition.kind;
    const retryMayDuplicate = duplicateRiskFor({
      disposition,
      storedKind: kind,
      recordedOutcome: recorded?.status === "UNKNOWN" ? "UNKNOWN" : null,
      step,
    });

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
      // Prefer the recorded incident time; fall back to the step's end, then the
      // run's creation for rows predating `incidentRaisedAt`.
      stoppedAt: (
        run.incidentRaisedAt ??
        recorded?.endedAt ??
        run.createdAt
      ).toISOString(),
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

    // Post-filter, needed only for the null-kind rows admitted above: their kind
    // — and therefore their owner — is not known until classification.
    if (ownerFilter && shaped.owner !== ownerFilter) continue;
    if (kindFilter && shaped.kind !== kindFilter) continue;
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
