import Link from "next/link";
import { WorkflowEditor } from "@/components/workflow-editor";

export default function NewWorkflowPage() {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div>
        <Link href="/workflows" className="text-sm text-[var(--color-muted)] hover:underline">
          ← Workflows
        </Link>
        <h1 className="mt-2 text-xl font-semibold">New workflow</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Steps target elements semantically — by role and name, test id or text — not by
          coordinates. Steps that send, pay, submit or delete stop for approval automatically.
        </p>
      </div>

      <WorkflowEditor />
    </div>
  );
}
