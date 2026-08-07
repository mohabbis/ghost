import Link from "next/link";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";
import {
  ConcurrencyCap,
  CreateDemoButton,
  RunButton,
  SeparateApprover,
} from "@/components/workflow-actions";

export const dynamic = "force-dynamic";

export default async function WorkflowsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const workflows = orgId
    ? await prisma.workflow.findMany({
        where: { orgId, archived: false },
        orderBy: { updatedAt: "desc" },
        include: { _count: { select: { versions: true } } },
      })
    : [];

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Workflows</h1>
          <p className="mt-1 text-sm text-[var(--color-muted)]">
            Editable, versioned definitions Ghost executes step by step.
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <CreateDemoButton />
          <Link
            href="/workflows/new"
            className="inline-flex h-8 items-center justify-center rounded-lg bg-[var(--color-accent)] px-3 text-sm font-medium text-[var(--color-accent-fg)] transition-colors hover:opacity-90"
          >
            New workflow
          </Link>
        </div>
      </div>

      {workflows.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">No workflows yet</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              Author one with the step editor, or create the demo workflow to try the
              run → approval → verify loop against Ghost&apos;s own fixture page.
            </p>
          </CardBody>
        </Card>
      ) : (
        <div className="space-y-2">
          {workflows.map((w) => (
            <Card key={w.id}>
              <CardBody className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
                <div>
                  <Link href={`/workflows/${w.id}`} className="text-sm font-medium hover:underline">
                    {w.name}
                  </Link>
                  {w.description && (
                    <div className="max-w-md truncate text-sm text-[var(--color-muted)]">
                      {w.description}
                    </div>
                  )}
                </div>
                <div className="flex flex-wrap shrink-0 items-center gap-3">
                  <span className="text-xs text-[var(--color-muted)]">
                    {w._count.versions} version{w._count.versions === 1 ? "" : "s"}
                  </span>
                  <SeparateApprover workflowId={w.id} value={w.requireSeparateApprover} />
                  <ConcurrencyCap workflowId={w.id} value={w.maxActiveRuns} />
                  <RunButton workflowId={w.id} />
                </div>
              </CardBody>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
