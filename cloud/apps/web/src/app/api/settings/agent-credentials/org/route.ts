import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { isOrgAdmin } from "@ghost/core/roles";

/**
 * Organization-wide credential inventory, for an OWNER/ADMIN only.
 *
 * The plain `GET /api/settings/agent-credentials` intentionally stays scoped
 * to the caller's own credentials — this is the separate, admin-gated view of
 * every member's credentials in the org, so an admin can actually audit and
 * revoke what the rest of the org has issued to agents.
 */
export async function GET() {
  const session = await auth();
  if (!session?.user.id || !session.user.orgId) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;

  const membership = await prisma.membership.findUnique({
    where: { userId_orgId: { userId: session.user.id, orgId } },
    select: { role: true },
  });
  if (!membership || !isOrgAdmin(membership.role)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const credentials = await prisma.agentCredential.findMany({
    where: { orgId, revokedAt: null },
    select: {
      id: true,
      name: true,
      tokenHint: true,
      createdAt: true,
      lastUsedAt: true,
      expiresAt: true,
      user: { select: { email: true, name: true } },
    },
    orderBy: { createdAt: "desc" },
  });
  return Response.json({ credentials });
}
