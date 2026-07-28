import { redirect } from "next/navigation";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { RunTimeline } from "@/components/run-timeline";

export const dynamic = "force-dynamic";

export default async function RunDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) redirect("/signin");
  const { id } = await params;

  const run = await prisma.run.findFirst({
    where: { id, orgId: session.user.orgId },
    select: { id: true },
  });
  if (!run) redirect("/runs");

  return (
    <div className="mx-auto max-w-3xl">
      <RunTimeline runId={run.id} />
    </div>
  );
}
