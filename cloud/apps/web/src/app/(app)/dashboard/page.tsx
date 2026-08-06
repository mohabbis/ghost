import Link from "next/link";
import { ShieldAlert, AlertTriangle, ArrowRight } from "lucide-react";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { CreateDemoButton } from "@/components/workflow-actions";

export const dynamic = "force-dynamic";

/**
 * The org's actual state, not a description of the product.
 *
 * This page used to render a hardcoded list of five capability strings — the
 * first thing anyone saw after signing in was a bullet list that never changed
 * and never reflected anything. What a dashboard owes its reader is the answer
 * to "is anything waiting on me, and what happened recently"; the marketing
 * copy that used to live here now lives on the public landing page, where it
 * belongs.
 *
 * Everything below is scoped by `orgId` — a missing org means an empty page,
 * never another tenant's rows.
 */

/** Statuses that mean a human has to do something before the run can continue. */
const BLOCKED_STATUSES = ["AWAITING_APPROVAL", "INCIDENT"] as const;

const STATUS_TONE: Record<string, string> = {
  SUCCEEDED: "text-[var(--color-success)]",
  FAILED: "text-[var(--color-danger)]",
  INCIDENT: "text-[var(--color-danger)]",
  AWAITING_APPROVAL: "text-[var(--color-warning)]",
  RUNNING: "text-[var(--color-accent)]",
  COMPENSATED: "text-[var(--color-muted)]",
  COMPENSATING: "text-[var(--color-muted)]",
  CANCELED: "text-[var(--color-muted)]",
  QUEUED: "text-[var(--color-muted)]",
};

