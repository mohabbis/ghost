import Link from "next/link";
import { notFound } from "next/navigation";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { RecordingCompileView, type RecordingView } from "@/components/recording-compile-view";

export const dynamic = "force-dynamic";

export default async function RecordingDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const session = await auth();
  const orgId = session?.user.orgId;
  if (!orgId) notFound();

  const recording = await prisma.recording.findFirst({
    where: { id, orgId },
    select: {
      id: true,
      status: true,
      compileStatus: true,
      rawTraceFilename: true,
      compiledSteps: true,
      compileNotes: true,
      compileError: true,
      workflowId: true,
    },
  });
  if (!recording) notFound();

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div>
        <Link href="/recordings" className="text-sm text-[var(--color-muted)] hover:underline">
          ← Recordings
        </Link>
        <h1 className="mt-2 text-xl font-semibold">{recording.rawTraceFilename ?? "Recording"}</h1>
      </div>

      <RecordingCompileView recordingId={recording.id} initial={recording as RecordingView} />
    </div>
  );
}
