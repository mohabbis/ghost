import { auth } from "@/auth";
import { prisma } from "@/lib/db";

export async function DELETE(
  _req: Request,
  context: { params: Promise<{ id: string }> },
) {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await context.params;
  const result = await prisma.agentCredential.updateMany({
    where: {
      id,
      orgId: session.user.orgId,
      userId: session.user.id,
      revokedAt: null,
    },
    data: { revokedAt: new Date() },
  });
  if (!result.count)
    return Response.json({ error: "credential not found" }, { status: 404 });
  return Response.json({ revoked: true });
}
