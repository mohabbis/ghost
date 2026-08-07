import Link from "next/link";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";

export const dynamic = "force-dynamic";

const COMPILE_LABEL: Record<string, string> = {
  NONE: "Not compiled",
  RUNNING: "Compiling…",
  READY: "Ready for review",
  FAILED: "Compile failed",
};

export default async function RecordingsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const recordings = orgId
    ? await prisma.recording.findMany({
        where: { orgId },
        orderBy: { createdAt: "desc" },
        select: {
          id: true,
          rawTraceFilename: true,
          compileStatus: true,
          workflowId: true,
          createdAt: true,
        },
      })
    : [];

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <h1 className="text-xl font-semibold">Recordings</h1>
          <p className="mt-1 text-sm text-[var(--color-muted)]">
            Uploaded workflow traces and the editable steps Ghost compiled from them.
          </p>
        </div>
        <Link
          href="/recordings/new"
          className="inline-flex h-8 shrink-0 items-center justify-center rounded-lg bg-[var(--color-accent)] px-3 text-sm font-medium text-[var(--color-accent-fg)] transition-colors hover:opacity-90"
        >
          New recording
        </Link>
      </div>

      {recordings.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">No recordings yet</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              Upload the trace of a demonstrated workflow to turn it into an editable, reviewable
              step plan instead of authoring one by hand.
            </p>
          </CardBody>
        </Card>
      ) : (
        <div className="space-y-2">
          {recordings.map((r) => (
            <Card key={r.id}>
              <CardBody className="flex items-center justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <Link href={`/recordings/${r.id}`} className="text-sm font-medium hover:underline">
                    {r.rawTraceFilename ?? r.id}
                  </Link>
                </div>
                <div className="flex shrink-0 items-center gap-3">
                  {r.workflowId ? (
                    <span className="text-xs text-[var(--color-muted)]">Published</span>
                  ) : (
                    <span className="text-xs text-[var(--color-muted)]">
                      {COMPILE_LABEL[r.compileStatus] ?? r.compileStatus}
                    </span>
                  )}
                </div>
              </CardBody>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
