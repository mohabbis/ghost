import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { loadActor } from "@/lib/members";

/**
 * Members of the caller's organization.
 *
 * Readable by any member — knowing who else is in your own tenant is not
 * privileged, and the approvals UI needs it to explain who may approve when
 * separation of duties is on. Mutating routes stay admin-only.
 */
export async function GET(): Promise<Response> {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const actor = await loadActor(orgId, session.user.id);
  if (!actor) return Response.json({ error: "unauthorized" }, { status: 401 });

  const memberships = await prisma.membership.findMany({
    where: { orgId },
    orderBy: { createdAt: "asc" },
    select: {
      role: true,
      createdAt: true,
      user: { select: { id: true, email: true, name: true, image: true } },
    },
  });

  return Response.json({
    members: memberships.map((m) => ({
      userId: m.user.id,
      email: m.user.email,
      name: m.user.name,
      image: m.user.image,
      role: m.role,
      joinedAt: m.createdAt,
      isSelf: m.user.id === actor.userId,
    })),
    viewerRole: actor.role,
  });
}
