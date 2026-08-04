import { auth } from "@/auth";
import { prisma } from "@/lib/db";

export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await params;

  const recording = await prisma.recording.findFirst({
    where: { id, orgId: session.user.orgId },
    select: {
      id: true,
      status: true,
      compileStatus: true,
      rawTraceFilename: true,
      compiledSteps: true,
      compileNotes: true,
      compileError: true,
      workflowId: true,
      createdAt: true,
      updatedAt: true,
    },
  });
  if (!recording) return Response.json({ error: "not found" }, { status: 404 });

  return Response.json(recording);
}
