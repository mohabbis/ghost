import Link from "next/link";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";

export const dynamic = "force-dynamic";

export default async function RunsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const runs = orgId
    ? await prisma.run.findMany({
        where: { orgId },
        orderBy: { createdAt: "desc" },
        take: 50,
      })
    : [];

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Runs</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Every execution, with per-step status, screenshots, verification, and approvals.
        </p>
      </div>

      {runs.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">No runs yet</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              Create the demo workflow and hit Run to see a live run here.
            </p>
          </CardBody>
        </Card>
      ) : (
        <div className="space-y-2">
          {runs.map((r) => (
            <Link key={r.id} href={`/runs/${r.id}`} className="block">
              <Card className="transition-colors hover:border-[var(--color-accent)]">
                <CardBody className="flex items-center justify-between">
                  <div className="font-mono text-xs text-[var(--color-muted)]">{r.id}</div>
                  <div className="text-xs">{r.status}</div>
                </CardBody>
              </Card>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
