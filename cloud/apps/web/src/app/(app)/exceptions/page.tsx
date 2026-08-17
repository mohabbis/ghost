import Link from "next/link";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import {
  classifyException,
  duplicateRiskFor,
  type ExceptionKind,
  type ExceptionOwner,
} from "@ghost/core/classifier/exception";
import { ExceptionAssignee } from "@/components/exception-assignee";

export const dynamic = "force-dynamic";

/**
 * The exception queue — the work an ops team actually does.
 *
 * Ghost's promise is that it involves a human only when necessary. That promise
 * is only kept if the necessary involvement is *findable*: before this page an
 * incident could only be reached by knowing a run's id, which meant the failure
 * mode of a 40-invoice batch was twelve parked runs nobody knew about.
 *
 * Grouped by owner rather than by workflow or time, because the owner is the
 * routing decision: a changed selector is the workflow author's problem, an
 * expired credential is an administrator's, and a rejected value is the
 * operator's. Sorting by anything else makes every reader re-triage the list.
 */

const OWNER_COPY: Record<ExceptionOwner, { title: string; blurb: string }> = {
  operator: {
    title: "For an operator",
    blurb:
      "Needs a judgement call about the work itself, or a look at the target system.",
  },
  author: {
    title: "For the workflow author",
    blurb:
      "The workflow no longer matches the software it drives. Retrying will fail the same way.",
  },
  administrator: {
    title: "For an administrator",
    blurb:
      "Credentials, permissions, or approvals need attention before these can proceed.",
  },
};

const OWNER_ORDER: ExceptionOwner[] = ["operator", "author", "administrator"];

function kindTone(kind: ExceptionKind): string {
  // OUTCOME_UNKNOWN is the only kind that carries a risk of repeating an effect,
  // so it is the only one styled as a danger rather than a warning. If
  // everything is red, the one that matters stops standing out.
  if (kind === "OUTCOME_UNKNOWN") return "text-[var(--color-danger)]";
  if (kind === "TARGET_MISSING" || kind === "AUTH")
    return "text-[var(--color-warning)]";
  return "text-[var(--color-muted)]";
}

function age(from: Date): string {
  const mins = Math.floor((Date.now() - from.getTime()) / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

export default async function ExceptionsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const runs = orgId
    ? await prisma.run.findMany({
        where: { orgId, status: "INCIDENT" },
        // Longest-parked first, on when the incident was raised — a run's own age
        // says nothing about how long it has been waiting for someone.
        orderBy: [{ incidentRaisedAt: "asc" }, { createdAt: "asc" }],
        take: 200,
        include: {
          workflowVersion: {
            include: { workflow: { select: { name: true } } },
          },
          incidentAssignee: { select: { id: true, name: true, email: true } },
        },
      })
    : [];

  // One batched lookup for the stopped-on step of every parked run, rather than
  // a query per row.
  const stepRows =
    runs.length > 0
      ? await prisma.runStep.findMany({
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
        })
      : [];
  const stopped = new Map(stepRows.map((s) => [`${s.runId}:${s.index}`, s]));

  const members = orgId
    ? await prisma.membership.findMany({
        where: { orgId },
        select: { user: { select: { id: true, name: true, email: true } } },
      })
    : [];
  const assignable = members.map((m) => m.user);

  const rows = runs.map((run) => {
    let step;
    try {
      step = parseWorkflowSteps(run.workflowVersion.steps)[run.cursor];
    } catch {
      step = undefined;
    }
    const recorded = stopped.get(`${run.id}:${run.cursor}`) ?? null;
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
    // Stored kind is what the engine decided at the time and is what an auditor
    // should see; fall back to the live computation only for runs raised before
    // the column existed. Duplicate risk always takes the cautious answer.
    const kind = (run.incidentKind as ExceptionKind | null) ?? disposition.kind;
    return {
      run,
      kind,
      owner: disposition.owner,
      headline: disposition.headline,
      guidance: disposition.guidance,
      retryMayDuplicate: duplicateRiskFor({
        disposition,
        storedKind: kind,
        recordedOutcome: recorded?.status === "UNKNOWN" ? "UNKNOWN" : null,
        step,
      }),
      stepLabel: recorded?.label ?? step?.label ?? step?.type ?? null,
      // When the incident was raised. `Run.endedAt` stays null for an INCIDENT
      // (it is not terminal), and `createdAt` is the run's age, not its wait.
      stoppedAt: run.incidentRaisedAt ?? recorded?.endedAt ?? run.createdAt,
    };
  });

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Exceptions</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Runs that stopped and need a person. Oldest first, grouped by who can
          resolve them.
        </p>
      </div>

      {rows.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">Nothing waiting</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              No run is parked. Exceptions appear here the moment one stops and
              cannot safely continue on its own.
            </p>
          </CardBody>
        </Card>
      ) : (
        OWNER_ORDER.map((owner) => {
          const group = rows.filter((r) => r.owner === owner);
          if (group.length === 0) return null;
          const copy = OWNER_COPY[owner];
          return (
            <section key={owner} className="space-y-2">
              <div>
                <h2 className="text-sm font-medium">
                  {copy.title}
                  <span className="ml-2 font-normal text-[var(--color-muted)]">
                    {group.length}
                  </span>
                </h2>
                <p className="text-xs text-[var(--color-muted)]">
                  {copy.blurb}
                </p>
              </div>

              {group.map(
                ({
                  run,
                  kind,
                  headline,
                  guidance,
                  retryMayDuplicate,
                  stepLabel,
                  stoppedAt,
                }) => (
                  <Card key={run.id}>
                    <CardBody className="space-y-2">
                      <div className="flex flex-wrap items-baseline justify-between gap-2">
                        <Link
                          href={`/runs/${run.id}`}
                          className="text-sm font-medium hover:text-[var(--color-accent)]"
                        >
                          {run.workflowVersion.workflow.name}
                          <span className="ml-1.5 font-normal text-[var(--color-muted)]">
                            v{run.workflowVersion.version}
                          </span>
                        </Link>
                        <span className="text-xs text-[var(--color-muted)]">
                          stopped {age(stoppedAt)} ago
                        </span>
                      </div>

                      <div className="text-xs">
                        <span className={kindTone(kind)}>{headline}</span>
                        <span className="text-[var(--color-muted)]">
                          {" — step "}
                          {run.cursor + 1}
                          {stepLabel ? ` · ${stepLabel}` : ""}
                        </span>
                      </div>

                      <p className="text-xs text-[var(--color-muted)]">
                        {guidance}
                      </p>

                      {retryMayDuplicate && (
                        <p className="text-xs font-medium text-[var(--color-danger)]">
                          This step&apos;s effect may already have happened.
                          Check the target system before retrying.
                        </p>
                      )}

                      {run.error && (
                        <pre className="overflow-x-auto rounded border border-[var(--color-border)] bg-[var(--color-bg)] p-2 font-mono text-[11px] text-[var(--color-muted)]">
                          {run.error}
                        </pre>
                      )}

                      <ExceptionAssignee
                        runId={run.id}
                        assignee={run.incidentAssignee}
                        members={assignable}
                      />
                    </CardBody>
                  </Card>
                ),
              )}
            </section>
          );
        })
      )}
    </div>
  );
}
