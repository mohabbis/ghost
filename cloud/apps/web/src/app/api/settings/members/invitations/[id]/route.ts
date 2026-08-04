import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { actorIsAdmin, loadActor } from "@/lib/members";

/** Revoke an outstanding invitation. Admin-only. */
export async function DELETE(_req: Request, { params }: { params: Promise<{ id: string }> }): Promise<Response> {
  const session = await auth();
  if (!session?.user?.orgId || !session.user.id) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const actor = await loadActor(orgId, session.user.id);
  if (!actorIsAdmin(actor)) {
    return Response.json({ error: "only an owner or admin may revoke" }, { status: 403 });
  }
  const { id } = await params;

  const revoked = await prisma.$transaction(async (tx) => {
    // Org-scoped so an invitation id alone cannot revoke another tenant's.
    const invitation = await tx.invitation.findFirst({
      where: { id, orgId, acceptedAt: null, revokedAt: null },
      select: { id: true, email: true },
    });
    if (!invitation) return null;

    await tx.invitation.update({ where: { id }, data: { revokedAt: new Date() } });
    await appendAuditEvent(
      orgId,
      actor.userId,
      {
        action: "member.invite_revoked",
        entityType: "Invitation",
        entityId: id,
        metadata: { email: invitation.email },
      },
      tx,
    );
    return invitation;
  });

  if (!revoked) return Response.json({ error: "not found" }, { status: 404 });
  return Response.json({ ok: true });
}
