import Link from "next/link";
import { notFound } from "next/navigation";
import { parseWorkflowSteps } from "@ghost/core/schema/step";
import type { WorkflowSteps } from "@ghost/core/schema/step";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { WorkflowEditor } from "@/components/workflow-editor";

export const dynamic = "force-dynamic";

export default async function EditWorkflowPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const session = await auth();
  const orgId = session?.user.orgId;
  const { id } = await params;
  if (!orgId) notFound();

  // Org-scoped: another tenant's workflow id is indistinguishable from a
  // nonexistent one, rather than a 403 that confirms it exists.
  const workflow = await prisma.workflow.findFirst({
    where: { id, orgId },
    include: { versions: { orderBy: { version: "desc" }, take: 1 } },
  });
  if (!workflow) notFound();

  const latest = workflow.versions[0];

  // Steps stored before a schema change could fail to parse. Showing an empty
  // editor would silently discard them on the next save, so say so instead.
  let steps: WorkflowSteps | null = null;
  try {
    steps = latest ? parseWorkflowSteps(latest.steps) : [];
  } catch {
    steps = null;
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div>
        <Link href="/workflows" className="text-sm text-[var(--color-muted)] hover:underline">
          ← Workflows
        </Link>
        <h1 className="mt-2 text-xl font-semibold">{workflow.name}</h1>
        {workflow.description && (
          <p className="mt-1 text-sm text-[var(--color-muted)]">{workflow.description}</p>
        )}
      </div>

      {steps === null ? (
        <p role="alert" className="text-sm text-[var(--color-danger)]">
          Version {latest?.version} does not match the current step schema, so it cannot be edited
          here without discarding it. Its steps are unchanged and still runnable.
        </p>
      ) : (
        <WorkflowEditor
          workflowId={workflow.id}
          initialName={workflow.name}
          initialDescription={workflow.description ?? ""}
          initialSteps={steps}
          currentVersion={latest?.version ?? 0}
        />
      )}
    </div>
  );
}
