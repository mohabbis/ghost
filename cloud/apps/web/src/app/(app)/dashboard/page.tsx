import Link from "next/link";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";

export const dynamic = "force-dynamic";

/**
 * Landing spot after sign-in. Previously a static "what Ghost does" list —
 * identical to the public landing page's copy and a dead end with no link
 * anywhere in the app. Now points a workflow-less org straight at the one
 * thing worth doing first: the demo run → approval → verify loop.
 */
export default async function DashboardPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const workflowCount = orgId ? await prisma.workflow.count({ where: { orgId, archived: false } }) : 0;

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Teach Ghost a workflow once — it executes it reliably, pauses for approval on
          sensitive steps, verifies the outcome, and logs what changed.
        </p>
      </div>

      {workflowCount === 0 ? (
        <Card>
          <CardHeader>
            <CardTitle>Try it</CardTitle>
          </CardHeader>
          <CardBody className="space-y-3">
            <p className="text-sm text-[var(--color-muted)]">
              Nothing here yet. Create Ghost&apos;s own demo workflow to watch a run pause
              for approval, execute, verify, and log — against Ghost&apos;s own fixture
              page, no external accounts needed.
            </p>
            <Link
              href="/workflows"
              className="inline-flex h-9 items-center justify-center rounded-lg bg-[var(--color-accent)] px-4 text-sm font-medium text-[var(--color-accent-fg)] transition-colors hover:opacity-90"
            >
              Go create it
            </Link>
          </CardBody>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>Continue</CardTitle>
          </CardHeader>
          <CardBody className="flex flex-wrap gap-2">
            <Link
              href="/workflows"
              className="inline-flex h-9 items-center justify-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-border)]/40"
            >
              Workflows
            </Link>
            <Link
              href="/runs"
              className="inline-flex h-9 items-center justify-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-border)]/40"
            >
              Runs
            </Link>
            <Link
              href="/audit"
              className="inline-flex h-9 items-center justify-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-border)]/40"
            >
              Audit log
            </Link>
          </CardBody>
        </Card>
      )}
    </div>
  );
}
