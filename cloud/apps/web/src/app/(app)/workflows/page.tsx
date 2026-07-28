import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";

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
      <div>
        <h1 className="text-xl font-semibold">Workflows</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Editable, versioned definitions Ghost executes step by step.
        </p>
      </div>

      {workflows.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">No workflows yet</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              Recording and the step editor land in Phase 2. For now this is the shell that
              lists an organization&apos;s workflows.
            </p>
          </CardBody>
        </Card>
      ) : (
        <div className="space-y-2">
          {workflows.map((w) => (
            <Card key={w.id}>
              <CardBody className="flex items-center justify-between">
                <div>
                  <div className="text-sm font-medium">{w.name}</div>
                  {w.description && (
                    <div className="text-sm text-[var(--color-muted)]">{w.description}</div>
                  )}
                </div>
                <div className="text-xs text-[var(--color-muted)]">
                  {w._count.versions} version{w._count.versions === 1 ? "" : "s"}
                </div>
              </CardBody>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