function relativeTime(date: Date): string {
  const seconds = Math.round((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

export default async function DashboardPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  if (!orgId) {
    return (
      <div className="mx-auto max-w-3xl">
        <Card>
          <CardBody className="py-12 text-center text-sm text-[var(--color-muted)]">
            No workspace is associated with this account yet.
          </CardBody>
        </Card>
      </div>
    );
  }

  const [pendingApprovals, blockedRuns, recentRuns, runTotals, workflowCount, auditCount] =
    await Promise.all([
      prisma.approval.findMany({
        where: { status: "PENDING", run: { orgId } },
        orderBy: { requestedAt: "asc" },
        take: 5,
        select: {
          id: true,
          runId: true,
          stepIndex: true,
          reason: true,
          requestedAt: true,
          expiresAt: true,
        },
      }),
      prisma.run.count({ where: { orgId, status: { in: [...BLOCKED_STATUSES] } } }),
      prisma.run.findMany({
        where: { orgId },
        orderBy: { createdAt: "desc" },
        take: 6,
        select: {
          id: true,
          status: true,
          createdAt: true,
          workflowVersion: {
            select: { version: true, workflow: { select: { name: true } } },
          },
        },
      }),
      prisma.run.groupBy({ by: ["status"], where: { orgId }, _count: { _all: true } }),
      prisma.workflow.count({ where: { orgId, archived: false } }),
      prisma.auditEvent.count({ where: { orgId } }),
    ]);

  const totalRuns = runTotals.reduce((sum, row) => sum + row._count._all, 0);
  const succeeded = runTotals.find((r) => r.status === "SUCCEEDED")?._count._all ?? 0;

  const stats = [
    { label: "Workflows", value: workflowCount, href: "/workflows" },
    { label: "Runs", value: totalRuns, href: "/runs" },
    { label: "Succeeded", value: succeeded, href: "/runs" },
    { label: "Audit entries", value: auditCount, href: "/audit" },
  ];

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          What&apos;s waiting on you, and what has run recently.
        </p>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Waiting on a human — the only section that is ever urgent.         */}
      {/* ----------------------------------------------------------------- */}
      {pendingApprovals.length > 0 && (
        <Card className="border-[var(--color-warning)]/50 bg-[var(--color-warning)]/[0.06]">
          <CardHeader className="border-[var(--color-warning)]/30">
            <CardTitle className="flex items-center gap-2 text-[var(--color-warning)]">
              <ShieldAlert className="h-4 w-4" />
              {pendingApprovals.length} step
              {pendingApprovals.length === 1 ? "" : "s"} waiting for approval
            </CardTitle>
          </CardHeader>
          <CardBody className="space-y-2">
            {pendingApprovals.map((a) => (
              <Link
                key={a.id}
                href={`/runs/${a.runId}`}
                className="flex items-center justify-between gap-4 rounded-lg bg-[var(--color-bg)] px-3 py-2.5 transition-colors hover:bg-[var(--color-bg)]/60"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">{a.reason}</div>
                  <div className="mt-0.5 font-mono text-[11px] text-[var(--color-muted)]">
                    step {a.stepIndex} · asked {relativeTime(a.requestedAt)}
                    {a.expiresAt ? ` · expires ${a.expiresAt.toLocaleString()}` : ""}
                  </div>
                </div>
                <ArrowRight className="h-4 w-4 shrink-0 text-[var(--color-muted)]" />
              </Link>
            ))}
          </CardBody>
        </Card>
      )}

      {blockedRuns > pendingApprovals.length && (
        <Card className="border-[var(--color-danger)]/40">
          <CardBody className="flex items-center gap-3 text-sm">
            <AlertTriangle className="h-4 w-4 shrink-0 text-[var(--color-danger)]" />
            <span>
              {blockedRuns} run{blockedRuns === 1 ? " is" : "s are"} stopped and need a decision.
            </span>
            <Link href="/runs" className="ml-auto shrink-0 text-sm font-medium hover:underline">
              Review
            </Link>
          </CardBody>
        </Card>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* Counts                                                            */}
      {/* ----------------------------------------------------------------- */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {stats.map(({ label, value, href }) => (
          <Link key={label} href={href}>
            <Card className="transition-colors hover:border-[var(--color-accent)]">
              <CardBody>
                <div className="text-2xl font-semibold tabular-nums">{value}</div>
                <div className="mt-0.5 text-xs text-[var(--color-muted)]">{label}</div>
              </CardBody>
            </Card>
          </Link>
        ))}
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Recent runs                                                       */}
      {/* ----------------------------------------------------------------- */}
      <Card>
        <CardHeader className="flex items-center justify-between">
          <CardTitle>Recent runs</CardTitle>
          {recentRuns.length > 0 && (
            <Link href="/runs" className="text-xs text-[var(--color-muted)] hover:underline">
              All runs
            </Link>
          )}
        </CardHeader>
        <CardBody className={recentRuns.length === 0 ? "py-10 text-center" : "space-y-1"}>
          {recentRuns.length === 0 ? (
            <>
              <p className="text-sm font-medium">Nothing has run yet</p>
              <p className="mx-auto mt-1 mb-4 max-w-md text-sm text-[var(--color-muted)]">
                Create the demo workflow to watch a run fill a form, stop before submitting, and
                wait for you to approve it.
              </p>
              <CreateDemoButton />
            </>
          ) : (
            recentRuns.map((run) => (
              <Link
                key={run.id}
                href={`/runs/${run.id}`}
                className="flex items-center justify-between gap-4 rounded-lg px-2 py-2 transition-colors hover:bg-[var(--color-bg)]"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm">
                    {run.workflowVersion.workflow.name}
                    <span className="ml-1.5 font-mono text-[11px] text-[var(--color-muted)]">
                      v{run.workflowVersion.version}
                    </span>
                  </div>
                  <div className="mt-0.5 text-[11px] text-[var(--color-muted)]">
                    {relativeTime(run.createdAt)}
                  </div>
                </div>
                <span
                  className={`shrink-0 font-mono text-xs ${
                    STATUS_TONE[run.status] ?? "text-[var(--color-muted)]"
                  }`}
                >
                  {run.status.toLowerCase().replace(/_/g, " ")}
                </span>
              </Link>
            ))
          )}
        </CardBody>
      </Card>
    </div>
  );
}
