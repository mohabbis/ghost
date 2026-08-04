import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { isOrgAdmin } from "@ghost/core/roles";

export async function DELETE(
  _req: Request,
  context: { params: Promise<{ id: string }> },
) {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const { id } = await context.params;
  const orgId = session.user.orgId;
  const userId = session.user.id;

  // An org admin may revoke any member's credential; anyone else may only
  // revoke their own. Scoping the `where` itself (rather than checking after
  // the fact) means a non-admin's attempt to revoke a colleague's credential
  // fails the same way a nonexistent id would — no row to leak whether it
  // exists in someone else's name.
  const membership = await prisma.membership.findUnique({
    where: { userId_orgId: { userId, orgId } },
    select: { role: true },
  });
  const admin = membership !== null && isOrgAdmin(membership.role);

  const result = await prisma.agentCredential.updateMany({
    where: {
      id,
      orgId,
      revokedAt: null,
      ...(admin ? {} : { userId }),
    },
    data: { revokedAt: new Date() },
  });
  if (!result.count)
    return Response.json({ error: "credential not found" }, { status: 404 });
  return Response.json({ revoked: true });
}
